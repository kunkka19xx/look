//! Translation lookup, shared by macOS (FFI bridge) and Linux/Windows (gpui).
//! Auto-detects the source language and translates to a
//! target BCP-47-ish code. Two keyless providers, tried in order: Google's
//! `gtx` endpoint, which is fast and has no daily cap but throttles bursts per
//! IP, then MyMemory, which is slower and capped at 5000 characters a day per
//! IP but answers when Google is throttled. Successful results are cached for
//! the session so a repeated query costs neither a round trip nor quota.
//!
//! Best-effort: every failure path returns a `Translation` with `error` set
//! rather than panicking. Each shell formats the result for its own wire shape
//! (macOS surfaces a `{code, message}` object; linows surfaces just the
//! message), so the error type exposes both.

use crate::http;
use std::collections::HashMap;
use std::sync::Mutex;

const GOOGLE_URL_PREFIX: &str =
    "https://translate.googleapis.com/translate_a/single?client=gtx&sl=auto&tl=";
const GOOGLE_URL_MIDDLE: &str = "&dt=t&q=";
// Google answers in 300-500ms when it answers at all, so this is headroom
// rather than a budget: past it the request is not slow, it is not coming, and
// every extra second here is a second before MyMemory is even asked.
const GOOGLE_TIMEOUT_SECS: u32 = 2;
// Google rejects an empty/odd User-Agent; present as a browser like the
// originals did. Accept-Language keeps responses ASCII-stable.
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
const ACCEPT_LANGUAGE: &str = "Accept-Language: en-US,en;q=0.9";

const MYMEMORY_URL_PREFIX: &str = "https://api.mymemory.translated.net/get?langpair=Autodetect%7C";
const MYMEMORY_URL_MIDDLE: &str = "&q=";
// A 500-character query regularly takes 2-4s on MyMemory and sometimes more than
// 5s, which failed one language in a parallel set with "request failed".
const MYMEMORY_TIMEOUT_SECS: u32 = 10;
// MyMemory rejects a `q` longer than 500 characters (code points) with QUERY
// LENGTH LIMIT EXCEEDED, so longer input is split and the pieces reassembled.
// Its spec page says "Max 500 bytes", but this was checked live: a request with
// 500 three-byte characters (1500 bytes) still translated, so the API counts
// code points. Keep this in characters, not bytes.
const MAX_QUERY_CHARS: usize = 500;
/// MyMemory's free allowance: characters a day, per IP, shared across every
/// target language.
const MYMEMORY_DAILY_CHARS: usize = 5000;
/// The longest text the MyMemory fallback accepts, the daily allowance split
/// across the target cap. Without it one long paste spends the whole day, so
/// the next query of any length fails. Google has no daily cap and is tried
/// first, so this only ever turns away the fallback.
const MYMEMORY_MAX_TEXT_CHARS: usize = MYMEMORY_DAILY_CHARS / crate::languages::MAX_TARGETS;
/// Where a chunk prefers to end, so the machine translation keeps a whole
/// sentence of context: these followed by whitespace, or a line break.
const SENTENCE_ENDS: [char; 6] = ['.', '!', '?', '。', '！', '？'];

/// Session cache size. Once full it is cleared rather than evicted: a launcher
/// rarely sees this many distinct queries, so LRU bookkeeping buys nothing.
const CACHE_LIMIT: usize = 256;
static CACHE: Mutex<Option<HashMap<(String, String), String>>> = Mutex::new(None);

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
    TextTooLong,
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
            Self::TextTooLong => "translate_text_too_long",
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
            Self::TextTooLong => "Text too long to translate, try a shorter selection",
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

/// A translation backend. Tried in `PROVIDERS` order; any failure moves to the
/// next one, and the last failure is the one reported.
#[derive(Debug, Clone, Copy)]
enum Provider {
    Google,
    MyMemory,
}

const PROVIDERS: [Provider; 2] = [Provider::Google, Provider::MyMemory];

/// Translates `text` into `target_lang`, auto-detecting the source.
pub fn translate(text: &str, target_lang: &str) -> Translation {
    let text = text.trim().to_string();
    if text.is_empty() {
        return Translation::failed(text, TranslateError::EmptyText);
    }
    let target_lang = target_lang.trim();
    if !is_valid_lang_code(target_lang) {
        return Translation::failed(text, TranslateError::InvalidTargetLang);
    }

    let key = (target_lang.to_string(), text.clone());
    if let Some(translated) = cache_get(&key) {
        return Translation {
            original: text,
            translated,
            error: None,
        };
    }

    let mut error = TranslateError::RequestFailed;
    for provider in PROVIDERS {
        match provider.translate(&text, target_lang) {
            Ok(translated) => {
                cache_put(key, translated.clone());
                return Translation {
                    original: text,
                    translated,
                    error: None,
                };
            }
            Err(e) => error = e,
        }
    }
    Translation::failed(text, error)
}

