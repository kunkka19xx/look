//! C-ABI wrapper over `look_emoji`, so the macOS Swift shell shares the same
//! search as linows.

use crate::state::{cstr_to_string, store_json_allocation};
use std::ffi::CString;
use std::os::raw::c_char;

const EMPTY_HITS: &str = "[]";

/// Serialized `Vec<EmojiHit>` for `query`; `[]` on no match.
pub(crate) fn look_emoji_search_json_impl(query: *const c_char, limit: usize) -> *mut c_char {
    let query = cstr_to_string(query);
    let json = serde_json::to_string(&look_emoji::search(&query, limit))
        .unwrap_or_else(|_| EMPTY_HITS.to_string());
    store_json_allocation(
        CString::new(json).unwrap_or_else(|_| CString::new(EMPTY_HITS).expect("valid")),
    )
}
