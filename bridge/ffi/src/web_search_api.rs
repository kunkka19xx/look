//! C-ABI wrapper over `look_storage::SearchEngine`, so the macOS Swift shell
//! builds `Cmd+Enter` search URLs from the same code as linows. Mirrors
//! `answers_api`: read the C strings, call the pure core, hand back an owned
//! C string (freed via `look_free_cstring`), or null when there is no query.

use crate::state::{cstr_to_string, store_json_allocation};
use look_storage::SearchEngine;
use std::ffi::CString;
use std::os::raw::c_char;

/// URL for `query` on `engine_key`, or null when the trimmed query is empty.
/// Missing or unknown engine keys fall back to Google, like the config file.
pub(crate) fn look_web_search_url_impl(
    engine_key: *const c_char,
    query: *const c_char,
) -> *mut c_char {
    let engine = SearchEngine::from_key(&cstr_to_string(engine_key));
    let query = cstr_to_string(query);
    let query = query.trim();
    if query.is_empty() {
        return std::ptr::null_mut();
    }
    let url = engine.build_search_url(query);
    let Ok(cstring) = CString::new(url) else {
        return std::ptr::null_mut();
    };
    store_json_allocation(cstring)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::look_free_cstring;
    use std::ffi::CStr;

    fn url_for(engine: &str, query: &str) -> Option<String> {
        let engine = CString::new(engine).expect("engine cstring");
        let query = CString::new(query).expect("query cstring");
        let ptr = look_web_search_url_impl(engine.as_ptr(), query.as_ptr());
        if ptr.is_null() {
            return None;
        }
        let url = unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        look_free_cstring(ptr);
        Some(url)
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
    fn empty_query_returns_null() {
        assert_eq!(url_for("google", ""), None);
        assert_eq!(url_for("google", "   "), None);
    }

    #[test]
    fn unknown_engine_falls_back_to_google() {
        assert_eq!(
            url_for("yahoo", "rust").as_deref(),
            Some("https://www.google.com/search?q=rust")
        );
    }
}
