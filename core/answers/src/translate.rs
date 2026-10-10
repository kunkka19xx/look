//! MyMemory translation lookup, shared by macOS (FFI bridge) and Linux/Windows
//! (Tauri command). Auto-detects the source language and translates to a target
//! BCP-47-ish code via MyMemory's keyless public API. Best-effort: every failure
//! path returns a `Translation` with `error` set rather than panicking.
//!
//! Each shell formats the result for its own wire shape (macOS surfaces a
//! `{code, message}` object; linows surfaces just the message), so the error
//! type exposes both.

use crate::http;

const URL_PREFIX: &str = "https://api.mymemory.translated.net/get?langpair=Autodetect%7C";
const URL_MIDDLE: &str = "&q=";
const TIMEOUT_SECS: u32 = 5;
// MyMemory does not inspect the User-Agent, but a browser string keeps it in
// line with the other sources and avoids any bot filtering.
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

/// Why a translation didn't produce text. `code` is a stable identifier; the
/// `message` is user-facing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranslateError {
    EmptyText,
    InvalidTargetLang,
    RequestFailed,
    RateLimited,
    ParseFailed,
    EmptyResult,
}

impl TranslateError {
    pub fn code(self) -> &'static str {
        match self {
            Self::EmptyText => "empty_text",
            Self::InvalidTargetLang => "invalid_target_lang",
            Self::RequestFailed => "translate_request_failed",
            Self::RateLimited => "translate_rate_limited",
            Self::ParseFailed => "translate_parse_failed",
            Self::EmptyResult => "translate_empty_result",
        }
    }

    pub fn message(self) -> &'static str {
        match self {
            Self::EmptyText => "Type text after t\" to translate",
            Self::InvalidTargetLang => "Invalid target language code",
            Self::RequestFailed => "Translation request failed",
            Self::RateLimited => "Translation is rate limited, try again later",
            Self::ParseFailed => "Translation response parse failed",
            Self::EmptyResult => "Translation returned empty result",
        }
    }
}

/// Outcome of a translation request. `error` is `None` on success.
pub struct Translation {
    pub original: String,
    pub translated: String,
    pub error: Option<TranslateError>,
}

impl Translation {
    fn failed(original: String, error: TranslateError) -> Self {
        Translation {
            original,
            translated: String::new(),
            error: Some(error),
        }
    }
}

/// Translates `text` into `target_lang`, auto-detecting the source.
pub fn translate(text: &str, target_lang: &str) -> Translation {
    let text = text.trim().to_string();
    if text.is_empty() {
        return Translation::failed(text, TranslateError::EmptyText);
    }
    if !is_valid_lang_code(target_lang) {
        return Translation::failed(text, TranslateError::InvalidTargetLang);
    }

    let url = format!(
        "{URL_PREFIX}{}{URL_MIDDLE}{}",
        target_lang.trim(),
        http::encode(&text)
    );
    let Some(response) = http::get(&url, TIMEOUT_SECS, USER_AGENT, &[]) else {
        return Translation::failed(text, TranslateError::RequestFailed);
    };
    // A throttled host answers 429; MyMemory also keeps HTTP 200 while flagging
    // a spent daily quota in the body, which the parse below catches.
    if response.is_rate_limited() {
        return Translation::failed(text, TranslateError::RateLimited);
    }
    if !response.is_success() {
        return Translation::failed(text, TranslateError::RequestFailed);
    }
    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&response.body) else {
        return Translation::failed(text, TranslateError::ParseFailed);
    };
    // MyMemory refuses to translate a language into itself. The input already
    // reads in the target language, so echoing it is the honest result rather
    // than a failure for a section that is working as intended.
    if is_same_language(&parsed) {
        return Translation {
            original: text.clone(),
            translated: text,
            error: None,
        };
    }
    if let Some(error) = body_error(&parsed) {
        return Translation::failed(text, error);
    }
    let translated = extract_translation(&parsed);
    if translated.trim().is_empty() {
        return Translation::failed(text, TranslateError::EmptyResult);
    }
    Translation {
        original: text,
        translated,
        error: None,
    }
}

