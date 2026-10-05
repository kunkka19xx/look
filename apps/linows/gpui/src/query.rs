//! One query in, the rows out: the engine, the pinned folders, the inline
//! calculator, and the URL rows, in the order the webview's `publish` puts
//! them. Blocks on SQLite for the URL history, so it runs off the UI thread.

use std::sync::OnceLock;

use linows_backend::{calc, files, search, weburl};

use crate::rows::{self, Row};
use crate::state;

/// The engine's cap; the list scrolls past the first screenful.
const SEARCH_LIMIT: u32 = 40;
const RECENT_URL_LIMIT: u32 = 5;

/// The query prefixes the launcher reads for itself or the engine scopes on.
/// A prefixed query gets no URL or calc rows: it asked for one kind of thing.
const PREFIXES: [&str; 9] = [
    "a\"", "f\"", "d\"", "rc\"", "r\"", "ps\"", "c\"", "ci\"", "t\"",
];

pub fn is_prefixed(query: &str) -> bool {
    let trimmed = query.trim_start();
    !trimmed.is_empty()
        && (trimmed.starts_with('"')
            || trimmed.starts_with(':')
            || PREFIXES.iter().any(|p| trimmed.starts_with(p)))
}

/// The home folders the search pins, read once: they do not move while the
/// launcher runs.
fn quick_folders() -> &'static [(String, String)] {
    static FOLDERS: OnceLock<Vec<(String, String)>> = OnceLock::new();
    FOLDERS.get_or_init(|| {
        files::get_quick_folders()
            .into_iter()
            .map(|f| (f.title, f.path))
            .collect()
    })
}

pub fn run(query: &str) -> Vec<Row> {
    if query.trim().is_empty() {
        return Vec::new();
    }
    let local: Vec<Row> = search::search(state(), query, SEARCH_LIMIT)
        .results
        .into_iter()
        .map(Row::from_engine)
        .collect();
    let mut rows = rows::prepend_quick_folders(local, query, quick_folders());
    if is_prefixed(query) {
        return rows;
    }

    let trimmed = query.trim();
    let live = weburl::classify_url(query);
    let recents: Vec<Row> = weburl::recent_urls(trimmed, RECENT_URL_LIMIT)
        .into_iter()
        .filter(|e| live.as_ref().is_none_or(|m| m.url != e.url))
        .map(|e| Row::url(&e.url, rows::WEB_URL_RECENT_SUBTITLE, e.score))
        .collect();
    rows = rows::merge_by_score(rows, recents);
    if let Some(m) = live {
        rows = rows::place_url_row(
            Row::url(&m.url, rows::WEB_URL_OPEN_SUBTITLE, 0),
            m.tier,
            rows,
        );
    }
    // Above everything and takes the selection: an expression is a question,
    // and the answer outranks a file that fuzzy-matched some of its digits.
    if let Some(answer) = calc::calc_inline(query) {
        rows.insert(0, Row::calc(trimmed, answer));
    }
    rows
}
