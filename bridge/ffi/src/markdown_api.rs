//! Markdown for the shells: they decide how a heading or a link looks, this
//! decides what is one.

use std::ffi::{CString, c_char};

use crate::state;

pub(crate) fn look_markdown_blocks_json_impl(text: *const c_char) -> *mut c_char {
    json(look_markdown::blocks_json(&state::cstr_to_string(text)))
}

pub(crate) fn look_ai_markdown_segments_json_impl(text: *const c_char) -> *mut c_char {
    json(look_markdown::segments_json(&state::cstr_to_string(text)))
}

fn json(text: String) -> *mut c_char {
    let cstring = CString::new(text).unwrap_or_else(|_| CString::new("[]").expect("valid"));
    state::store_json_allocation(cstring)
}
