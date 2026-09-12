//! C-ABI wrappers over `look_storage`'s clipboard history. Mirrors
//! `url_history_api`; panic-safe at `lib.rs`.
//!
//! The concealed/transient gate is NOT here and cannot be: only the shell sees
//! the pasteboard markers. This layer trusts what it is handed.

use crate::state::{cstr_to_string, default_db_path, store_json_allocation};
use look_storage::{CLIPBOARD_KIND_TEXT, ClipboardEntry, SqliteStore};
use serde::Serialize;
use std::collections::HashSet;
use std::ffi::CString;
use std::os::raw::c_char;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};

const JSON_EMPTY_ARRAY: &str = "[]";
/// Beside the database, not in it: an image clip is megabytes of pixels, and
/// SQLite is the wrong place to keep them.
const IMAGE_DIR_NAME: &str = "clipboard-images";

/// One connection, opened once: `record` runs on every copy made anywhere in
/// the OS, and `SqliteStore::open` re-runs the whole migration each time.
/// Keyed on the path: `set_db_path_for_test` can repoint the database, and an
/// unkeyed cache would keep writing to the old file.
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
    #[serde(rename = "contentHash")]
    content_hash: String,
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
            content_hash: entry.content_hash,
            app_bundle_id: entry.app_bundle_id,
            copied_at_unix_s: entry.copied_at_unix_s,
        }
    }
}

/// Where image clips keep their bytes, created on demand. The shell writes the
/// files here and rebuilds each path from the row's hash, so this is the one
/// place the location is decided.
fn image_dir() -> PathBuf {
    let dir = default_db_path()
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join(IMAGE_DIR_NAME);
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Deletes every image file the table no longer refers to.
///
/// Unlinking at each deletion site instead would leave megabytes behind
/// whenever a prune, a crash, or a failed write got in between. Sweeping
/// against the rows is the same work and self-heals, and the directory holds
/// tens of files, so the listing is cheap.
///
/// The one rule the shell has to honour: a file here is named starting with the
/// hash of the row that owns it. Everything after that (extension, a thumbnail
/// suffix) is the shell's business, so the two sides cannot drift over a
/// filename format.
fn sweep_orphan_images() {
    let guard = store();
    let Some((_, store)) = guard.as_ref() else {
        return;
    };
    let Ok(live) = store.clipboard_image_hashes() else {
        return;
    };
    let live: HashSet<String> = live.into_iter().collect();
    drop(guard);

    let Ok(entries) = std::fs::read_dir(image_dir()) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !live.iter().any(|hash| name.starts_with(hash.as_str())) {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// Remembers a clip, returning its row id (0 on failure). The id is what lets
/// the shell delete this clip later.
pub(crate) fn look_clipboard_record_impl(
    content: *const c_char,
    app_bundle_id: *const c_char,
) -> i64 {
    let content = cstr_to_string(content);
    if content.trim().is_empty() {
        return 0;
    }
    let app = optional_string(app_bundle_id);
    let guard = store();
    let Some((_, store)) = guard.as_ref() else {
        return 0;
    };
    store
        .record_clipboard_entry(&content, app.as_deref())
        .ok()
        .flatten()
        .unwrap_or(0)
}

/// Remembers a copied image, returning its row id (0 on failure). `label` is
/// what the row is listed and searched by; `image_hash` names the file the
/// shell already wrote under `look_clipboard_images_dir`.
pub(crate) fn look_clipboard_record_image_impl(
    label: *const c_char,
    image_hash: *const c_char,
    app_bundle_id: *const c_char,
) -> i64 {
    let label = cstr_to_string(label);
    let image_hash = cstr_to_string(image_hash);
    if label.trim().is_empty() || image_hash.trim().is_empty() {
        return 0;
    }
    let app = optional_string(app_bundle_id);
    let row_id = {
        let guard = store();
        let Some((_, store)) = guard.as_ref() else {
            return 0;
        };
        store
            .record_clipboard_image(&label, &image_hash, app.as_deref())
            .ok()
            .flatten()
            .unwrap_or(0)
    };
    // A record can prune, and a prune leaves bytes behind.
    sweep_orphan_images();
    row_id
}

/// The directory image clips are stored in. The shell writes `<hash>.png` there
/// before recording the row, and rebuilds the path from the hash when listing.
pub(crate) fn look_clipboard_images_dir_impl() -> *mut c_char {
    sweep_orphan_images();
    let path = image_dir().to_string_lossy().into_owned();
    let cstring = CString::new(path).unwrap_or_else(|_| CString::new("").expect("valid"));
    store_json_allocation(cstring)
}

/// JSON array of up to `limit` clips of `kind` matching `query` (newest first),
/// or `[]`. The kind is required: `c"` and `ci"` must not see each other's rows.
pub(crate) fn look_clipboard_list_json_impl(
    kind: *const c_char,
    query: *const c_char,
    limit: u32,
) -> *mut c_char {
    let kind = cstr_to_string(kind);
    let kind = if kind.is_empty() {
        CLIPBOARD_KIND_TEXT.to_string()
    } else {
        kind
    };
    let query = cstr_to_string(query);
    let json = store()
        .as_ref()
        .and_then(|(_, store)| store.clipboard_entries(&kind, &query, limit as usize).ok())
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
    let deleted = store()
        .as_ref()
        .and_then(|(_, store)| store.delete_clipboard_entry(id).ok())
        .unwrap_or(false);
    if deleted {
        sweep_orphan_images();
    }
    deleted
}

/// Forgets every clip, returning how many were removed.
pub(crate) fn look_clipboard_clear_impl() -> u32 {
    let removed = store()
        .as_ref()
        .and_then(|(_, store)| store.clear_clipboard_entries().ok())
        .unwrap_or(0) as u32;
    // The promise behind persisting history at all covers the pixels too.
    sweep_orphan_images();
    removed
}

fn optional_string(raw: *const c_char) -> Option<String> {
    let value = cstr_to_string(raw);
    if value.is_empty() { None } else { Some(value) }
}
