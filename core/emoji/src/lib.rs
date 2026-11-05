//! Emoji search shared by macOS (via `bridge/ffi`) and linows (via Tauri), so
//! both platforms return identical results. Pure: no I/O, no network.

use serde::Serialize;

mod data;

/// One dataset entry. `keywords` are extra search terms beyond the name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Emoji {
    pub glyph: &'static str,
    pub name: &'static str,
    pub keywords: &'static [&'static str],
}

/// What the shells receive: just enough to render and copy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EmojiHit {
    pub glyph: String,
    pub name: String,
}

/// Case-insensitive match over name and keywords, ranked name-prefix, then
/// name-substring, then keyword. An empty query yields no hits.
pub fn search(query: &str, limit: usize) -> Vec<EmojiHit> {
    search_in(data::EMOJI, query, limit)
}

/// `search` over an explicit dataset, so ranking is testable without the
/// bundled data. Ties keep dataset order.
pub fn search_in(data: &[Emoji], query: &str, limit: usize) -> Vec<EmojiHit> {
    let query = query.trim().to_lowercase();
    if query.is_empty() || limit == 0 {
        return Vec::new();
    }
    let mut ranked: Vec<(u8, &Emoji)> = data
        .iter()
        .filter_map(|e| rank(e, &query).map(|r| (r, e)))
        .collect();
    ranked.sort_by_key(|(r, _)| *r);
    ranked
        .into_iter()
        .take(limit)
        .map(|(_, e)| hit(e))
        .collect()
}

/// Resolves a persisted glyph (e.g. a recent) back to its hit. Ignores the
/// emoji variation selector so `❤` and `❤️` are the same emoji.
pub fn lookup(glyph: &str) -> Option<EmojiHit> {
    let wanted = strip_variation(glyph);
    data::EMOJI
        .iter()
        .find(|e| strip_variation(e.glyph) == wanted)
        .map(hit)
}

fn rank(e: &Emoji, query: &str) -> Option<u8> {
    let name = e.name.to_lowercase();
    if name.starts_with(query) {
        Some(0)
    } else if name.contains(query) {
        Some(1)
    } else if e.keywords.iter().any(|k| k.to_lowercase().contains(query)) {
        Some(2)
    } else {
        None
    }
}

fn hit(e: &Emoji) -> EmojiHit {
    EmojiHit {
        glyph: e.glyph.to_string(),
        name: e.name.to_string(),
    }
}

fn strip_variation(s: &str) -> String {
    s.chars().filter(|&c| c != '\u{fe0f}').collect()
}
