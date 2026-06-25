//! Instant-answer providers. Each is pattern-gated in [`crate::parse`] and only
//! hits the network once its shape matches, so adding providers never fans out
//! wasted requests.

mod crypto;
mod currency;
pub mod knowledge;
pub mod suggest;
mod weather;

use crate::{parse, types::Answer};

/// Runs the first provider whose pattern matches `query`. Providers are mutually
/// exclusive in practice (a currency query is never also a weather query), so we
/// resolve in order and return the first hit.
pub fn instant(query: &str) -> Option<Answer> {
    let q = query.trim();
    if let Some(c) = parse::currency(q) {
        return currency::answer(&c);
    }
    if let Some(place) = parse::weather(q) {
        return weather::answer(&place);
    }
    if let Some(id) = parse::crypto(q) {
        return crypto::answer(&id);
    }
    None
}

/// Whether any provider's pattern matches - network-free gate.
pub fn has_match(query: &str) -> bool {
    let q = query.trim();
    parse::currency(q).is_some() || parse::weather(q).is_some() || parse::crypto(q).is_some()
}
