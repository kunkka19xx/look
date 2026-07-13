//! C-ABI wrappers over `look_answers`, so the macOS Swift shell can call the
//! shared instant-answer logic. Mirrors `translate_api`: read the C string,
//! call the pure core, hand back an owned JSON C string (freed via
//! `look_free_cstring`). Best-effort - any failure returns the JSON literal
//! `null` rather than erroring.

use crate::state::{cstr_to_string, store_json_allocation};
use std::ffi::CString;
use std::os::raw::c_char;

const JSON_NULL: &str = "null";

/// Resolves an instant answer for `query`. Returns a serialized `Answer` object
/// on a hit, or the JSON literal `null` when there's no match / no result.
pub(crate) fn look_instant_answer_json_impl(query: *const c_char) -> *mut c_char {
    let query = cstr_to_string(query);
    let json = look_answers::instant_answer(&query)
        .and_then(|answer| serde_json::to_string(&answer).ok())
        .unwrap_or_else(|| JSON_NULL.to_string());
    let cstring = CString::new(json).unwrap_or_else(|_| CString::new(JSON_NULL).expect("valid"));
    store_json_allocation(cstring)
}

/// Network-free gate: whether `query` matches any instant-answer provider's
/// shape, so the caller can decide whether to fetch without spending a request.
pub(crate) fn look_instant_has_match_impl(query: *const c_char) -> bool {
    let query = cstr_to_string(query);
    look_answers::has_match(&query)
}

/// JSON array of up to `limit` autocomplete suggestion strings for `query`.
pub(crate) fn look_web_suggestions_json_impl(query: *const c_char, limit: u32) -> *mut c_char {
    let query = cstr_to_string(query);
    let suggestions = look_answers::web_suggestions(&query, limit as usize);
    json_cstring(serde_json::to_string(&suggestions).ok())
}

/// Serialized DuckDuckGo answer object, or the JSON literal `null`.
pub(crate) fn look_duckduckgo_answer_json_impl(query: *const c_char) -> *mut c_char {
    let query = cstr_to_string(query);
    json_cstring(
        look_answers::duckduckgo_answer(&query).and_then(|a| serde_json::to_string(&a).ok()),
    )
}

/// Serialized Wikipedia answer object for `search_term`, or the JSON literal `null`.
pub(crate) fn look_wikipedia_answer_json_impl(search_term: *const c_char) -> *mut c_char {
    let search_term = cstr_to_string(search_term);
    json_cstring(
        look_answers::wikipedia_answer(&search_term).and_then(|a| serde_json::to_string(&a).ok()),
    )
}

/// Serialized `UrlMatch` object for `query`, or the JSON literal `null` when the
/// query is not a URL.
pub(crate) fn look_classify_url_json_impl(query: *const c_char) -> *mut c_char {
    let query = cstr_to_string(query);
    json_cstring(look_answers::classify_url(&query).and_then(|m| serde_json::to_string(&m).ok()))
}

/// The definitional entity for `query` as a JSON string, or the JSON literal `null`.
pub(crate) fn look_definitional_entity_json_impl(query: *const c_char) -> *mut c_char {
    let query = cstr_to_string(query);
    json_cstring(
        look_answers::definitional_entity(&query).and_then(|e| serde_json::to_string(&e).ok()),
    )
}

/// Wraps an optional pre-serialized JSON string into an owned C string, falling
/// back to the JSON literal `null`.
fn json_cstring(json: Option<String>) -> *mut c_char {
    let json = json.unwrap_or_else(|| JSON_NULL.to_string());
    let cstring = CString::new(json).unwrap_or_else(|_| CString::new(JSON_NULL).expect("valid"));
    store_json_allocation(cstring)
}
