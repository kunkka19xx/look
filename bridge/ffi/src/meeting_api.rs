//! C-ABI wrapper over `look_ai::meeting`. Panic-safe at `lib.rs`.
//!
//! The shell owns the calendar (EventKit here, something else elsewhere) and
//! hands over the events it fetched; the choice of WHICH meeting, and the link
//! hiding in it, are decided in core so every platform answers the same.

use crate::state::{cstr_to_string, json_cstring_or_null};
use look_ai::meeting::{self, EventInput};
use std::os::raw::c_char;

pub(crate) fn look_meeting_is_join_query_impl(query: *const c_char) -> bool {
    meeting::is_join_query(&cstr_to_string(query))
}

pub(crate) fn look_meeting_next_json_impl(
    events_json: *const c_char,
    now_epoch: i64,
) -> *mut c_char {
    let raw = cstr_to_string(events_json);
    // A malformed payload is "no meeting", never a panic: this runs on the
    // launcher's open path.
    let events: Vec<EventInput> = serde_json::from_str(&raw).unwrap_or_default();
    let found = meeting::next_joinable(&events, now_epoch);
    json_cstring_or_null(found.and_then(|meeting| serde_json::to_string(&meeting).ok()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    fn call(events_json: &str, now: i64) -> String {
        let input = CString::new(events_json).expect("valid");
        let ptr = look_meeting_next_json_impl(input.as_ptr(), now);
        let out = unsafe { std::ffi::CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        crate::state::free_json_allocation(ptr);
        out
    }

    #[test]
    fn returns_the_chosen_meeting_as_json() {
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
    fn returns_json_null_when_nothing_is_joinable() {
        let events = r#"[{"title":"Desk work","startUnixS":1000,"endUnixS":2000}]"#;
        assert_eq!(call(events, 900), "null");
    }

    #[test]
    fn a_malformed_payload_is_not_a_meeting() {
        assert_eq!(call("not json", 900), "null");
        assert_eq!(call("", 900), "null");
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
