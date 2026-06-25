//! Shared, platform-agnostic "web answer" features for Look.
//!
//! These are the network-backed lookups that are identical on every OS -
//! instant answers (currency/weather/crypto) today, with search suggestions and
//! knowledge sources to follow. Each platform shell (macOS via the C FFI bridge,
//! Linux/Windows via Tauri commands) calls into here so the logic lives once.
//!
//! Design rules that keep this crate shareable:
//! - **No async runtime.** HTTP is a blocking `curl` subprocess (see [`http`]),
//!   matching the existing `translate_api` approach, so no `tokio`/`reqwest`.
//! - **Best-effort.** Every entry point returns "no answer" on any failure and
//!   never panics, so a caller can fire it speculatively while typing.
//! - **Pure pattern-gating.** [`has_match`] decides cheaply (no network) whether
//!   a query is even a candidate, so callers don't fan out wasted requests.

mod fmt;
mod http;
mod json;
mod parse;
mod sources;
mod translate;
mod types;

pub use translate::{TranslateError, Translation, translate};
pub use types::Answer;

/// Resolve a one-shot instant answer (currency, weather, or crypto) for `query`,
/// or `None` if the query matches no provider or every source fails. Performs
/// network I/O; call off the UI thread.
pub fn instant_answer(query: &str) -> Option<Answer> {
    sources::instant(query)
}

/// Whether `query` matches any instant-answer provider's shape. Cheap and
/// network-free - use it to gate whether an answer is worth fetching.
pub fn has_match(query: &str) -> bool {
    sources::has_match(query)
}

/// Up to `limit` search autocomplete suggestions for `query` (empty on failure
/// or for sub-2-char queries). Performs network I/O.
pub fn web_suggestions(query: &str, limit: usize) -> Vec<String> {
    sources::suggest::web_suggestions(query, limit)
}

/// DuckDuckGo instant answer for `query`, or `None`. Performs network I/O.
pub fn duckduckgo_answer(query: &str) -> Option<Answer> {
    sources::knowledge::duckduckgo(query)
}

/// Wikipedia summary for an already-chosen `search_term`, or `None`. Performs
/// network I/O.
pub fn wikipedia_answer(search_term: &str) -> Option<Answer> {
    sources::knowledge::wikipedia(search_term)
}

/// Entity from a definitional query ("what is vim" -> "vim"), or `None`.
/// Network-free heuristic used to decide whether to consult Wikipedia.
pub fn definitional_entity(query: &str) -> Option<String> {
    sources::knowledge::definitional_entity(query)
}
