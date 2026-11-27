//! URL-like queries (issue #232) and the launcher-opened URL history
//! (url-history spec). Classification lives in `look_answers`, storage in
//! `look_storage`, frecency ranking in `look_engine::url_history` - all shared
//! with macOS, so this file is only the glue. The store calls do SQLite I/O,
//! so the shell runs them on its blocking pool.

use crate::state::default_db_path;
use look_answers::UrlMatch;
use look_engine::url_history::{ScoredUrlEntry, ranked_url_history};
use look_storage::{SqliteStore, resolve_web_search};

pub const WEB_SEARCH_ENGINE_KEY: &str = "web_search_engine";

/// URL for a web search of `query` on the configured engine or template
/// (Google when the core refuses `value`), or `None` for an empty query.
pub fn search_url(value: &str, query: &str) -> Option<String> {
    let query = query.trim();
    (!query.is_empty()).then(|| resolve_web_search(value).build_url(query))
}

/// Classify `query` as a URL to offer as an "Open <url>" row, or `None` to
/// leave it as a search term. Pure and network-free, stays on the calling
/// thread.
pub fn classify_url(query: &str) -> Option<UrlMatch> {
    look_answers::classify_url(query)
}

/// Records that `url` was opened. Best-effort: returns false on empty input
/// or any store failure; the frontend fires and forgets.
pub fn record_url_hit(url: &str) -> bool {
    if url.trim().is_empty() {
        return false;
    }
    let Ok(store) = SqliteStore::open(default_db_path()) else {
        return false;
    };
    store.record_url_hit(url).is_ok()
}

/// Up to `limit` remembered URLs matching `query`, in frecency order. Empty
/// vec on any failure - history rows are a convenience, never an error.
pub fn recent_urls(query: &str, limit: u32) -> Vec<ScoredUrlEntry> {
    SqliteStore::open(default_db_path())
        .ok()
        .and_then(|store| store.recent_urls(query, limit as usize).ok())
        .map(|entries| ranked_url_history(entries, query))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_url_follows_the_setting_and_skips_empty_queries() {
        assert_eq!(
            search_url("https://search.yahoo.com/search?p=%s", " rust & tea ").as_deref(),
            Some("https://search.yahoo.com/search?p=rust%20%26%20tea")
        );
        assert_eq!(
            search_url("yahoo", "rust").as_deref(),
            Some("https://www.google.com/search?q=rust")
        );
        assert_eq!(search_url("bing", "   "), None);
    }
}
