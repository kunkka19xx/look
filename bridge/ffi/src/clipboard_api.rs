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
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};

const JSON_EMPTY_ARRAY: &str = "[]";

/// One connection, opened once. `record` runs for every copy made anywhere in
/// the OS, and `SqliteStore::open` is not cheap: it creates the directory and
/// re-runs the full migration (a PRAGMA batch including a WAL journal-mode
/// switch, ~10 CREATE IF NOT EXISTS, two PRAGMA table_info) to perform one
/// INSERT. The shell already serializes these calls through one actor, so a
/// mutex-guarded connection matches the access pattern exactly.
/// Keyed on the path, because `set_db_path_for_test` can repoint the database
/// at runtime - a connection cached without the key would keep writing to the
/// previous file. Comparing a path per call is free next to a migration.
static CLIPBOARD_STORE: OnceLock<Mutex<Option<(PathBuf, SqliteStore)>>> = OnceLock::new();

fn store() -> MutexGuard<'static, Option<(PathBuf, SqliteStore)>> {
    let path = default_db_path();
    let cell = CLIPBOARD_STORE.get_or_init(|| Mutex::new(None));
    let mut guard = cell.lock().unwrap_or_else(|e| e.into_inner());
    let matches_path = guard.as_ref().is_some_and(|(cached, _)| *cached == path);
    if !matches_path {
        // A failed open is retried on the next call rather than cached.
        *guard = SqliteStore::open(&path).ok().map(|store| (path, store));
    }
    guard
}

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

/// Remembers a clip and returns its row id, or 0 on empty input or any store
/// failure. The id is what lets the shell delete THIS clip later; without it a
/// deleted clip returns on the next launch.
pub(crate) fn look_clipboard_record_impl(
    content: *const c_char,
    kind: *const c_char,
    app_bundle_id: *const c_char,
) -> i64 {
    let content = cstr_to_string(content);
    if content.trim().is_empty() {
        return 0;
    }
    let kind = cstr_to_string(kind);
    let kind = if kind.is_empty() {
        "text".to_string()
    } else {
        kind
    };
    let app = cstr_to_string(app_bundle_id);
    let app = if app.is_empty() { None } else { Some(app) };
    let guard = store();
    let Some((_, store)) = guard.as_ref() else {
        return 0;
    };
    store
        .record_clipboard_entry(&content, &kind, app.as_deref())
        .ok()
        .flatten()
        .unwrap_or(0)
}

/// JSON array of up to `limit` clips matching `query` (newest first), or `[]`.
pub(crate) fn look_clipboard_list_json_impl(query: *const c_char, limit: u32) -> *mut c_char {
    let query = cstr_to_string(query);
    let json = store()
        .as_ref()
        .and_then(|(_, store)| store.clipboard_entries(&query, limit as usize).ok())
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
    store()
        .as_ref()
        .and_then(|(_, store)| store.delete_clipboard_entry(id).ok())
        .unwrap_or(false)
}

/// Forgets every clip, returning how many were removed. The promise that makes
/// persisting clipboard history acceptable in the first place.
pub(crate) fn look_clipboard_clear_impl() -> u32 {
    store()
        .as_ref()
        .and_then(|(_, store)| store.clear_clipboard_entries().ok())
        .unwrap_or(0) as u32
}
