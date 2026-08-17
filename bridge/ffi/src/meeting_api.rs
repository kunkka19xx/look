//! C-ABI wrapper over `look_ai::meeting`. Panic-safe at `lib.rs`.
//!
//! The shell owns the calendar (EventKit here, something else elsewhere) and
//! hands over the events it fetched; the choice of WHICH meeting, and the link
//! hiding in it, are decided in core so every platform answers the same.

use crate::state::{cstr_to_string, json_cstring_or_null};
use look_ai::meeting::{self, EventInput};
use std::os::raw::c_char;

const JSON_EMPTY_OUTCOME: &str = r#"{"meetings":[],"withoutLink":[]}"#;

/// The join request in `query` as JSON (`{"name": "standup"}`, or `{}` for a
/// bare "join"), or the literal `null` when this is an ordinary search.
pub(crate) fn look_meeting_join_query_json_impl(query: *const c_char) -> *mut c_char {
    let request = meeting::join_query(&cstr_to_string(query));
    json_cstring_or_null(request.and_then(|request| serde_json::to_string(&request).ok()))
}

/// What a `join` found, as JSON: `{"meetings":[...],"withoutLink":["Testing"]}`.
/// The head of `meetings` is what a bare "join" takes, so a picker and a direct
/// join can never disagree about which meeting is next. `withoutLink` names the
/// events that answered to the name but carry no link, so the shell can say
/// which meeting is missing one instead of claiming it does not exist.
pub(crate) fn look_meeting_outcome_json_impl(
    events_json: *const c_char,
    now_epoch: i64,
    name: *const c_char,
) -> *mut c_char {
    let raw = cstr_to_string(events_json);
    // A malformed payload is "no meetings", never a panic: this runs on the
    // launcher's open path.
    let events: Vec<EventInput> = serde_json::from_str(&raw).unwrap_or_default();
    // An empty name is "no name": the shell passes "" rather than juggling a
    // null pointer across the boundary.
    let name = cstr_to_string(name);
    let name = if name.trim().is_empty() {
        None
    } else {
        Some(name)
    };
    let outcome = meeting::join_outcome(&events, now_epoch, name.as_deref());
    json_cstring_or_null(Some(
        serde_json::to_string(&outcome).unwrap_or_else(|_| JSON_EMPTY_OUTCOME.to_string()),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    fn call(events_json: &str, now: i64) -> String {
        call_named(events_json, now, "")
    }

    fn call_named(events_json: &str, now: i64, name: &str) -> String {
        let input = CString::new(events_json).expect("valid");
        let name = CString::new(name).expect("valid");
        let ptr = look_meeting_outcome_json_impl(input.as_ptr(), now, name.as_ptr());
        let out = unsafe { std::ffi::CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        crate::state::free_json_allocation(ptr);
        out
    }

    fn join_query(query: &str) -> String {
        let input = CString::new(query).expect("valid");
        let ptr = look_meeting_join_query_json_impl(input.as_ptr());
        let out = unsafe { std::ffi::CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        crate::state::free_json_allocation(ptr);
        out
    }

    #[test]
    fn the_join_grammar_crosses_the_boundary() {
        assert_eq!(join_query("join"), "{}");
        assert_eq!(join_query("join my next meeting"), "{}");
        assert_eq!(join_query("join testing"), r#"{"name":"testing"}"#);
        assert_eq!(join_query("standup notes"), "null");
    }

    #[test]
    fn a_name_selects_among_the_events() {
        let events = r#"[
            {"title":"Sooner","startUnixS":1000,"endUnixS":2000,
             "url":"https://meet.jit.si/sooner"},
            {"title":"Testing","startUnixS":5000,"endUnixS":6000,
             "url":"https://meet.jit.si/testing"}
        ]"#;
        assert!(call_named(events, 900, "testing").contains("\"title\":\"Testing\""));
        // Empty name means "whatever is next", not "match nothing".
        assert!(call_named(events, 900, "").contains("\"title\":\"Sooner\""));
        assert!(call_named(events, 900, "retro").contains(r#""meetings":[]"#));
    }

    #[test]
    fn returns_the_candidates_as_json() {
        let events = r#"[
            {"title":"Standup","startUnixS":1000,"endUnixS":2000,
             "url":"https://meet.google.com/abc-defg-hij"}
        ]"#;
        let json = call(events, 900);
        assert!(json.contains("\"title\":\"Standup\""), "got {json}");
        assert!(json.contains("\"provider\":\"meet\""), "got {json}");
        assert!(
            json.contains("\"providerLabel\":\"Google Meet\""),
            "got {json}"
        );
        assert!(json.contains("\"startsInS\":100"), "got {json}");
    }

    #[test]
    fn names_what_it_matched_but_could_not_join() {
        let events = r#"[{"title":"Desk work","startUnixS":1000,"endUnixS":2000}]"#;
        let json = call(events, 900);
        assert!(json.contains(r#""meetings":[]"#), "got {json}");
        assert!(
            json.contains(r#""withoutLink":["Desk work"]"#),
            "got {json}"
        );
    }

    #[test]
    fn a_malformed_payload_is_not_a_meeting() {
        assert!(call("not json", 900).contains(r#""meetings":[]"#));
        assert!(call("", 900).contains(r#""meetings":[]"#));
    }

    #[test]
    fn optional_fields_may_be_absent() {
        // The shell omits url/location/notes/allDay when empty; serde defaults
        // must cover that or every event would fail to decode.
        let events = r#"[
            {"title":"Sync","startUnixS":1000,"endUnixS":2000,
             "notes":"Join https://meet.jit.si/sync"}
        ]"#;
        assert!(call(events, 900).contains("meet.jit.si/sync"));
    }
}
