//! C-ABI wrapper over `look_answers::translate`. The translation logic lives in
//! the shared crate (so macOS and linows share one implementation); this just
//! adapts it to the macOS wire shape: `{original, translated, error}` where
//! `error` is `{code, message}` or `null`.

use crate::state::{cstr_to_string, store_json_allocation};
use std::ffi::CString;
use std::os::raw::c_char;

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
