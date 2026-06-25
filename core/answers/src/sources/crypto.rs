//! Crypto spot price via CoinGecko (keyless). `id` is an already-normalized
//! CoinGecko coin id from [`crate::parse::crypto`].

use crate::{fmt, http, json::ValueExt, types::Answer};

const TIMEOUT_SECS: u32 = 4;

pub fn answer(id: &str) -> Option<Answer> {
    let url = format!(
        "https://api.coingecko.com/api/v3/simple/price?ids={}&vs_currencies=usd&include_24hr_change=true",
        http::encode(id),
    );
    let json = http::get_json(&url, TIMEOUT_SECS)?;
    let coin = json.get(id)?;
    let usd = coin.get_f64("usd")?;

    let mut text = format!("{}: ${}", capitalize_words(id), fmt::format_number(usd));
    if let Some(change) = coin.get_f64("usd_24h_change") {
        let sign = if change >= 0.0 { "+" } else { "" };
        text.push_str(&format!(" ({sign}{change:.2}% 24h)"));
    }
    let page = format!("https://www.coingecko.com/en/coins/{id}");
    Some(Answer::linked(text, "Crypto", Some(page), None))
}

/// Capitalizes each hyphen/space-separated word, e.g. `bitcoin` -> `Bitcoin`,
/// `binance-coin` -> `Binance-Coin`.
fn capitalize_words(id: &str) -> String {
    id.split_inclusive(['-', ' '])
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capitalizes() {
        assert_eq!(capitalize_words("bitcoin"), "Bitcoin");
        assert_eq!(capitalize_words("binance-coin"), "Binance-Coin");
    }
}
