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

/// `$` is ambiguous in principle (CAD, AUD) but means USD to anyone typing it
/// into a launcher unqualified.
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
    // The patterns are case-insensitive, so a sign arrives in the user's case
    // and `Đ` has to find `đ`.
    let lowered = token.to_lowercase();
    CURRENCY_SIGNS
        .iter()
        .find(|(sign, _)| *sign == lowered)
        .map(|(_, code)| (*code).to_string())
        .unwrap_or_else(|| token.to_uppercase())
}

/// `1,000` is a thousands separator and `1,5` a decimal comma, the same rule
/// the calculator reads numbers by. Getting this wrong is worse than not
/// parsing: `1,000 usd to jpy` would silently convert 1 dollar.
fn parse_amount(raw: &str) -> f64 {
    static GROUPED: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^[0-9]{1,3}(?:,[0-9]{3})+(?:\.[0-9]+)?$").expect("valid amount regex")
    });
    let normalized = if GROUPED.is_match(raw) {
        raw.replace(',', "")
    } else {
        raw.replace(',', ".")
    };
    normalized.parse::<f64>().unwrap_or(1.0)
}

/// Parses `<amount?> <FROM> (to|in|->|=>|=|>) <TO>`: either side may be a
/// 3-letter code or a sign, and the amount either side of its sign (`20$`,
/// `$20`). Amount defaults to 1. Symbol operators need no whitespace, since
/// `20usd->jpy` is the same request; word operators do, so `usdtojpy` stays a
/// word.
pub fn currency(query: &str) -> Option<CurrencyQuery> {
    const NUM: &str = r"(?:[0-9]{1,3}(?:,[0-9]{3})+(?:\.[0-9]+)?|[0-9]+(?:[.,][0-9]+)?)";
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
        .map(|m| parse_amount(m.as_str()))
        .unwrap_or(1.0);
    let from = caps.get(2).or_else(|| caps.get(3))?.as_str();
    Some(CurrencyQuery {
        amount: if amount == 0.0 { 1.0 } else { amount },
        from: currency_code(from),
        to: currency_code(&caps[5]),
    })
}

/// Place name, or empty for a bare `weather` - the provider decides whether it
/// has a last place to offer.
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

    /// A dropped thousands separator converts the wrong amount and says
    /// nothing about it, so it has to survive the parse.
    #[test]
    fn currency_amounts_keep_their_thousands_separators() {
        assert_eq!(currency("1,000 usd to jpy").unwrap().amount, 1000.0);
        assert_eq!(
            currency("1,234,567 usd to jpy").unwrap().amount,
            1_234_567.0
        );
        assert_eq!(currency("1,000.50 usd to jpy").unwrap().amount, 1000.5);
        // Not a group of three: a decimal comma, as European keyboards write it.
        assert_eq!(currency("1,5 usd to jpy").unwrap().amount, 1.5);
    }

    #[test]
    fn currency_signs_are_case_insensitive() {
        assert_eq!(currency("Đ20 to usd").unwrap().from, "VND");
        assert_eq!(currency("20đ to usd").unwrap().from, "VND");
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

    /// Shapes neither the fix nor the review covered.
    #[test]
    fn currency_amount_edge_cases() {
        // Group of three that is the whole number.
        assert_eq!(currency("100 usd to jpy").unwrap().amount, 100.0);
        // Leading group shorter than three.
        assert_eq!(currency("12,345 usd to jpy").unwrap().amount, 12345.0);
        // Four digits with no separator stay themselves.
        assert_eq!(currency("1000 usd to jpy").unwrap().amount, 1000.0);
        // Decimal point, no grouping.
        assert_eq!(currency("0.5 usd to jpy").unwrap().amount, 0.5);
        // Not groups of three, so they read as decimal commas.
        assert_eq!(currency("1,00 usd to jpy").unwrap().amount, 1.0);
        assert_eq!(currency("1,0000 usd to jpy").unwrap().amount, 1.0);
        // Amount attached to a sign, grouped.
        assert_eq!(currency("$1,500->jpy").unwrap().amount, 1500.0);
        assert_eq!(currency("1,500$->jpy").unwrap().amount, 1500.0);
    }

    #[test]
    fn crypto_aliases() {
        assert_eq!(crypto("btc price").unwrap(), "bitcoin");
        assert_eq!(crypto("price of solana").unwrap(), "solana");
        assert!(crypto("solana").is_none());
    }
}
