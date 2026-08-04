//! C-ABI wrappers over `look_calc`, so the macOS Swift shell shares the same
//! arithmetic engine as linows.

use crate::state::{cstr_to_string, json_cstring_or_null, store_json_allocation};
use std::ffi::CString;
use std::os::raw::c_char;

const JSON_ERROR_FALLBACK: &str =
    "{\"calculation\":null,\"error\":\"Failed to serialize calculation response\"}";

/// Evaluates `expr` as arithmetic - the dedicated `/calc` panel, where the
/// user already declared this is a calculation and a specific error (division
/// by zero, unbalanced parens, ...) is worth showing. Mirrors
/// `translate_api`'s `{result, error}` shape: `{"calculation": Calculation |
/// null, "error": string | null}`.
pub(crate) fn look_calc_eval_json_impl(expr: *const c_char) -> *mut c_char {
    let expr = cstr_to_string(expr);
    let payload = match look_calc::eval(&expr) {
        Ok(calc) => serde_json::json!({ "calculation": calc, "error": null }),
        Err(message) => serde_json::json!({ "calculation": null, "error": message }),
    };
    let json = serde_json::to_string(&payload).unwrap_or_else(|_| JSON_ERROR_FALLBACK.to_string());
    let cstring =
        CString::new(json).unwrap_or_else(|_| CString::new(JSON_ERROR_FALLBACK).expect("valid"));
    store_json_allocation(cstring)
}

/// The main search field: resolves `query` only when it was clearly meant as
/// arithmetic (see `look_calc::is_math`). No error text - a miss just means
/// "leave the search alone", never a row worth explaining. Returns a
/// serialized `Calculation` on a hit, or the JSON literal `null` otherwise.
/// Cheap enough to call on every keystroke.
pub(crate) fn look_calc_inline_json_impl(query: *const c_char) -> *mut c_char {
    let query = cstr_to_string(query);
    json_cstring_or_null(
        look_calc::eval_query(&query).and_then(|calc| serde_json::to_string(&calc).ok()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn json_of(ptr: *mut c_char) -> String {
        assert!(!ptr.is_null());
        unsafe { std::ffi::CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn eval_round_trips_a_calculation() {
        let expr = CString::new("2 + 2").unwrap();
        let json = json_of(look_calc_eval_json_impl(expr.as_ptr()));
        assert!(json.contains("\"display\":\"4\""), "{json}");
        assert!(json.contains("\"raw\":\"4\""), "{json}");
        assert!(json.contains("\"error\":null"), "{json}");
    }

    #[test]
    fn eval_reports_the_specific_error() {
        let expr = CString::new("1 / 0").unwrap();
        let json = json_of(look_calc_eval_json_impl(expr.as_ptr()));
        assert!(json.contains("\"calculation\":null"), "{json}");
        assert!(json.contains("Division by zero"), "{json}");
    }

    #[test]
    fn inline_respects_the_intent_gate() {
        let math = CString::new("1/1000").unwrap();
        let json = json_of(look_calc_inline_json_impl(math.as_ptr()));
        assert!(json.contains("\"display\":\"0.001\""), "{json}");

        let not_math = CString::new("20-05-2026").unwrap();
        assert_eq!(
            json_of(look_calc_inline_json_impl(not_math.as_ptr())),
            "null"
        );
    }

    #[test]
    fn null_pointers_do_not_crash() {
        let json = json_of(look_calc_eval_json_impl(std::ptr::null()));
        assert!(json.contains("\"calculation\":null"), "{json}");
        assert_eq!(
            json_of(look_calc_inline_json_impl(std::ptr::null())),
            "null"
        );
    }
}
