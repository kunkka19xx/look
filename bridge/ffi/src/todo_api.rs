//! C-ABI wrappers over `look_todo`, so the macOS Swift shell persists the
//! /todo command's tasks through the same store linows uses. Two endpoints:
//! `list` returns the full task set as JSON, `save` replaces it from JSON.
//! Both are best-effort and panic-safe at the `lib.rs` boundary.

use crate::state::{cstr_to_string, default_db_path, store_json_allocation};
use look_todo::{TodoStore, TodoTask};
use std::ffi::CString;
use std::os::raw::c_char;

const JSON_EMPTY_ARRAY: &str = "[]";

/// Opens the todo table in the same look.db the engine uses (separate
/// connection, own table). Opened per call rather than held: /todo reads
/// once per app launch and writes on explicit save, so connection reuse
/// buys nothing, and resolving the path each call keeps `LOOK_DB_PATH`
/// honored (tests point it at a scratch database).
fn with_store<T>(f: impl FnOnce(&mut TodoStore) -> T) -> Option<T> {
    let mut store = TodoStore::open(default_db_path()).ok()?;
    Some(f(&mut store))
}

/// Returns every stored task (within the one-year retention window) as a
/// JSON array of `TodoTask`. Empty array on any failure.
pub(crate) fn look_todo_list_json_impl() -> *mut c_char {
    let json = with_store(|store| store.list().ok())
        .flatten()
        .and_then(|tasks| serde_json::to_string(&tasks).ok())
        .unwrap_or_else(|| JSON_EMPTY_ARRAY.to_string());
    let cstring =
        CString::new(json).unwrap_or_else(|_| CString::new(JSON_EMPTY_ARRAY).expect("valid"));
    store_json_allocation(cstring)
}

/// Replaces the full task set with the JSON array of `TodoTask` in `json`.
/// Returns true on success.
pub(crate) fn look_todo_save_json_impl(json: *const c_char) -> bool {
    let raw = cstr_to_string(json);
    let tasks: Vec<TodoTask> = match serde_json::from_str(&raw) {
        Ok(tasks) => tasks,
        Err(_) => return false,
    };
    with_store(|store| store.save(&tasks).is_ok()).unwrap_or(false)
}
