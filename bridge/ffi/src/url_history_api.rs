//! C-ABI wrappers over `look_storage`'s URL history, so the macOS Swift shell
//! records launcher-opened URLs and queries them back through the same look.db
//! linows uses. Direct-store access (own connection, own table), mirroring
//! `todo_api`; both endpoints are best-effort and panic-safe at `lib.rs`.
//! Frecency ranking lives in `look_engine::url_history`, shared with linows.

use crate::state::{cstr_to_string, default_db_path, store_json_allocation};
use look_engine::url_history::ranked_url_history;
use look_storage::SqliteStore;
use std::ffi::CString;
use std::os::raw::c_char;

const JSON_EMPTY_ARRAY: &str = "[]";

/// Records that `url` was opened. Returns false on empty input or any store
/// failure; the caller treats recording as fire-and-forget.
pub(crate) fn look_record_url_hit_impl(url: *const c_char) -> bool {
    let url = cstr_to_string(url);
    if url.trim().is_empty() {
        return false;
    }
    let Ok(store) = SqliteStore::open(default_db_path()) else {
        return false;
    };
    store.record_url_hit(&url).is_ok()
}

/// JSON array of up to `limit` remembered URLs matching `query` (frecency
/// order), or `[]` on any failure.
pub(crate) fn look_recent_urls_json_impl(query: *const c_char, limit: u32) -> *mut c_char {
    let query = cstr_to_string(query);
    let json = SqliteStore::open(default_db_path())
        .ok()
        .and_then(|store| store.recent_urls(&query, limit as usize).ok())
        .map(|entries| ranked_url_history(entries, &query))
        .and_then(|scored| serde_json::to_string(&scored).ok())
        .unwrap_or_else(|| JSON_EMPTY_ARRAY.to_string());
    let cstring =
        CString::new(json).unwrap_or_else(|_| CString::new(JSON_EMPTY_ARRAY).expect("valid"));
    store_json_allocation(cstring)
}
