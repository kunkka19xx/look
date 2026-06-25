//! Currency conversion. Frankfurter (ECB) is precise but only covers ~30 major
//! currencies, so pairs like USD->VND return nothing; the open ER-API fallback
//! spans ~160 currencies. Frankfurter stays first so common pairs keep using
//! official ECB reference rates.

use crate::{fmt, http, json::ValueExt, parse::CurrencyQuery, types::Answer};

const TIMEOUT_SECS: u32 = 4;

pub fn answer(q: &CurrencyQuery) -> Option<Answer> {
    let value = frankfurter_value(q).or_else(|| er_api_value(q))?;
    let text = format!(
        "{} {} = {} {}",
        fmt::format_number(q.amount),
        q.from,
        fmt::format_number(value),
        q.to,
    );
    Some(Answer::text(text, "Currency"))
}

/// ECB reference rates. Frankfurter multiplies by `amount` server-side, so the
/// returned rate is already the final value.
fn frankfurter_value(q: &CurrencyQuery) -> Option<f64> {
    let url = format!(
        "https://api.frankfurter.dev/v1/latest?amount={}&base={}&symbols={}",
        fmt::query_amount(q.amount),
        q.from,
        q.to,
    );
    let json = http::get_json(&url, TIMEOUT_SECS)?;
    json.get("rates")?.get_f64(&q.to)
}

/// Open ER-API: ~160 currencies, keyless. Returns a per-unit rate, so the amount
/// is applied client-side.
fn er_api_value(q: &CurrencyQuery) -> Option<f64> {
    let url = format!("https://open.er-api.com/v6/latest/{}", q.from);
    let json = http::get_json(&url, TIMEOUT_SECS)?;
    if json.get_str("result")? != "success" {
        return None;
    }
    let rate = json.get("rates")?.get_f64(&q.to)?;
    Some(rate * q.amount)
}
