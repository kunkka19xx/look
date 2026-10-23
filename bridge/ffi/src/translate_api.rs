//! C-ABI wrappers over `look_answers::translate` and its language parsing. The
//! logic lives in the shared crate (so macOS and linows share one
//! implementation); this just adapts it to the macOS wire shapes:
//! `{original, translated, error}` where `error` is `{code, message}` or `null`,
//! and `[{code, label}]` for the configured target languages.

use crate::state::{cstr_to_string, store_json_allocation};
use std::ffi::CString;
use std::os::raw::c_char;

const JSON_LANGUAGES_FALLBACK: &str = "[]";
const JSON_ERROR_FALLBACK: &str = "{\"original\":\"\",\"translated\":\"\",\"error\":{\"code\":\"serialize_failed\",\"message\":\"Failed to serialize translation response\"}}";

pub(crate) fn look_translate_json_impl(
    text: *const c_char,
    target_lang: *const c_char,
) -> *mut c_char {
    let text = cstr_to_string(text);
    let target_lang = cstr_to_string(target_lang);
    let result = look_answers::translate(&text, &target_lang);

    let error = result
        .error
        .map(|e| serde_json::json!({ "code": e.code(), "message": e.message() }));
    let payload = serde_json::json!({
        "original": result.original,
        "translated": result.translated,
        "error": error,
    });

    let json = serde_json::to_string(&payload).unwrap_or_else(|_| JSON_ERROR_FALLBACK.to_string());
    let cstring =
        CString::new(json).unwrap_or_else(|_| CString::new(JSON_ERROR_FALLBACK).expect("valid"));
    store_json_allocation(cstring)
}

/// Parses the raw `translate_languages` value into the macOS wire shape: a JSON
/// array of `{code, label}` in the configured order. The parsing and the label
/// table live in `look_answers`, shared with linows.
pub(crate) fn look_translate_languages_json_impl(raw: *const c_char) -> *mut c_char {
    let raw = cstr_to_string(raw);
    let languages = look_answers::translate_languages(&raw);

    let json =
        serde_json::to_string(&languages).unwrap_or_else(|_| JSON_LANGUAGES_FALLBACK.to_string());
    let cstring = CString::new(json)
        .unwrap_or_else(|_| CString::new(JSON_LANGUAGES_FALLBACK).expect("valid"));
    store_json_allocation(cstring)
}
