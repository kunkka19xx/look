//! URL-like queries (issue #232) and the launcher-opened URL history
//! (url-history spec). Classification lives in `look_answers`, storage in
//! `look_storage`, frecency ranking in `look_engine::url_history` - all shared
//! with macOS, so this file is only the glue. The store calls do SQLite I/O,
//! so the shell runs them on its blocking pool.

use crate::state::default_db_path;
use look_answers::UrlMatch;
use look_engine::url_history::{ScoredUrlEntry, ranked_url_history};
use look_storage::{SqliteStore, parse_web_search, resolve_web_search};

/// URL for a web search of `query` on the configured engine/template, or
/// `None` for an empty query. A `web_search_engine` the core refuses searches
/// on Google; Settings marks it red so the user can fix it.
pub fn search_url(value: &str, query: &str) -> Option<String> {
    let query = query.trim();
    if query.is_empty() {
        return None;
    }
    Some(resolve_web_search(value).build_url(query))
}

/// Whether Settings should mark `value` red.
pub fn web_search_is_refused(value: &str) -> bool {
    parse_web_search(value).is_err()
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
    fn search_url_uses_the_selected_engine() {
        assert_eq!(
            search_url("google", "rust").as_deref(),
            Some("https://www.google.com/search?q=rust")
        );
        assert_eq!(
            search_url("duckduckgo", "rust").as_deref(),
            Some("https://duckduckgo.com/?q=rust")
        );
        assert_eq!(
            search_url("bing", "rust").as_deref(),
            Some("https://www.bing.com/search?q=rust")
        );
    }

    #[test]
    fn search_url_uses_a_custom_template() {
        assert_eq!(
            search_url("https://search.yahoo.com/search?p={encoded}", "rust & tea").as_deref(),
            Some("https://search.yahoo.com/search?p=rust+%26+tea")
        );
    }

    #[test]
    fn search_url_falls_back_to_google_for_a_refused_value() {
        assert_eq!(
            search_url("yahoo", "rust").as_deref(),
            Some("https://www.google.com/search?q=rust")
        );
    }

    #[test]
    fn search_url_skips_empty_queries() {
        assert_eq!(search_url("bing", "   "), None);
        assert_eq!(search_url("yahoo", "   "), None);
    }

    #[test]
    fn refused_values_are_flagged_for_settings() {
        assert!(web_search_is_refused("yahoo"));
        assert!(web_search_is_refused("https://example.com/"));
        assert!(!web_search_is_refused("bing"));
        assert!(!web_search_is_refused(""));
        assert!(!web_search_is_refused("https://x.com/?q={encoded}"));
    }
}
