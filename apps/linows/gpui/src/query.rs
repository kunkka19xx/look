//! One query in, the rows out. Plain search is the engine, the pinned
//! folders, the inline calculator and the URL rows, in the order the
//! webview's `publish` puts them; a prefixed query asks one history or the
//! process table instead. Blocks on SQLite and `/proc`, so it runs off the UI
//! thread.

use std::sync::OnceLock;

use linows_backend::{calc, clipboard, files, process, search, weburl};

use crate::modes::{self, Mode};
use crate::rows::{self, Row};
use crate::{actions, blocks, state};

/// The engine's cap; the list scrolls past the first screenful.
const SEARCH_LIMIT: u32 = 40;
const RECENT_URL_LIMIT: u32 = 5;

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

/// `refresh_processes`: walk `/proc` again rather than score the snapshot,
/// on entering `ps"` and after a kill.
pub fn run(query: &str, refresh_processes: bool) -> Vec<Row> {
    // Only asked where the empty query lists the index: the engine's ranking
    // of everything, as the webview's `performSearch('')`.
    if query.trim().is_empty() {
        return search::search(state(), "", SEARCH_LIMIT)
            .results
            .into_iter()
            .map(Row::from_engine)
            .map(dress)
            .collect();
    }
    let (mode, term) = Mode::of(query);
    match mode {
        Mode::PrefixMenu => modes::prefix_rows(term),
        Mode::CommandMenu => modes::command_rows(term),
        // Translation runs on Enter, not per keystroke.
        Mode::Translate => Vec::new(),
        Mode::Clipboard => clipboard::get_clipboard_history(term)
            .into_iter()
            .enumerate()
            .map(|(i, entry)| Row::clip(entry, i))
            .collect(),
        Mode::ClipboardImage => {
            let needle = term.trim().to_lowercase();
            clipboard::get_clipboard_images()
                .into_iter()
                .map(|row| Row::clip_image(row.entry, row.thumb_path))
                .filter(|row| {
                    needle.is_empty()
                        || format!("{} {}", row.title, row.context)
                            .to_lowercase()
                            .contains(&needle)
                })
                .collect()
        }
        Mode::Process => process::search_processes(term.trim(), refresh_processes)
            .into_iter()
            .map(Row::process)
            .collect(),
        Mode::Recent => search::search(state(), query, SEARCH_LIMIT)
            .results
            .into_iter()
            .map(Row::from_engine)
            .collect(),
        Mode::Search => plain(query),
    }
}

/// A block's row says which block, and wears what it declared.
fn dress(mut row: Row) -> Row {
    if let Some(block) = actions::block_id_of(&row.id).map(str::to_string) {
        let catalog = blocks::get();
        row.dress_source(
            catalog.name(&block),
            catalog.icon(&block),
            catalog.home.as_deref(),
        );
    }
    row
}

fn plain(query: &str) -> Vec<Row> {
    let local: Vec<Row> = search::search(state(), query, SEARCH_LIMIT)
        .results
        .into_iter()
        .map(Row::from_engine)
        .map(dress)
        .collect();
    let mut rows = rows::prepend_quick_folders(local, query, quick_folders());
    // The engine's own scopes (`a"`, `f"`, `d"`, `r"`) asked for one kind of
    // thing: no URL or calc rows.
    if query.trim_start().contains('"') {
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
