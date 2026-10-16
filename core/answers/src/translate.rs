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
// MyMemory rejects a `q` longer than 500 characters (code points) with QUERY
// LENGTH LIMIT EXCEEDED, so longer input is split and the pieces reassembled.
// Its spec page says "Max 500 bytes", but this was checked live: a request with
// 500 three-byte characters (1500 bytes) still translated, so the API counts
// code points. Keep this in characters, not bytes.
const MAX_QUERY_CHARS: usize = 500;
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

    // MyMemory caps a single query at 500 characters, so longer input is split
    // into chunks that are translated separately and reassembled in order, keeping
    // the original whitespace that sat between them. A failure in any chunk
    // fails the whole translation rather than returning a partial result.
    let mut translated = String::new();
    for chunk in split_query(&text) {
        let payload = chunk.trim();
        if payload.is_empty() {
            translated.push_str(chunk);
            continue;
        }
        let lead = chunk.len() - chunk.trim_start().len();
        let trail = chunk.len() - chunk.trim_end().len();
        match fetch_translation(payload, target_lang) {
            Ok(part) => {
                translated.push_str(&chunk[..lead]);
                translated.push_str(&part);
                if trail > 0 {
                    translated.push_str(&chunk[chunk.len() - trail..]);
                }
            }
            Err(error) => return Translation::failed(text, error),
        }
    }
    Translation {
        original: text,
        translated,
        error: None,
    }
}

/// Sends a single request (already within the character limit) and returns the
/// translated text, or the error to report. The same-language refusal echoes the
/// input back as a success.
fn fetch_translation(text: &str, target_lang: &str) -> Result<String, TranslateError> {
    let url = format!(
        "{URL_PREFIX}{}{URL_MIDDLE}{}",
        target_lang.trim(),
        http::encode(text)
    );
    let Some(response) = http::get(&url, TIMEOUT_SECS, USER_AGENT, &[]) else {
        return Err(TranslateError::RequestFailed);
    };
    // A throttled host answers 429; MyMemory also keeps HTTP 200 while flagging
    // a spent daily quota in the body, which the parse below catches.
    if response.is_rate_limited() {
        return Err(TranslateError::RateLimited);
    }
    if !response.is_success() {
        return Err(TranslateError::RequestFailed);
    }
    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&response.body) else {
        return Err(TranslateError::ParseFailed);
    };
    // A spent quota can arrive together with the same-language detail, so the
    // quota check runs first: a rate limit must never read as a successful echo.
    if quota_finished(&parsed) {
        return Err(TranslateError::RateLimited);
    }
    // MyMemory refuses to translate a language into itself. The input already
    // reads in the target language, so echoing it is the honest result rather
    // than a failure for a section that is working as intended.
    if is_same_language(&parsed) {
        return Ok(text.to_string());
    }
    if let Some(error) = body_error(&parsed) {
        return Err(error);
    }
    let translated = extract_translation(&parsed);
    if translated.trim().is_empty() {
        return Err(TranslateError::EmptyResult);
    }
    Ok(translated)
}

