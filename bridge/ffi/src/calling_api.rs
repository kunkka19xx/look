//! C-ABI wrapper over `look_ai::calling`. Panic-safe at `lib.rs`.
//!
//! The shell owns the address book (Contacts on macOS) and the dialling; the
//! words and the URL are decided in core so every platform reads "call mom on
//! facetime" the same way.

use crate::state::{cstr_to_string, json_cstring_or_null};
use look_ai::calling::{self, Modality};
use std::os::raw::c_char;

/// The call request in `query` as JSON (`{"name":"mom","modality":null}`), or
/// the literal `null` when this is an ordinary search.
pub(crate) fn look_call_query_json_impl(query: *const c_char) -> *mut c_char {
    let request = calling::call_query(&cstr_to_string(query));
    json_cstring_or_null(request.and_then(|request| serde_json::to_string(&request).ok()))
}

/// The modality a bare "call" means, as an id. Read from core so the shell
/// never hard-codes a second opinion.
pub(crate) fn look_call_default_modality_impl() -> *mut c_char {
    json_cstring_or_null(Some(Modality::DEFAULT.id().to_string()))
}

/// The URL that dials `handle` with `modality` (an id from `Modality::id`), or
/// null when the modality is unknown.
pub(crate) fn look_call_url_impl(modality: *const c_char, handle: *const c_char) -> *mut c_char {
    let handle = cstr_to_string(handle);
    let Some(modality) = Modality::from_id(&cstr_to_string(modality)) else {
        return std::ptr::null_mut();
    };
    let url = calling::call_url(modality, &handle);
    json_cstring_or_null(Some(url))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    fn call(f: impl Fn(*const c_char) -> *mut c_char, input: &str) -> String {
        let input = CString::new(input).expect("valid");
        let ptr = f(input.as_ptr());
        if ptr.is_null() {
            return String::new();
        }
        let out = unsafe { std::ffi::CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        crate::state::free_json_allocation(ptr);
        out
    }

    #[test]
    fn the_grammar_crosses_the_boundary() {
        assert_eq!(
            call(look_call_query_json_impl, "call mom"),
            r#"{"name":"mom","modality":null}"#
        );
        assert_eq!(
            call(look_call_query_json_impl, "facetime sarah lee"),
            r#"{"name":"sarah lee","modality":"face_time_video"}"#
        );
        assert_eq!(call(look_call_query_json_impl, "recall that"), "null");
    }

    #[test]
    fn urls_are_built_from_the_modality_id() {
        let modality = CString::new("message").expect("valid");
        let handle = CString::new("+1 (555) 123-4567").expect("valid");
        let ptr = look_call_url_impl(modality.as_ptr(), handle.as_ptr());
        let url = unsafe { std::ffi::CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        crate::state::free_json_allocation(ptr);
        assert_eq!(url, "sms:+15551234567");
    }

    #[test]
    fn an_unknown_modality_is_null_not_a_guess() {
        let modality = CString::new("carrier pigeon").expect("valid");
        let handle = CString::new("+15551234567").expect("valid");
        assert!(look_call_url_impl(modality.as_ptr(), handle.as_ptr()).is_null());
    }
}
