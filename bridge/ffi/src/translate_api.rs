use crate::state::{cstr_to_string, store_json_allocation};
use look_storage::percent_encode;
use std::ffi::CString;
use std::os::raw::c_char;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
// Suppress the console window when curl spawns from a GUI shell.
// See https://learn.microsoft.com/en-us/windows/win32/procthread/process-creation-flags
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

const TRANSLATE_URL_PREFIX: &str =
    "https://translate.googleapis.com/translate_a/single?client=gtx&sl=auto&tl=";
const TRANSLATE_URL_MIDDLE: &str = "&dt=t&q=";
const CURL_BIN: &str = "curl";
const CURL_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
// `--compressed` was dropped: the curl shipped in C:\Windows\System32 on some
// Windows 10 builds lacks zlib support and exits non-zero with "the installed
// libcurl version doesn't support this", surfacing as "Translation request
// failed". Translation responses are ~hundreds of bytes; the bandwidth saving
// isn't worth a platform fork.
const CURL_ARGS_PREFIX: [&str; 8] = [
    "-s",
    "-m",
    "3",
    "--user-agent",
    CURL_USER_AGENT,
    "--tlsv1.2",
    "-H",
    "Accept-Language: en-US,en;q=0.9",
];

#[derive(Clone, Copy)]
enum TranslateError {
    EmptyText,
    InvalidTargetLang,
    RequestFailed,
    DecodeFailed,
    ExecFailed,
    ParseFailed,
    EmptyResult,
    SerializeFailed,
}

impl TranslateError {
    fn code(self) -> &'static str {
        match self {
            Self::EmptyText => "empty_text",
            Self::InvalidTargetLang => "invalid_target_lang",
            Self::RequestFailed => "translate_request_failed",
            Self::DecodeFailed => "translate_decode_failed",
            Self::ExecFailed => "translate_exec_failed",
            Self::ParseFailed => "translate_parse_failed",
            Self::EmptyResult => "translate_empty_result",
            Self::SerializeFailed => "serialize_failed",
        }
    }

    fn message(self) -> &'static str {
        match self {
            Self::EmptyText => "Type text after t\" to translate",
            Self::InvalidTargetLang => "Invalid target language code",
            Self::RequestFailed => "Translation request failed",
            Self::DecodeFailed => "Translation response decode failed",
            Self::ExecFailed => "Translation command execution failed",
            Self::ParseFailed => "Translation response parse failed",
            Self::EmptyResult => "Translation returned empty result",
            Self::SerializeFailed => "Failed to serialize translation response",
        }
    }
}

const JSON_TRANSLATE_ERROR_FALLBACK: &str = "{\"original\":\"\",\"translated\":\"\",\"error\":{\"code\":\"unknown\",\"message\":\"Unknown translation error\"}}";

#[derive(serde::Deserialize)]
struct TranslateResponse(serde_json::Value);

pub(crate) fn look_translate_json_impl(
    text: *const c_char,
    target_lang: *const c_char,
) -> *mut c_char {
    let text = cstr_to_string(text);
    let target_lang = cstr_to_string(target_lang);

    if text.trim().is_empty() {
        return translate_error_json(&text, TranslateError::EmptyText);
    }

    if !is_valid_lang_code(&target_lang) {
        return translate_error_json(&text, TranslateError::InvalidTargetLang);
    }

    let encoded_text = percent_encode(&text);
    let mut url = String::with_capacity(
        TRANSLATE_URL_PREFIX.len()
            + TRANSLATE_URL_MIDDLE.len()
            + target_lang.len()
            + encoded_text.len(),
    );
    url.push_str(TRANSLATE_URL_PREFIX);
    url.push_str(&target_lang);
    url.push_str(TRANSLATE_URL_MIDDLE);
    url.push_str(&encoded_text);

    let mut command = std::process::Command::new(CURL_BIN);
    command.args(CURL_ARGS_PREFIX).arg(&url);
    #[cfg(target_os = "windows")]
    command.creation_flags(CREATE_NO_WINDOW);
    let output = command.output();

    let body = match output {
        Ok(out) => {
            if !out.status.success() {
                return translate_error_json(&text, TranslateError::RequestFailed);
            }
            match String::from_utf8(out.stdout) {
                Ok(s) => s,
                Err(_) => {
                    return translate_error_json(&text, TranslateError::DecodeFailed);
                }
            }
        }
        Err(_) => {
            return translate_error_json(&text, TranslateError::ExecFailed);
        }
    };

    let parsed: TranslateResponse = match serde_json::from_str(&body) {
        Ok(p) => p,
        Err(_) => {
            return translate_error_json(&text, TranslateError::ParseFailed);
        }
    };

    let translated = parse_translate_response(&parsed.0);
    if translated.trim().is_empty() {
        return translate_error_json(&text, TranslateError::EmptyResult);
    }

    let result = serde_json::json!({
        "original": text,
        "translated": translated,
        "error": null
    });

    let json = serde_json::to_string(&result)
        .unwrap_or_else(|_| translate_error_string("", TranslateError::SerializeFailed));
    let cstring = CString::new(json).expect("valid json");
    store_json_allocation(cstring)
}

fn translate_error_json(text: &str, err: TranslateError) -> *mut c_char {
    let json = translate_error_string(text, err);
    let cstring = CString::new(json).expect("valid json");
    store_json_allocation(cstring)
}

fn translate_error_string(text: &str, err: TranslateError) -> String {
    let payload = serde_json::json!({
        "original": text,
        "translated": "",
        "error": {
            "code": err.code(),
            "message": err.message(),
        }
    });
    serde_json::to_string(&payload).unwrap_or_else(|_| JSON_TRANSLATE_ERROR_FALLBACK.to_string())
}

fn parse_translate_response(value: &serde_json::Value) -> String {
    let arr = match value.as_array() {
        Some(a) => a,
        None => return String::new(),
    };

    let translations = match arr.first() {
        Some(v) => match v.as_array() {
            Some(a) => a,
            None => return String::new(),
        },
        None => return String::new(),
    };

    let mut result = String::new();
    for group in translations {
        if let Some(parts) = group.as_array()
            && let Some(translated) = parts.first()
            && let Some(s) = translated.as_str()
        {
            result.push_str(s);
        }
    }
    result
}

/// Validates that `code` looks like a BCP-47 language tag accepted by Google
/// Translate (e.g. "en", "vi", "zh-CN", "pt-BR").
fn is_valid_lang_code(code: &str) -> bool {
    let code = code.trim();
    if code.is_empty() || code.len() > 10 {
        return false;
    }
    code.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}
