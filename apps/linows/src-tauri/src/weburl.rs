//! Tauri commands for URL-like queries (issue #232) and the launcher-opened
//! URL history (url-history spec). Classification lives in `look_answers`,
//! storage in `look_storage`, frecency ranking in `look_engine::url_history` -
//! all shared with macOS, so this file is only the command layer. The store
//! calls do SQLite I/O and run on the blocking pool.

use crate::state::default_db_path;
use look_answers::UrlMatch;
use look_engine::url_history::{ScoredUrlEntry, ranked_url_history};
use look_storage::SqliteStore;
use tauri::async_runtime;

/// Classify `query` as a URL to offer as an "Open <url>" row, or `None` to
/// leave it as a search term. Pure and network-free, stays on the calling
/// thread.
#[tauri::command]
pub fn classify_url(query: String) -> Option<UrlMatch> {
    look_answers::classify_url(&query)
}

/// Records that `url` was opened. Best-effort: returns false on empty input
/// or any store failure; the frontend fires and forgets.
#[tauri::command]
pub async fn record_url_hit(url: String) -> bool {
    async_runtime::spawn_blocking(move || {
        if url.trim().is_empty() {
            return false;
        }
        let Ok(store) = SqliteStore::open(default_db_path()) else {
            return false;
        };
        store.record_url_hit(&url).is_ok()
    })
    .await
    .unwrap_or(false)
}

/// Up to `limit` remembered URLs matching `query`, in frecency order. Empty
/// vec on any failure - history rows are a convenience, never an error.
#[tauri::command]
pub async fn recent_urls(query: String, limit: u32) -> Vec<ScoredUrlEntry> {
    async_runtime::spawn_blocking(move || {
        SqliteStore::open(default_db_path())
            .ok()
            .and_then(|store| store.recent_urls(&query, limit as usize).ok())
            .map(|entries| ranked_url_history(entries, &query))
            .unwrap_or_default()
    })
    .await
    .unwrap_or_default()
}
