//! C-ABI wrapper over `look_storage::parse_web_search`, so the macOS Swift
//! shell builds `Cmd+Enter` search URLs from the same code as linows. Mirrors
//! `answers_api`: read the C strings, call the pure core, hand back an owned
//! C string (freed via `look_free_cstring`), or null when there is no URL.

use crate::state::{cstr_to_string, store_json_allocation};
use look_storage::{parse_web_search, resolve_web_search};
use std::ffi::CString;
use std::os::raw::c_char;

/// URL for `query` on the configured engine/template (Google when `value` is
/// not one the core accepts), or null when the trimmed query is empty.
pub(crate) fn look_web_search_url_impl(value: *const c_char, query: *const c_char) -> *mut c_char {
    let target = resolve_web_search(&cstr_to_string(value));
    let query = cstr_to_string(query);
    let query = query.trim();
    if query.is_empty() {
        return std::ptr::null_mut();
    }
    let url = target.build_url(query);
    let Ok(cstring) = CString::new(url) else {
        return std::ptr::null_mut();
    };
    store_json_allocation(cstring)
}

/// Null when `value` is a valid engine/template, else the shared error message
/// as an owned C string.
pub(crate) fn look_web_search_validate_impl(value: *const c_char) -> *mut c_char {
    let Err(error) = parse_web_search(&cstr_to_string(value)) else {
        return std::ptr::null_mut();
    };
    let Ok(cstring) = CString::new(error.message()) else {
        return std::ptr::null_mut();
    };
    store_json_allocation(cstring)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::look_free_cstring;
    use std::ffi::CStr;

    fn url_for(value: &str, query: &str) -> Option<String> {
        let value = CString::new(value).expect("value cstring");
        let query = CString::new(query).expect("query cstring");
        let ptr = look_web_search_url_impl(value.as_ptr(), query.as_ptr());
        if ptr.is_null() {
            return None;
        }
        let url = unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        look_free_cstring(ptr);
        Some(url)
    }

    fn validate(value: &str) -> Option<String> {
        let value = CString::new(value).expect("value cstring");
        let ptr = look_web_search_validate_impl(value.as_ptr());
        if ptr.is_null() {
            return None;
        }
        let message = unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        look_free_cstring(ptr);
        Some(message)
    }

    #[test]
    fn builds_each_engine_url() {
        assert_eq!(
            url_for("google", "rust").as_deref(),
            Some("https://www.google.com/search?q=rust")
        );
        assert_eq!(
            url_for("duckduckgo", "rust").as_deref(),
            Some("https://duckduckgo.com/?q=rust")
        );
        assert_eq!(
            url_for("bing", "rust").as_deref(),
            Some("https://www.bing.com/search?q=rust")
        );
    }

    #[test]
    fn builds_custom_template_urls() {
        assert_eq!(
            url_for("https://search.yahoo.com/search?p={encoded}", "rust & tea").as_deref(),
            Some("https://search.yahoo.com/search?p=rust+%26+tea")
        );
    }

    #[test]
    fn empty_query_returns_null() {
        assert_eq!(url_for("google", ""), None);
        assert_eq!(url_for("google", "   "), None);
    }

    #[test]
    fn invalid_value_falls_back_to_google() {
        for value in [
            "yahoo",
            "https://example.com/no-placeholder",
            "ftp://example.com/?q={encoded}",
        ] {
            assert_eq!(
                url_for(value, "rust").as_deref(),
                Some("https://www.google.com/search?q=rust"),
                "{value}"
            );
        }
        assert_eq!(url_for("yahoo", "  "), None);
    }

    #[test]
    fn validate_accepts_names_and_templates() {
        assert_eq!(validate("google"), None);
        assert_eq!(validate("  BING "), None);
        assert_eq!(validate(""), None);
        assert_eq!(
            validate("https://search.yahoo.com/search?p={encoded}"),
            None
        );
    }

    #[test]
    fn validate_returns_the_shared_message_for_invalid_values() {
        let message = validate("yahoo").expect("invalid value has a message");
        assert!(message.contains("yahoo"), "{message}");
        assert!(!message.contains("github.com"), "{message}");
    }
}