fn cache_get(key: &(String, String)) -> Option<String> {
    let slot = CACHE.lock().unwrap_or_else(|p| p.into_inner());
    slot.as_ref().and_then(|map| map.get(key).cloned())
}

fn cache_put(key: (String, String), translated: String) {
    let mut slot = CACHE.lock().unwrap_or_else(|p| p.into_inner());
    let map = slot.get_or_insert_with(HashMap::new);
    if map.len() >= CACHE_LIMIT {
        map.clear();
    }
    map.insert(key, translated);
}

impl Provider {
    fn translate(self, text: &str, target_lang: &str) -> Result<String, TranslateError> {
        match self {
            Self::Google => google(text, target_lang),
            Self::MyMemory => mymemory(text, target_lang),
        }
    }
}

/// One request to Google's `gtx` endpoint, which takes the whole text.
fn google(text: &str, target_lang: &str) -> Result<String, TranslateError> {
    let url = format!(
        "{GOOGLE_URL_PREFIX}{target_lang}{GOOGLE_URL_MIDDLE}{}",
        http::encode(text)
    );
    let Some(response) = http::get(&url, GOOGLE_TIMEOUT_SECS, USER_AGENT, &[ACCEPT_LANGUAGE])
    else {
        return Err(TranslateError::RequestFailed);
    };
    // Google answers a throttled caller with an HTML apology page, which would
    // otherwise read as an unparseable response from a healthy service.
    if response.is_rate_limited() {
        return Err(TranslateError::RateLimited);
    }
    if !response.is_success() {
        return Err(TranslateError::RequestFailed);
    }
    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&response.body) else {
        return Err(TranslateError::ParseFailed);
    };
    non_empty(google_text(&parsed))
}

/// MyMemory caps a single query at 500 characters, so longer input is split
/// into chunks that are translated separately and reassembled in order, keeping
/// the original whitespace that sat between them. A failure in any chunk fails
/// the whole translation rather than returning a partial result. Text past
/// `MYMEMORY_MAX_TEXT_CHARS` is turned away before any request, because the
/// chunks would spend a disproportionate share of the daily allowance.
fn mymemory(text: &str, target_lang: &str) -> Result<String, TranslateError> {
    if text.chars().count() > MYMEMORY_MAX_TEXT_CHARS {
        return Err(TranslateError::TextTooLong);
    }
    let mut translated = String::new();
    for chunk in split_query(text) {
        let payload = chunk.trim();
        if payload.is_empty() {
            translated.push_str(chunk);
            continue;
        }
        let lead = chunk.len() - chunk.trim_start().len();
        let trail = chunk.len() - chunk.trim_end().len();
        let part = mymemory_request(payload, target_lang)?;
        translated.push_str(&chunk[..lead]);
        translated.push_str(&part);
        if trail > 0 {
            translated.push_str(&chunk[chunk.len() - trail..]);
        }
    }
    Ok(translated)
}

/// Sends a single MyMemory request (already within the character limit). The
/// same-language refusal echoes the input back as a success.
fn mymemory_request(text: &str, target_lang: &str) -> Result<String, TranslateError> {
    let url = format!(
        "{MYMEMORY_URL_PREFIX}{target_lang}{MYMEMORY_URL_MIDDLE}{}",
        http::encode(text)
    );
    let Some(response) = http::get(&url, MYMEMORY_TIMEOUT_SECS, USER_AGENT, &[]) else {
        return Err(TranslateError::RequestFailed);
    };
    // A throttled host answers 429; MyMemory also keeps HTTP 200 while flagging
    // a spent daily quota in the body, which the checks below catch.
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
    if let Some(error) = status_error(&parsed) {
        return Err(error);
    }
    non_empty(mymemory_text(&parsed))
}

fn non_empty(translated: String) -> Result<String, TranslateError> {
    if translated.trim().is_empty() {
        return Err(TranslateError::EmptyResult);
    }
    Ok(translated)
}

/// Splits `text` into chunks of at most `MAX_QUERY_CHARS` characters. Each cut
/// lands after the last sentence end inside the window, failing that after the
/// last whitespace, failing that at the limit, so a word is never torn apart
/// and a sentence only when it is longer than the window. Concatenating the
/// returned chunks reproduces `text` exactly, so the caller can keep the
/// separators when reassembling.
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
        let cut = cut_point(&rest[..end]).unwrap_or(end);
        chunks.push(&rest[..cut]);
        rest = &rest[cut..];
    }
    if !rest.is_empty() {
        chunks.push(rest);
    }
    chunks
}

/// The byte offset just past the best whitespace to cut `window` at: the last
/// one that closes a sentence if there is one, otherwise the last one at all.
/// `None` when the window has no whitespace past its first character.
fn cut_point(window: &str) -> Option<usize> {
    let mut sentence = None;
    let mut whitespace = None;
    let mut prev = None;
    for (i, c) in window.char_indices() {
        if c.is_whitespace() && i > 0 {
            let after = i + c.len_utf8();
            if c == '\n' || prev.is_some_and(|p| SENTENCE_ENDS.contains(&p)) {
                sentence = Some(after);
            }
            whitespace = Some(after);
        }
        prev = Some(c);
    }
    sentence.or(whitespace)
}

