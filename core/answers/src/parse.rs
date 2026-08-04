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

/// Currency signs people actually type, and the code each stands for. `$` is
/// ambiguous in principle (CAD, AUD, ...) but means USD to anyone typing it
/// into a launcher without qualification.
const CURRENCY_SIGNS: &[(&str, &str)] = &[
    ("$", "USD"),
    ("€", "EUR"),
    ("£", "GBP"),
    ("¥", "JPY"),
    ("₫", "VND"),
    ("đ", "VND"),
    ("₩", "KRW"),
    ("₹", "INR"),
    ("₽", "RUB"),
    ("₴", "UAH"),
    ("฿", "THB"),
];

fn currency_code(token: &str) -> String {
    CURRENCY_SIGNS
        .iter()
        .find(|(sign, _)| *sign == token)
        .map(|(_, code)| (*code).to_string())
        .unwrap_or_else(|| token.to_uppercase())
}

/// Parses `<amount?> <FROM> (to|in|->|=>|=|>) <TO>`, where either side is a
/// 3-letter code or a currency sign, and the amount may sit on either side of
/// its sign (`20$` or `$20`). Amount defaults to 1 (and 0 is treated as 1).
///
/// Symbol operators need no whitespace, because nobody types `20 usd -> jpy`
/// when they're in a hurry - `20usd->jpy` is the same request. Word operators
/// still need it, so `usdtojpy` stays a word rather than a conversion.
pub fn currency(query: &str) -> Option<CurrencyQuery> {
    const NUM: &str = r"[0-9]+(?:[.,][0-9]+)?";
    const CUR: &str = r"(?:[a-z]{3}|[$€£¥₫đ₩₹₽₴฿])";
    // Groups: 1 amount-before, 2 unit-after, 3 unit-before, 4 amount-after, 5 target.
    static SYMBOL: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(&format!(
            r"(?i)^(?:({NUM})?\s*({CUR})|({CUR})\s*({NUM}))\s*(?:->|=>|→|=|>)\s*({CUR})$"
        ))
        .expect("valid currency regex")
    });
    static WORD: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(&format!(
            r"(?i)^(?:({NUM})?\s*({CUR})|({CUR})\s*({NUM}))\s+(?:to|in)\s+({CUR})$"
        ))
        .expect("valid currency regex")
    });
    let q = query.trim();
    let caps = SYMBOL.captures(q).or_else(|| WORD.captures(q))?;
    let amount = caps
        .get(1)
        .or_else(|| caps.get(4))
        .map(|m| m.as_str().replace(',', ".").parse::<f64>().unwrap_or(1.0))
        .unwrap_or(1.0);
    let from = caps.get(2).or_else(|| caps.get(3))?.as_str();
    Some(CurrencyQuery {
        amount: if amount == 0.0 { 1.0 } else { amount },
        from: currency_code(from),
        to: currency_code(&caps[5]),
    })
}

/// Parses `weather [in|at|for] <place>` and returns the place name, or an empty
/// string for a bare `weather` - "the place I asked about last". The provider
/// decides whether it has one to offer.
pub fn weather(query: &str) -> Option<String> {
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^weather(?:\s+(?:in|at|for))?(?:\s+(.+))?$").expect("valid weather regex")
    });
    let caps = RE.captures(query.trim())?;
    Some(
        caps.get(1)
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_default(),
    )
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

    /// Spacing is a typing habit, not a different request.
    #[test]
    fn currency_ignores_spacing_around_symbols() {
        for q in [
            "20usd->jpy",
            "20 usd->jpy",
            "20usd -> jpy",
            "20 usd -> jpy",
            "20usd=>jpy",
            "20usd>jpy",
            "20usd=jpy",
            "20usd→jpy",
        ] {
            let parsed = currency(q).unwrap_or_else(|| panic!("{q} should parse"));
            assert_eq!(
                (parsed.amount, parsed.from.as_str(), parsed.to.as_str()),
                (20.0, "USD", "JPY"),
                "{q}"
            );
        }
        assert_eq!(currency("usd->jpy").unwrap().amount, 1.0);
    }

    /// Word operators still need their spaces, or every six-letter word would
    /// look like a conversion.
    #[test]
    fn currency_word_operators_need_spacing() {
        assert!(currency("usdtojpy").is_none());
        assert!(currency("20usdtojpy").is_none());
        assert!(currency("usd to jpy").is_some());
    }

    /// People type the sign, not the ISO code, and put it on whichever side
    /// their keyboard habits favour.
    #[test]
    fn currency_accepts_signs_on_either_side_of_the_amount() {
        for q in [
            "20$ to jpy",
            "$20 to jpy",
            "20$->jpy",
            "$20->jpy",
            "20 $ to jpy",
        ] {
            let parsed = currency(q).unwrap_or_else(|| panic!("{q} should parse"));
            assert_eq!(
                (parsed.amount, parsed.from.as_str(), parsed.to.as_str()),
                (20.0, "USD", "JPY"),
                "{q}"
            );
        }
    }

    #[test]
    fn currency_signs_map_to_codes_on_both_sides() {
        assert_eq!(currency("100€ to ¥").unwrap().from, "EUR");
        assert_eq!(currency("100€ to ¥").unwrap().to, "JPY");
        assert_eq!(currency("50 usd to ₫").unwrap().to, "VND");
        assert_eq!(currency("£5->usd").unwrap().from, "GBP");
        assert_eq!(currency("₹99 in usd").unwrap().amount, 99.0);
    }

    #[test]
    fn weather_place() {
        assert_eq!(weather("weather in Hanoi").unwrap(), "Hanoi");
        assert_eq!(weather("weather Tokyo").unwrap(), "Tokyo");
        assert_eq!(weather("weather at Osaka").unwrap(), "Osaka");
        assert!(weather("weathering heights").is_none());
        assert!(weather("the weather").is_none());
    }

    /// A bare `weather` parses to "no place given"; whether that can be
    /// answered is the provider's call, not the pattern's.
    #[test]
    fn weather_alone_leaves_the_place_open() {
        assert_eq!(weather("weather").unwrap(), "");
        assert_eq!(weather("  weather  ").unwrap(), "");
    }

    #[test]
    fn crypto_aliases() {
        assert_eq!(crypto("btc price").unwrap(), "bitcoin");
        assert_eq!(crypto("price of solana").unwrap(), "solana");
        assert!(crypto("solana").is_none());
    }
}
