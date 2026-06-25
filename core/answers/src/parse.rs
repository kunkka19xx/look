//! Synchronous shape detectors for each instant-answer provider. A query must
//! match one of these *before* any network call, so unrelated typing never hits
//! the wire. Patterns mirror the macOS `InstantAnswerSources` regexes.

use regex::Regex;
use std::sync::LazyLock;

/// A parsed currency conversion request, e.g. `1 usd -> vnd`.
#[derive(Debug, Clone, PartialEq)]
pub struct CurrencyQuery {
    pub amount: f64,
    pub from: String,
    pub to: String,
}

/// Parses `<amount?> <FROM> (to|in|->|=) <TO>` with 3-letter codes. Amount
/// defaults to 1 (and 0 is treated as 1). Codes are upper-cased.
pub fn currency(query: &str) -> Option<CurrencyQuery> {
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^([0-9]+(?:[.,][0-9]+)?)?\s*([a-z]{3})\s+(?:to|in|->|=)\s+([a-z]{3})$")
            .expect("valid currency regex")
    });
    let caps = RE.captures(query.trim())?;
    let amount = caps
        .get(1)
        .map(|m| m.as_str().replace(',', ".").parse::<f64>().unwrap_or(1.0))
        .unwrap_or(1.0);
    Some(CurrencyQuery {
        amount: if amount == 0.0 { 1.0 } else { amount },
        from: caps[2].to_uppercase(),
        to: caps[3].to_uppercase(),
    })
}

/// Parses `weather [in|at|for] <place>` and returns the place name.
pub fn weather(query: &str) -> Option<String> {
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^weather(?:\s+(?:in|at|for))?\s+(.+)$").expect("valid weather regex")
    });
    let place = RE.captures(query.trim())?[1].trim().to_string();
    (!place.is_empty()).then_some(place)
}

/// Parses `<coin> price` or `price of <coin>` and returns a CoinGecko id
/// (common ticker aliases mapped, spaces hyphenated).
pub fn crypto(query: &str) -> Option<String> {
    static TRAILING: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)^(.+?)\s+price$").expect("valid crypto regex"));
    static LEADING: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)^price\s+of\s+(.+)$").expect("valid crypto regex"));

    let q = query.trim();
    let name = TRAILING
        .captures(q)
        .or_else(|| LEADING.captures(q))
        .map(|c| c[1].to_string())?;
    let name = name.to_lowercase();
    let name = name.trim();
    if name.is_empty() {
        return None;
    }

    let mapped = match name {
        "btc" => "bitcoin",
        "eth" => "ethereum",
        "sol" => "solana",
        "doge" => "dogecoin",
        "ada" => "cardano",
        "xrp" => "ripple",
        "bnb" => "binancecoin",
        "ltc" => "litecoin",
        other => other,
    };
    Some(mapped.replace(' ', "-"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn currency_variants() {
        let q = currency("1 usd -> vnd").unwrap();
        assert_eq!(
            (q.amount, q.from.as_str(), q.to.as_str()),
            (1.0, "USD", "VND")
        );
        assert_eq!(currency("50 EUR to JPY").unwrap().amount, 50.0);
        assert_eq!(currency("usd in gbp").unwrap().amount, 1.0); // default
        assert!(currency("hello world").is_none());
    }

    #[test]
    fn weather_place() {
        assert_eq!(weather("weather in Hanoi").unwrap(), "Hanoi");
        assert_eq!(weather("weather Tokyo").unwrap(), "Tokyo");
        assert!(weather("weather").is_none());
    }

    #[test]
    fn crypto_aliases() {
        assert_eq!(crypto("btc price").unwrap(), "bitcoin");
        assert_eq!(crypto("price of solana").unwrap(), "solana");
        assert!(crypto("solana").is_none());
    }
}