/// A BCP-47-ish tag accepted by MyMemory (e.g. "en", "vi", "zh-CN").
fn is_valid_lang_code(code: &str) -> bool {
    let code = code.trim();
    !code.is_empty()
        && code.len() <= 10
        && code.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

/// Pulls `responseData.translatedText` out of the MyMemory response shape.
fn extract_translation(value: &serde_json::Value) -> String {
    value
        .get("responseData")
        .and_then(|data| data.get("translatedText"))
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

/// True when MyMemory refused because the detected source and the target are
/// the same language. It reports this as HTTP 200/403 with the detail
/// "PLEASE SELECT TWO DISTINCT LANGUAGES".
fn is_same_language(value: &serde_json::Value) -> bool {
    const DETAIL: &str = "PLEASE SELECT TWO DISTINCT LANGUAGES";
    value
        .get("responseDetails")
        .and_then(|v| v.as_str())
        .map(|detail| detail.contains(DETAIL))
        .unwrap_or(false)
}

/// Classifies MyMemory's in-body status: `None` when the body looks usable,
/// otherwise the error to report. The API keeps HTTP 200 while flagging a spent
/// quota or a rejected request in the body, so the transport status alone would
/// read as success.
fn body_error(value: &serde_json::Value) -> Option<TranslateError> {
    if value
        .get("quotaFinished")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        return Some(TranslateError::RateLimited);
    }
    match response_status(value) {
        Some(429) => Some(TranslateError::RateLimited),
        Some(200) | None => None,
        Some(_) => Some(TranslateError::RequestFailed),
    }
}

/// `responseStatus` as a number, whether MyMemory sent it as a JSON number or a
/// numeric string.
fn response_status(value: &serde_json::Value) -> Option<u16> {
    match value.get("responseStatus") {
        Some(serde_json::Value::Number(n)) => n.as_u64().and_then(|v| u16::try_from(v).ok()),
        Some(serde_json::Value::String(s)) => s.parse().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_and_bad_lang() {
        assert_eq!(translate("  ", "en").error, Some(TranslateError::EmptyText));
        assert_eq!(
            translate("hello", "toolonglang").error,
            Some(TranslateError::InvalidTargetLang)
        );
        assert_eq!(
            translate("hello", "e n").error,
            Some(TranslateError::InvalidTargetLang)
        );
    }

    #[test]
    fn reads_mymemory_body() {
        let ok = serde_json::json!({
            "responseData": {"translatedText": "Xin chào"},
            "responseStatus": 200,
            "quotaFinished": false,
        });
        assert_eq!(extract_translation(&ok), "Xin chào");
        assert_eq!(response_status(&ok), Some(200));
        assert_eq!(body_error(&ok), None);

        let spent = serde_json::json!({
            "responseData": {"translatedText": "MYMEMORY WARNING"},
            "responseStatus": 200,
            "quotaFinished": true,
        });
        assert_eq!(body_error(&spent), Some(TranslateError::RateLimited));

        let throttled = serde_json::json!({ "responseStatus": "429" });
        assert_eq!(body_error(&throttled), Some(TranslateError::RateLimited));

        let rejected = serde_json::json!({
            "responseData": {"translatedText": ""},
            "responseStatus": "403",
        });
        assert_eq!(body_error(&rejected), Some(TranslateError::RequestFailed));
    }

    #[test]
    fn detects_same_language_refusal() {
        let same = serde_json::json!({
            "responseData": {"translatedText": "PLEASE SELECT TWO DISTINCT LANGUAGES"},
            "responseStatus": 403,
            "responseDetails": "PLEASE SELECT TWO DISTINCT LANGUAGES",
        });
        assert!(is_same_language(&same));

        let other = serde_json::json!({
            "responseData": {"translatedText": "xin chào"},
            "responseStatus": 200,
        });
        assert!(!is_same_language(&other));
    }
}
