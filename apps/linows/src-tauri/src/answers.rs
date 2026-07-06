//! Tauri commands over `look_answers`. The look-answers crate ships blocking
//! HTTP (via curl subprocess) and no async runtime, so anything that touches
//! the network runs on the Tauri blocking pool to keep the UI thread free.
//!
//! Mirrors `bridge/ffi/src/answers_api.rs` on macOS - the wire shape is the
//! same `Answer` struct from look-answers, serialised straight to JSON.

use look_answers::Answer;
use tauri::async_runtime;

/// Network-free pattern gate. Used to decide whether `instant_answer` is even
/// worth firing (matches currency/weather/crypto grammar without hitting the
/// network), and as part of the AI-answer trigger heuristic on the JS side.
#[tauri::command]
pub fn instant_has_match(query: String) -> bool {
    look_answers::has_match(&query)
}

/// Definitional entity extractor - e.g. `"what is vim"` → `Some("vim")`. Used
/// by the JS controller to pick the Wikipedia search term. Cheap (regex only),
/// so it stays on the calling thread.
#[tauri::command]
pub fn definitional_entity(query: String) -> Option<String> {
    look_answers::definitional_entity(&query)
}

/// Instant answer for currency / weather / crypto grammar. Returns `None` when
/// the query doesn't match a provider or the network call fails.
#[tauri::command]
pub async fn instant_answer(query: String) -> Option<Answer> {
    async_runtime::spawn_blocking(move || look_answers::instant_answer(&query))
        .await
        .ok()
        .flatten()
}

/// DuckDuckGo Instant Answer API. Used for "what is X" lookups that aren't
/// covered by Wikipedia, and for short factoids.
#[tauri::command]
pub async fn duckduckgo_answer(query: String) -> Option<Answer> {
    async_runtime::spawn_blocking(move || look_answers::duckduckgo_answer(&query))
        .await
        .ok()
        .flatten()
}

/// Wikipedia REST API summary. `term` should be the extracted entity, not the
/// raw query - the JS controller calls `definitional_entity` first.
#[tauri::command]
pub async fn wikipedia_answer(term: String) -> Option<Answer> {
    async_runtime::spawn_blocking(move || look_answers::wikipedia_answer(&term))
        .await
        .ok()
        .flatten()
}

/// Google autocomplete (via DuckDuckGo's `/ac/` endpoint). Returns up to
/// `limit` suggestions; empty vec on failure or short queries.
#[tauri::command]
pub async fn web_suggestions(query: String, limit: usize) -> Vec<String> {
    async_runtime::spawn_blocking(move || look_answers::web_suggestions(&query, limit))
        .await
        .unwrap_or_default()
}