/// Splits `text` into chunks of at most `MAX_QUERY_CHARS` characters, preferring
/// to cut at the last whitespace inside the window so a word is not torn apart.
/// Concatenating the returned chunks reproduces `text` exactly, so the caller
/// can keep the separators when reassembling.
fn split_query(text: &str) -> Vec<&str> {
    if text.chars().count() <= MAX_QUERY_CHARS {
        return vec![text];
    }
    let mut chunks = Vec::new();
    let mut rest = text;
    while rest.chars().count() > MAX_QUERY_CHARS {
        let end = rest
            .char_indices()
            .nth(MAX_QUERY_CHARS)
            .map_or(rest.len(), |(i, _)| i);
        let window = &rest[..end];
        let cut = match window.rfind(char::is_whitespace) {
            Some(pos) if pos > 0 => {
                let ws_len = window[pos..].chars().next().map_or(0, char::len_utf8);
                pos + ws_len
            }
            _ => end,
        };
        chunks.push(&rest[..cut]);
        rest = &rest[cut..];
    }
    if !rest.is_empty() {
        chunks.push(rest);
    }
    chunks
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
/// "PLEASE SELECT TWO DISTINCT LANGUAGES"; the detail is ignored on other
/// statuses (e.g. 429) so a throttled response is not echoed as a success.
fn is_same_language(value: &serde_json::Value) -> bool {
    const DETAIL: &str = "PLEASE SELECT TWO DISTINCT LANGUAGES";
    let matches_detail = value
        .get("responseDetails")
        .and_then(|v| v.as_str())
        .map(|detail| detail.contains(DETAIL))
        .unwrap_or(false);
    matches_detail && matches!(response_status(value), None | Some(200) | Some(403))
}

/// True when MyMemory flagged the daily quota as spent in the response body.
fn quota_finished(value: &serde_json::Value) -> bool {
    value
        .get("quotaFinished")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// Classifies MyMemory's in-body status: `None` when the body looks usable,
/// otherwise the error to report. The API keeps HTTP 200 while flagging a spent
/// quota or a rejected request in the body, so the transport status alone would
/// read as success.
fn body_error(value: &serde_json::Value) -> Option<TranslateError> {
    if quota_finished(value) {
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

        // The same detail on a throttled status must not be echoed as success.
        let throttled = serde_json::json!({
            "responseData": {"translatedText": "PLEASE SELECT TWO DISTINCT LANGUAGES"},
            "responseStatus": 429,
            "responseDetails": "PLEASE SELECT TWO DISTINCT LANGUAGES",
        });
        assert!(!is_same_language(&throttled));
    }

    #[test]
    fn detects_finished_quota() {
        assert!(quota_finished(
            &serde_json::json!({ "quotaFinished": true })
        ));
        assert!(!quota_finished(
            &serde_json::json!({ "quotaFinished": false })
        ));
        assert!(!quota_finished(
            &serde_json::json!({ "quotaFinished": null })
        ));
    }

    #[test]
    fn keeps_short_queries_whole() {
        let text = "hello world";
        assert_eq!(split_query(text), vec![text]);
    }

    #[test]
    fn splits_long_queries_within_the_char_limit() {
        let text = "từ ".repeat(400);
        assert!(text.chars().count() > MAX_QUERY_CHARS);
        let chunks = split_query(&text);
        assert!(chunks.len() > 1);
        for chunk in &chunks {
            assert!(chunk.chars().count() <= MAX_QUERY_CHARS);
        }
        assert_eq!(chunks.concat(), text);
    }

    #[test]
    fn splits_without_whitespace_on_char_boundaries() {
        // A run of 3-byte characters and no whitespace, longer than the limit:
        // the split still has to land on valid character boundaries.
        let text = "日本語".repeat(300);
        let chunks = split_query(&text);
        for chunk in &chunks {
            assert!(chunk.chars().count() <= MAX_QUERY_CHARS);
            assert!(!chunk.is_empty());
        }
        assert_eq!(chunks.concat(), text);
    }

    #[test]
    fn splits_sample_vietnamese_text() {
        let text = "Look là một công cụ tìm kiếm và khởi chạy siêu nhanh trên máy tính của bạn. Nó cho phép bạn tìm kiếm ứng dụng, tệp tin, cửa sổ đang mở và nhiều thứ khác chỉ bằng vài thao tác gõ phím đơn giản. Giao diện gọn gàng, phản hồi tức thì, và hoạt động hoàn toàn cục bộ nên dữ liệu của bạn luôn được bảo mật. Bạn có thể tùy chỉnh danh sách ngôn ngữ dịch trong phần cấu hình. Tính năng dịch tự động phát hiện ngôn ngữ nguồn và gửi truy vấn đến máy chủ. Kết quả trả về được hiển thị song song cho từng ngôn ngữ đích đã chọn.";
        assert!(text.chars().count() > MAX_QUERY_CHARS);
        let chunks = split_query(text);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].chars().count(), 500);
        assert_eq!(chunks[1].chars().count(), 13);
        assert!(chunks.iter().all(|c| c.chars().count() <= MAX_QUERY_CHARS));
        assert_eq!(chunks.concat(), text);
    }
}
