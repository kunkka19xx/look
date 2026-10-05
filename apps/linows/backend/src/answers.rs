//! The launcher's surface over `look_answers`. The look-answers crate ships
//! blocking HTTP (via curl subprocess) and no async runtime, so the shell runs
//! anything that touches the network on its blocking pool.
//!
//! Mirrors `bridge/ffi/src/answers_api.rs` on macOS - the wire shape is the
//! same `Answer` struct from look-answers, serialised straight to JSON.

use look_answers::Answer;

/// Network-free pattern gate. Used to decide whether `instant_answer` is even
/// worth firing (matches currency/weather/crypto grammar without hitting the
/// network), and as part of the AI-answer trigger heuristic on the JS side.
pub fn instant_has_match(query: &str) -> bool {
    look_answers::has_match(query)
}

/// Definitional entity extractor - e.g. `"what is vim"` → `Some("vim")`. Used
/// by the JS controller to pick the Wikipedia search term. Cheap (regex only),
/// so it stays on the calling thread.
pub fn definitional_entity(query: &str) -> Option<String> {
    look_answers::definitional_entity(query)
}

/// Instant answer for currency / weather / crypto grammar. Returns `None` when
/// the query doesn't match a provider or the network call fails.
pub fn instant_answer(query: &str) -> Option<Answer> {
    look_answers::instant_answer(query)
}

/// DuckDuckGo Instant Answer API. Used for "what is X" lookups that aren't
/// covered by Wikipedia, and for short factoids.
pub fn duckduckgo_answer(query: &str) -> Option<Answer> {
    look_answers::duckduckgo_answer(query)
}

/// Wikipedia REST API summary. `term` should be the extracted entity, not the
/// raw query - the JS controller calls `definitional_entity` first.
pub fn wikipedia_answer(term: &str) -> Option<Answer> {
    look_answers::wikipedia_answer(term)
}

/// Google autocomplete (via DuckDuckGo's `/ac/` endpoint). Returns up to
/// `limit` suggestions; empty vec on failure or short queries.
pub fn web_suggestions(query: &str, limit: usize) -> Vec<String> {
    look_answers::web_suggestions(query, limit)
}