/// A BCP-47-ish tag accepted by both providers (e.g. "en", "vi", "zh-CN").
fn is_valid_lang_code(code: &str) -> bool {
    !code.is_empty()
        && code.len() <= 10
        && code.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

/// Pulls the concatenated translated segments out of the gtx response shape
/// `[[[translated, original, ...], ...], ...]`.
fn google_text(value: &serde_json::Value) -> String {
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

/// Pulls `responseData.translatedText` out of the MyMemory response shape.
fn mymemory_text(value: &serde_json::Value) -> String {
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
        .is_some_and(|detail| detail.contains(DETAIL));
    matches_detail && matches!(response_status(value), None | Some(200) | Some(403))
}

/// True when MyMemory flagged the daily quota as spent in the response body.
fn quota_finished(value: &serde_json::Value) -> bool {
    value
        .get("quotaFinished")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// The error MyMemory's in-body `responseStatus` stands for, `None` when the
/// body looks usable. The API keeps HTTP 200 while rejecting a request in the
/// body, so the transport status alone would read as success.
fn status_error(value: &serde_json::Value) -> Option<TranslateError> {
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
    fn cache_round_trips_and_clears_when_full() {
        let key = ("en".to_string(), "xin chào".to_string());
        cache_put(key.clone(), "hello".to_string());
        assert_eq!(cache_get(&key).as_deref(), Some("hello"));

        for i in 0..CACHE_LIMIT {
            cache_put(("en".to_string(), format!("fill {i}")), String::new());
        }
        assert!(cache_get(&key).is_none());
    }

    #[test]
    fn reads_google_body() {
        let body = serde_json::json!([[["Xin ", "Hello "], ["chào", "there"]], null, "en"]);
        assert_eq!(google_text(&body), "Xin chào");
        assert_eq!(google_text(&serde_json::json!({})), "");
    }

    #[test]
    fn reads_mymemory_body() {
        let ok = serde_json::json!({
            "responseData": {"translatedText": "Xin chào"},
            "responseStatus": 200,
            "quotaFinished": false,
        });
        assert_eq!(mymemory_text(&ok), "Xin chào");
        assert_eq!(response_status(&ok), Some(200));
        assert_eq!(status_error(&ok), None);

        let throttled = serde_json::json!({ "responseStatus": "429" });
        assert_eq!(status_error(&throttled), Some(TranslateError::RateLimited));

        let rejected = serde_json::json!({
            "responseData": {"translatedText": ""},
            "responseStatus": "403",
        });
        assert_eq!(status_error(&rejected), Some(TranslateError::RequestFailed));
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
    fn the_fallback_turns_away_text_that_would_eat_the_daily_quota() {
        let text = "a".repeat(MYMEMORY_MAX_TEXT_CHARS + 1);
        assert_eq!(mymemory(&text, "vi"), Err(TranslateError::TextTooLong));
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
    fn prefers_a_sentence_end_over_the_last_space() {
        let text = "Look là một công cụ tìm kiếm và khởi chạy siêu nhanh trên máy tính của bạn. Nó cho phép bạn tìm kiếm ứng dụng, tệp tin, cửa sổ đang mở và nhiều thứ khác chỉ bằng vài thao tác gõ phím đơn giản. Giao diện gọn gàng, phản hồi tức thì, và hoạt động hoàn toàn cục bộ nên dữ liệu của bạn luôn được bảo mật. Bạn có thể tùy chỉnh danh sách ngôn ngữ dịch trong phần cấu hình. Tính năng dịch tự động phát hiện ngôn ngữ nguồn và gửi truy vấn đến máy chủ. Kết quả trả về được hiển thị song song cho từng ngôn ngữ đích đã chọn.";
        assert!(text.chars().count() > MAX_QUERY_CHARS);
        let chunks = split_query(text);
        assert_eq!(chunks.len(), 2);
        assert!(chunks[0].ends_with("máy chủ. "));
        assert!(chunks[1].starts_with("Kết quả"));
        assert!(chunks.iter().all(|c| c.chars().count() <= MAX_QUERY_CHARS));
        assert_eq!(chunks.concat(), text);
    }

    #[test]
    fn a_line_break_closes_a_sentence() {
        let line = "a".repeat(300);
        let text = format!("{line}\n{line} {line}");
        let chunks = split_query(&text);
        assert_eq!(chunks[0], format!("{line}\n"));
        assert_eq!(chunks.concat(), text);
    }

    #[test]
    fn falls_back_to_the_last_space_without_a_sentence_end() {
        let text = "word ".repeat(200);
        let chunks = split_query(&text);
        assert!(chunks[0].ends_with("word "));
        assert_eq!(chunks[0].chars().count(), MAX_QUERY_CHARS);
        assert_eq!(chunks.concat(), text);
    }
}
