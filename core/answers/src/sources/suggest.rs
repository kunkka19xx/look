//! Search autocomplete suggestions via DuckDuckGo's `ac/` endpoint, which
//! returns clean UTF-8 `[query, [suggestions]]` (unlike Google's `complete`,
//! which serves ISO-8859-1 and breaks on non-ASCII input). Pressing Enter still
//! runs a web *search*; only the completions come from here.

use crate::http;
use std::collections::HashSet;

const TIMEOUT_SECS: u32 = 4;

/// Up to `limit` autocomplete suggestions for `query`, with the echoed query and
/// duplicates removed. Empty for sub-2-char queries or on any failure.
pub fn web_suggestions(query: &str, limit: usize) -> Vec<String> {
    let trimmed = query.trim();
    if trimmed.chars().count() < 2 {
        return Vec::new();
    }

    let url = format!(
        "https://duckduckgo.com/ac/?q={}&type=list",
        http::encode(trimmed)
    );
    let Some(json) = http::get_json(&url, TIMEOUT_SECS) else {
        return Vec::new();
    };
    let Some(list) = json
        .as_array()
        .and_then(|arr| arr.get(1))
        .and_then(|v| v.as_array())
    else {
        return Vec::new();
    };

    let lower_query = trimmed.to_lowercase();
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for item in list {
        let Some(text) = item.as_str().map(str::trim) else {
            continue;
        };
        let lower = text.to_lowercase();
        if text.is_empty() || lower == lower_query || !seen.insert(lower) {
            continue;
        }
        result.push(text.to_string());
        if result.len() >= limit {
            break;
        }
    }
    result
}
