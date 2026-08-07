//! FFI impls for the shared AI brain (`core/ai`). Thin: convert C strings,
//! delegate, hand JSON back through the tracked allocation store.

use std::ffi::{CString, c_char};

use crate::state;

pub(crate) fn look_ai_is_referent_impl(phrase: *const c_char) -> bool {
    let phrase = state::cstr_to_string(phrase);
    look_ai::referent::is_referent(&phrase)
}

pub(crate) fn look_ai_query_window_impl(query: *const c_char, now_epoch: i64) -> *mut c_char {
    let query = state::cstr_to_string(query);
    match look_ai::window::query_window_json(&query, now_epoch) {
        Some(json) => {
            let cstring = CString::new(json).unwrap_or_else(|_| CString::new("null").expect("valid"));
            state::store_json_allocation(cstring)
        }
        None => std::ptr::null_mut(),
    }
}

pub(crate) fn look_ai_future_leaning_impl(
    phrase: *const c_char,
    resolved_epoch: i64,
    now_epoch: i64,
) -> i64 {
    let phrase = state::cstr_to_string(phrase);
    look_ai::window::future_leaning(&phrase, resolved_epoch, now_epoch)
}

pub(crate) fn look_ai_markdown_segments_json_impl(text: *const c_char) -> *mut c_char {
    let text = state::cstr_to_string(text);
    let json = look_ai::markdown::segments_json(&text);
    let cstring = CString::new(json).unwrap_or_else(|_| CString::new("[]").expect("valid"));
    state::store_json_allocation(cstring)
}

pub(crate) fn look_ai_plan_impl(
    host: *const c_char,
    model: *const c_char,
    query: *const c_char,
) -> *mut c_char {
    let host = state::cstr_to_string(host);
    let model = state::cstr_to_string(model);
    let query = state::cstr_to_string(query);
    match look_ai::planner::plan(&host, &model, &query) {
        Some(call) => {
            let json = call.to_string();
            let cstring = CString::new(json).unwrap_or_else(|_| CString::new("null").expect("valid"));
            state::store_json_allocation(cstring)
        }
        None => std::ptr::null_mut(),
    }
}

pub(crate) fn look_ai_warm_planner_impl(host: *const c_char, model: *const c_char) {
    let host = state::cstr_to_string(host);
    let model = state::cstr_to_string(model);
    look_ai::planner::warm(&host, &model);
}

pub(crate) fn look_ai_conversations_json_impl(path: *const c_char) -> *mut c_char {
    let path = state::cstr_to_string(path);
    let json = look_ai::conversations::load_json(std::path::Path::new(&path));
    let cstring = CString::new(json).unwrap_or_else(|_| CString::new("[]").expect("valid"));
    state::store_json_allocation(cstring)
}

pub(crate) fn look_ai_conversation_upsert_impl(
    path: *const c_char,
    conversation_json: *const c_char,
) -> bool {
    let path = state::cstr_to_string(path);
    let json = state::cstr_to_string(conversation_json);
    look_ai::conversations::upsert_json(std::path::Path::new(&path), &json)
}

pub(crate) fn look_ai_conversation_delete_impl(
    path: *const c_char,
    id: *const c_char,
) -> bool {
    let path = state::cstr_to_string(path);
    let id = state::cstr_to_string(id);
    look_ai::conversations::delete(std::path::Path::new(&path), &id)
}

const ENCODE_FAILED: &str = r#"{"outcome":"invalid","message":"encode failed"}"#;

pub(crate) fn look_ai_resolve_impl(request_json: *const c_char) -> *mut c_char {
    let request_str = state::cstr_to_string(request_json);
    let json = match serde_json::from_str::<look_ai::resolve::ResolveRequest>(&request_str) {
        Ok(request) => {
            // Empty lists => resolve against the loaded targets (the hot path).
            // A request that carries its own candidates (block_time) indexes them
            // inline via `resolve`.
            let outcome = if request.events.is_empty() && request.reminders.is_empty() {
                state::resolve_ai_with_targets(&request)
            } else {
                look_ai::resolve::resolve(&request)
            };
            serde_json::to_string(&outcome).unwrap_or_else(|_| ENCODE_FAILED.to_string())
        }
        Err(_) => r#"{"outcome":"invalid","message":"Bad resolve request."}"#.to_string(),
    };
    let cstring = CString::new(json).unwrap_or_else(|_| CString::new(ENCODE_FAILED).expect("valid"));
    state::store_json_allocation(cstring)
}

