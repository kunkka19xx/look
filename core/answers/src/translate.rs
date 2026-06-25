//! Google Translate lookup, shared by macOS (FFI bridge) and Linux/Windows
//! (Tauri command). Auto-detects the source language and translates to a target
//! BCP-47-ish code via Google's keyless `gtx` endpoint. Best-effort: every
//! failure path returns a `Translation` with `error` set rather than panicking.
//!
//! Each shell formats the result for its own wire shape (macOS surfaces a
//! `{code, message}` object; linows surfaces just the message), so the error
//! type exposes both.

use crate::http;

const URL_PREFIX: &str =
    "https://translate.googleapis.com/translate_a/single?client=gtx&sl=auto&tl=";
const URL_MIDDLE: &str = "&dt=t&q=";
const TIMEOUT_SECS: u32 = 3;
// Google rejects an empty/odd User-Agent; present as a browser like the
// originals did. Accept-Language keeps responses ASCII-stable.
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
const ACCEPT_LANGUAGE: &str = "Accept-Language: en-US,en;q=0.9";

/// Why a translation didn't produce text. `code` is a stable identifier; the
/// `message` is user-facing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranslateError {
    EmptyText,
    InvalidTargetLang,
    RequestFailed,
    ParseFailed,
    EmptyResult,
}

impl TranslateError {
    pub fn code(self) -> &'static str {
        match self {
            Self::EmptyText => "empty_text",
            Self::InvalidTargetLang => "invalid_target_lang",
            Self::RequestFailed => "translate_request_failed",
            Self::ParseFailed => "translate_parse_failed",
            Self::EmptyResult => "translate_empty_result",
        }
    }

    pub fn message(self) -> &'static str {
        match self {
            Self::EmptyText => "Type text after t\" to translate",
            Self::InvalidTargetLang => "Invalid target language code",
            Self::RequestFailed => "Translation request failed",
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
    let Some(body) = http::get(&url, TIMEOUT_SECS, USER_AGENT, &[ACCEPT_LANGUAGE]) else {
        return Translation::failed(text, TranslateError::RequestFailed);
    };
    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&body) else {
        return Translation::failed(text, TranslateError::ParseFailed);
    };
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

/// A BCP-47-ish tag accepted by Google Translate (e.g. "en", "vi", "zh-CN").
fn is_valid_lang_code(code: &str) -> bool {
    let code = code.trim();
    !code.is_empty()
        && code.len() <= 10
        && code.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

/// Pulls the concatenated translated segments out of the gtx response shape
/// `[[[translated, original, ...], ...], ...]`.
fn extract_translation(value: &serde_json::Value) -> String {
    let Some(segments) = value
        .as_array()
        .and_then(|a| a.first())
        .and_then(|v| v.as_array())
    else {
        return String::new();
    };
    let mut result = String::new();
    for group in segments {
        if let Some(s) = group
            .as_array()
            .and_then(|parts| parts.first())
            .and_then(|v| v.as_str())
        {
            result.push_str(s);
        }
    }
    result
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
}
