//! C-ABI wrappers over `look_storage`'s clipboard history, so every shell
//! remembers clips in the same look.db instead of holding them in memory and
//! losing them on quit. Direct-store access, mirroring `url_history_api`; all
//! endpoints are best-effort and panic-safe at `lib.rs`.
//!
//! The concealed/transient gate is NOT here and cannot be: only the shell can
//! read the pasteboard type markers that say a clip is a password or a
//! one-time secret, so the shell must refuse to call `record` for those. This
//! layer trusts what it is handed.

use crate::state::{cstr_to_string, default_db_path, store_json_allocation};
use look_storage::{ClipboardEntry, SqliteStore};
use serde::Serialize;
use std::ffi::CString;
use std::os::raw::c_char;

const JSON_EMPTY_ARRAY: &str = "[]";

#[derive(Serialize)]
struct ClipboardEntryJSON {
    id: i64,
    content: String,
    kind: String,
    #[serde(rename = "appBundleID", skip_serializing_if = "Option::is_none")]
    app_bundle_id: Option<String>,
    #[serde(rename = "copiedAtUnixS")]
    copied_at_unix_s: i64,
}

impl From<ClipboardEntry> for ClipboardEntryJSON {
    fn from(entry: ClipboardEntry) -> Self {
        Self {
            id: entry.id,
            content: entry.content,
            kind: entry.kind,
            app_bundle_id: entry.app_bundle_id,
            copied_at_unix_s: entry.copied_at_unix_s,
        }
    }
}

/// Remembers a clip. False on empty input or any store failure; capture is
/// fire-and-forget and must never block a copy.
pub(crate) fn look_clipboard_record_impl(
    content: *const c_char,
    kind: *const c_char,
    app_bundle_id: *const c_char,
) -> bool {
    let content = cstr_to_string(content);
    if content.trim().is_empty() {
        return false;
    }
    let kind = cstr_to_string(kind);
    let kind = if kind.is_empty() {
        "text".to_string()
    } else {
        kind
    };
    let app = cstr_to_string(app_bundle_id);
    let app = if app.is_empty() { None } else { Some(app) };
    let Ok(store) = SqliteStore::open(default_db_path()) else {
        return false;
    };
    store
        .record_clipboard_entry(&content, &kind, app.as_deref())
        .is_ok()
}

/// JSON array of up to `limit` clips matching `query` (newest first), or `[]`.
pub(crate) fn look_clipboard_list_json_impl(query: *const c_char, limit: u32) -> *mut c_char {
    let query = cstr_to_string(query);
    let json = SqliteStore::open(default_db_path())
        .ok()
        .and_then(|store| store.clipboard_entries(&query, limit as usize).ok())
        .map(|entries| {
            entries
                .into_iter()
                .map(ClipboardEntryJSON::from)
                .collect::<Vec<_>>()
        })
        .and_then(|entries| serde_json::to_string(&entries).ok())
        .unwrap_or_else(|| JSON_EMPTY_ARRAY.to_string());
    let cstring =
        CString::new(json).unwrap_or_else(|_| CString::new(JSON_EMPTY_ARRAY).expect("valid"));
    store_json_allocation(cstring)
}

pub(crate) fn look_clipboard_delete_impl(id: i64) -> bool {
    SqliteStore::open(default_db_path())
        .ok()
        .and_then(|store| store.delete_clipboard_entry(id).ok())
        .unwrap_or(false)
}

/// Forgets every clip, returning how many were removed. The promise that makes
/// persisting clipboard history acceptable in the first place.
pub(crate) fn look_clipboard_clear_impl() -> u32 {
    SqliteStore::open(default_db_path())
        .ok()
        .and_then(|store| store.clear_clipboard_entries().ok())
        .unwrap_or(0) as u32
}