/// Handle AI input as a possible memory command ("remember …", "forget …",
/// "memories"). Returns feedback to show (skip the model), or null for normal
/// input. Free with `look_free_cstring`.
pub(crate) fn look_ai_memory_command_impl(
    path: *const c_char,
    input: *const c_char,
) -> *mut c_char {
    let path = state::cstr_to_string(path);
    let input = state::cstr_to_string(input);
    match look_ai::memory::handle_command(std::path::Path::new(&path), &input) {
        Some(feedback) => match CString::new(feedback) {
            Ok(cstring) => state::store_json_allocation(cstring),
            Err(_) => std::ptr::null_mut(),
        },
        None => std::ptr::null_mut(),
    }
}

/// The stored facts as a model context block (empty string when none). Free with
/// `look_free_cstring`.
pub(crate) fn look_ai_memory_context_impl(path: *const c_char) -> *mut c_char {
    let path = state::cstr_to_string(path);
    let block = look_ai::memory::context_block(std::path::Path::new(&path));
    let cstring = CString::new(block).unwrap_or_default();
    state::store_json_allocation(cstring)
}

/// The stored facts as a JSON array. Free with `look_free_cstring`.
pub(crate) fn look_ai_memory_list_json_impl(path: *const c_char) -> *mut c_char {
    let path = state::cstr_to_string(path);
    let json = look_ai::memory::load_json(std::path::Path::new(&path));
    let cstring = CString::new(json).unwrap_or_else(|_| CString::new("[]").expect("valid"));
    state::store_json_allocation(cstring)
}

/// Load the AI mutate targets once (events + reminders as JSON arrays), so
/// per-keystroke resolves can omit them. Parse failures leave the prior set.
pub(crate) fn look_ai_load_targets_impl(
    events_json: *const c_char,
    reminders_json: *const c_char,
) {
    let events_json = state::cstr_to_string(events_json);
    let reminders_json = state::cstr_to_string(reminders_json);
    let events = serde_json::from_str::<Vec<look_ai::resolve::EventCandidate>>(&events_json)
        .unwrap_or_default();
    let reminders =
        serde_json::from_str::<Vec<look_ai::resolve::ReminderCandidate>>(&reminders_json)
            .unwrap_or_default();
    state::load_ai_targets(events, reminders);
}

pub(crate) fn look_ai_parse_explicit_impl(
    input: *const c_char,
    model_available: bool,
) -> *mut c_char {
    let input = state::cstr_to_string(input);
    match look_ai::explicit::parse_json(&input, model_available) {
        Some(json) => {
            let cstring = CString::new(json).unwrap_or_else(|_| CString::new("null").expect("valid"));
            state::store_json_allocation(cstring)
        }
        None => std::ptr::null_mut(),
    }
}

pub(crate) fn look_ai_chat_start_impl(
    host: *const c_char,
    model: *const c_char,
    messages_json: *const c_char,
) -> u64 {
    let host = state::cstr_to_string(host);
    let model = state::cstr_to_string(model);
    let messages = state::cstr_to_string(messages_json);
    look_ai::chat::start(&host, &model, &messages)
}

pub(crate) fn look_ai_chat_poll_impl(id: u64) -> *mut c_char {
    match look_ai::chat::poll(id) {
        Some(json) => {
            let cstring = CString::new(json).unwrap_or_else(|_| CString::new("null").expect("valid"));
            state::store_json_allocation(cstring)
        }
        None => std::ptr::null_mut(),
    }
}

pub(crate) fn look_ai_chat_cancel_impl(id: u64) {
    look_ai::chat::cancel(id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn referent_roundtrip() {
        let it = CString::new("this event").unwrap();
        assert!(look_ai_is_referent_impl(it.as_ptr()));
        let named = CString::new("dentist").unwrap();
        assert!(!look_ai_is_referent_impl(named.as_ptr()));
    }
}
