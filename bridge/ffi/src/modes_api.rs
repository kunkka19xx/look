use crate::state::store_json_allocation;
use look_engine::modes;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

const NULL_JSON: &str = "null";

fn allocate(json: String) -> *mut c_char {
    let cstring =
        CString::new(json).unwrap_or_else(|_| CString::new(NULL_JSON).expect("valid static json"));
    store_json_allocation(cstring)
}

pub(crate) fn look_modes_list_text_impl() -> *mut c_char {
    allocate(modes::list_text())
}

/// Argv in as a JSON array (program name already dropped), the decision out as
/// `{"kind":"normal"|"query"|"list_modes"|"unknown_mode", ...}`.
pub(crate) fn look_modes_parse_json_impl(argv_json: *const c_char) -> *mut c_char {
    if argv_json.is_null() {
        return allocate(NULL_JSON.to_string());
    }
    let Ok(raw) = (unsafe { CStr::from_ptr(argv_json) }).to_str() else {
        return allocate(NULL_JSON.to_string());
    };
    let Ok(args) = serde_json::from_str::<Vec<String>>(raw) else {
        return allocate(NULL_JSON.to_string());
    };

    let value = match modes::parse_args(args) {
        modes::Launch::Normal => serde_json::json!({ "kind": "normal" }),
        modes::Launch::ListModes => serde_json::json!({ "kind": "list_modes" }),
        modes::Launch::Query { text, toggle } => {
            serde_json::json!({ "kind": "query", "text": text, "toggle": toggle })
        }
        modes::Launch::UnknownMode(name) => {
            serde_json::json!({ "kind": "unknown_mode", "name": name })
        }
    };
    allocate(value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> serde_json::Value {
        let json = CString::new(serde_json::to_string(args).unwrap()).unwrap();
        let ptr = look_modes_parse_json_impl(json.as_ptr());
        let out = unsafe { CStr::from_ptr(ptr) }.to_str().unwrap().to_string();
        crate::look_free_cstring(ptr);
        serde_json::from_str(&out).unwrap()
    }

    #[test]
    fn a_mode_name_becomes_its_query() {
        let parsed = parse(&["clipboard", "pass"]);

        assert_eq!(parsed["kind"], "query");
        assert_eq!(parsed["text"], "c\"pass");
    }

    #[test]
    fn an_unrecognised_word_stays_a_normal_launch() {
        assert_eq!(parse(&["clipbaord"])["kind"], "normal");
        assert_eq!(parse(&[])["kind"], "normal");
    }

    #[test]
    fn an_explicit_unknown_mode_reports_the_name() {
        let parsed = parse(&["--mode", "clipbaord"]);

        assert_eq!(parsed["kind"], "unknown_mode");
        assert_eq!(parsed["name"], "clipbaord");
    }

    #[test]
    fn listing_is_asked_for_by_kind() {
        assert_eq!(parse(&["--list-modes"])["kind"], "list_modes");
    }

    #[test]
    fn junk_input_returns_null_rather_than_panicking() {
        let json = CString::new("not json").unwrap();
        let ptr = look_modes_parse_json_impl(json.as_ptr());
        let out = unsafe { CStr::from_ptr(ptr) }.to_str().unwrap().to_string();
        crate::look_free_cstring(ptr);

        assert_eq!(out, NULL_JSON);
        assert_eq!(
            unsafe { CStr::from_ptr(look_modes_parse_json_impl(std::ptr::null())) }
                .to_str()
                .unwrap(),
            NULL_JSON
        );
    }

    #[test]
    fn the_listing_comes_back_whole() {
        let ptr = look_modes_list_text_impl();
        let text = unsafe { CStr::from_ptr(ptr) }.to_str().unwrap().to_string();
        crate::look_free_cstring(ptr);

        assert!(text.contains("clipboard"));
        assert!(text.contains("macOS only"));
    }
}
