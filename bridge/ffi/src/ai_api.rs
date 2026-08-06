//! FFI impls for the shared AI brain (`core/ai`). Thin: convert C strings,
//! delegate, hand JSON back through the tracked allocation store.

use std::ffi::{CString, c_char};

use crate::state;

pub(crate) fn look_ai_is_referent_impl(phrase: *const c_char) -> bool {
    let phrase = state::cstr_to_string(phrase);
    look_ai::referent::is_referent(&phrase)
}

pub(crate) fn look_ai_markdown_segments_json_impl(text: *const c_char) -> *mut c_char {
    let text = state::cstr_to_string(text);
    let json = look_ai::markdown::segments_json(&text);
    let cstring = CString::new(json).unwrap_or_else(|_| CString::new("[]").expect("valid"));
    state::store_json_allocation(cstring)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn referent_roundtrip() {
        let it = CString::new("this event").unwrap();
        assert!(look_ai_is_referent_impl(it.as_ptr()));
        let named = CString::new("dentist").unwrap();
        assert!(!look_ai_is_referent_impl(named.as_ptr()));
    }
}
