//! C-ABI wrapper over `look_storage`'s web search target, so the macOS shell
//! builds `Cmd+Enter` search URLs from the same code as linows.

use crate::state::{cstr_to_string, store_json_allocation};
use look_storage::{parse_web_search, resolve_web_search};
use std::ffi::CString;
use std::os::raw::c_char;

pub(crate) fn look_web_search_url_impl(value: *const c_char, query: *const c_char) -> *mut c_char {
    let query = cstr_to_string(query);
    let query = query.trim();
    if query.is_empty() {
        return std::ptr::null_mut();
    }
    let url = resolve_web_search(&cstr_to_string(value)).build_url(query);
    CString::new(url).map_or(std::ptr::null_mut(), store_json_allocation)
}

pub(crate) fn look_web_search_is_valid_impl(value: *const c_char) -> bool {
    parse_web_search(&cstr_to_string(value)).is_some()
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

    #[test]
    fn builds_the_url_or_null_for_a_blank_query() {
        assert_eq!(
            url_for("https://search.yahoo.com/search?p=%s", "rust & tea").as_deref(),
            Some("https://search.yahoo.com/search?p=rust+%26+tea")
        );
        assert_eq!(
            url_for("yahoo", "rust").as_deref(),
            Some("https://www.google.com/search?q=rust")
        );
        assert_eq!(url_for("google", "   "), None);
    }

    #[test]
    fn is_valid_matches_the_core_parser() {
        let valid = |value: &str| {
            let value = CString::new(value).expect("value cstring");
            look_web_search_is_valid_impl(value.as_ptr())
        };
        assert!(valid("  BING "));
        assert!(valid("https://search.yahoo.com/search?p=%s"));
        assert!(!valid("yahoo"));
    }
}
