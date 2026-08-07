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

pub(crate) fn look_ai_resolve_impl(request_json: *const c_char) -> *mut c_char {
    let request = state::cstr_to_string(request_json);
    let json = look_ai::resolve::resolve_json(&request);
    let cstring = CString::new(json).unwrap_or_else(|_| {
        CString::new(r#"{"outcome":"invalid","message":"encode failed"}"#).expect("valid")
    });
    state::store_json_allocation(cstring)
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
