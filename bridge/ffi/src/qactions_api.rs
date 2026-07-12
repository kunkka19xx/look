//! C-ABI wrapper over `look_qactions`, so the platform shells can fetch the
//! Quick Action descriptors for a selected result. Read the result id + kind,
//! call the shared catalog, hand back a JSON array (empty `[]` on no match or
//! any failure). Mirrors `answers_api`.

use crate::state::{cstr_to_string, store_json_allocation};
use std::ffi::CString;
use std::os::raw::c_char;

const JSON_EMPTY_ARRAY: &str = "[]";

/// JSON array of `ActionDescriptor` for the result `(result_id, kind)`, or `[]`.
pub(crate) fn look_qactions_json_impl(
    result_id: *const c_char,
    kind: *const c_char,
) -> *mut c_char {
    let result_id = cstr_to_string(result_id);
    let kind = cstr_to_string(kind);
    let descriptors = look_qactions::descriptors_for(&result_id, &kind);
    let json = serde_json::to_string(&descriptors).unwrap_or_else(|_| JSON_EMPTY_ARRAY.to_string());
    let cstring =
        CString::new(json).unwrap_or_else(|_| CString::new(JSON_EMPTY_ARRAY).expect("valid"));
    store_json_allocation(cstring)
}
