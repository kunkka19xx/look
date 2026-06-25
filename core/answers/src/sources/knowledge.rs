//! Encyclopedic "knowledge card" sources - DuckDuckGo instant answers and
//! Wikipedia summaries - plus the definitional-query heuristic that decides what
//! (if anything) to look up on Wikipedia. Coverage is deliberately narrow: facts
//! and definitions, which is what these free APIs do well. Each shell decides
//! how to orchestrate/merge these; here they're independent best-effort fetches.

use crate::{http, json::ValueExt, types::Answer};

const TIMEOUT_SECS: u32 = 4;

/// DuckDuckGo instant answer, or `None`. Prefers the direct `Answer` field, then
/// the `AbstractText`. Labelled "DuckDuckGo" so it reads distinctly from a direct
/// Wikipedia hit when both are shown.
pub fn duckduckgo(query: &str) -> Option<Answer> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return None;
    }
    let url = format!(
        "https://api.duckduckgo.com/?q={}&format=json&no_html=1&skip_disambig=1",
        http::encode(trimmed)
    );
    let json = http::get_json(&url, TIMEOUT_SECS)?;

    let answer = json.get_str("Answer").unwrap_or("").trim();
    let abstract_text = json.get_str("AbstractText").unwrap_or("").trim();
    let text = if answer.is_empty() {
        abstract_text
    } else {
        answer
    };
    if text.is_empty() {
        return None;
    }

    let url_field = json
        .get_str("AbstractURL")
        .filter(|s| !s.is_empty())
        .map(String::from);
    let image_url = ddg_image(json.get_str("Image"));
    Some(Answer::linked(text, "DuckDuckGo", url_field, image_url))
}

/// DuckDuckGo's `Image` is often a site-relative path ("/i/abc.png").
fn ddg_image(raw: Option<&str>) -> Option<String> {
    let raw = raw?;
    if raw.is_empty() {
        None
    } else if raw.starts_with("http") {
        Some(raw.to_string())
    } else {
        Some(format!("https://duckduckgo.com{raw}"))
    }
}

/// Wikipedia summary for an already-chosen `search_term` (the caller decides
/// whether a query warrants a lookup and what to search for). Resolves free text
/// to a real article title, then fetches its REST summary. Disambiguation pages
/// are rejected - they aren't real answers.
pub fn wikipedia(search_term: &str) -> Option<Answer> {
    let trimmed = search_term.trim();
    if trimmed.is_empty() {
        return None;
    }

    let opensearch = format!(
        "https://en.wikipedia.org/w/api.php?action=opensearch&format=json&limit=1&search={}",
        http::encode(trimmed)
    );
    let parsed = http::get_json(&opensearch, TIMEOUT_SECS)?;
    let arr = parsed.as_array()?;
    if arr.len() < 4 {
        return None;
    }
    let title = arr.get(1)?.as_array()?.first()?.as_str()?;
    let page_url = arr
        .get(3)
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|v| v.as_str())
        .map(String::from);

    let summary_url = format!(
        "https://en.wikipedia.org/api/rest_v1/page/summary/{}",
        http::encode(title)
    );
    let object = http::get_json(&summary_url, TIMEOUT_SECS)?;

    // Disambiguation pages ("Vim may refer to:") aren't real answers.
    if object.get_str("type") == Some("disambiguation") {
        return None;
    }
    let extract = object.get_str("extract")?.trim();
    if extract.is_empty() {
        return None;
    }

    let image_url = object
        .get("thumbnail")
        .and_then(|t| t.get_str("source"))
        .map(String::from);
    Some(Answer::linked(extract, "Wikipedia", page_url, image_url))
}

/// Extracts the entity from a definitional query ("what is vim" -> "vim"), or
/// `None` when the query isn't asking for a definition. The prefix match is
/// case-insensitive; the returned entity keeps its original casing.
pub fn definitional_entity(query: &str) -> Option<String> {
    const PREFIXES: [&str; 13] = [
        "what is ",
        "what are ",
        "what was ",
        "what's ",
        "whats ",
        "who is ",
        "who was ",
        "who are ",
        "who's ",
        "define ",
        "definition of ",
        "meaning of ",
        "tell me about ",
    ];
    let lower = query.to_lowercase();
    for prefix in PREFIXES {
        if lower.starts_with(prefix) {
            // Prefixes are ASCII, so the byte offset is a valid char boundary in
            // the original (case-preserving) query.
            let entity = query[prefix.len()..]
                .trim_matches([' ', '?', '.', '!'])
                .to_string();
            return (!entity.is_empty()).then_some(entity);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn definitional_prefixes() {
        assert_eq!(definitional_entity("what is Vim").as_deref(), Some("Vim"));
        assert_eq!(
            definitional_entity("who was Ada Lovelace?").as_deref(),
            Some("Ada Lovelace")
        );
        assert_eq!(
            definitional_entity("tell me about Rust").as_deref(),
            Some("Rust")
        );
        assert_eq!(definitional_entity("david beckham"), None);
        assert_eq!(definitional_entity("what is "), None);
    }
}
