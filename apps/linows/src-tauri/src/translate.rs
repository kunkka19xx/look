use serde::Serialize;

const TRANSLATE_URL_PREFIX: &str =
    "https://translate.googleapis.com/translate_a/single?client=gtx&sl=auto&tl=";
const TRANSLATE_URL_MIDDLE: &str = "&dt=t&q=";
const CURL_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

#[derive(Serialize)]
pub struct TranslateResult {
    pub original: String,
    pub translated: String,
    pub error: Option<String>,
}

#[tauri::command]
pub fn translate(text: String, target_lang: String) -> TranslateResult {
    let text = text.trim().to_string();
    if text.is_empty() {
        return TranslateResult {
            original: text,
            translated: String::new(),
            error: Some("Type text after t\" to translate".into()),
        };
    }

    if target_lang.is_empty()
        || target_lang.len() > 10
        || !target_lang
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return TranslateResult {
            original: text,
            translated: String::new(),
            error: Some("Invalid target language".into()),
        };
    }

    let encoded = percent_encode(&text);
    let url = format!("{TRANSLATE_URL_PREFIX}{target_lang}{TRANSLATE_URL_MIDDLE}{encoded}");

    let mut cmd = std::process::Command::new("curl");
    cmd.args([
        "-s",
        "-m",
        "3",
        "--user-agent",
        CURL_USER_AGENT,
        "--tlsv1.2",
        "-H",
        "Accept-Language: en-US,en;q=0.9",
    ])
    .arg(&url);

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(crate::consts::CREATE_NO_WINDOW);
    }

    let output = cmd.output();

    let body = match output {
        Ok(out) if out.status.success() => match String::from_utf8(out.stdout) {
            Ok(s) => s,
            Err(_) => {
                return TranslateResult {
                    original: text,
                    translated: String::new(),
                    error: Some("Response decode failed".into()),
                };
            }
        },
        _ => {
            return TranslateResult {
                original: text,
                translated: String::new(),
                error: Some("Translation request failed".into()),
            };
        }
    };

    let parsed: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(_) => {
            return TranslateResult {
                original: text,
                translated: String::new(),
                error: Some("Response parse failed".into()),
            };
        }
    };

    let translated = extract_translation(&parsed);
    if translated.trim().is_empty() {
        return TranslateResult {
            original: text,
            translated: String::new(),
            error: Some("Empty translation result".into()),
        };
    }

    TranslateResult {
        original: text,
        translated,
        error: None,
    }
}

fn extract_translation(value: &serde_json::Value) -> String {
    let arr = match value.as_array() {
        Some(a) => a,
        None => return String::new(),
    };
    let translations = match arr.first().and_then(|v| v.as_array()) {
        Some(a) => a,
        None => return String::new(),
    };
    let mut result = String::new();
    for group in translations {
        if let Some(parts) = group.as_array()
            && let Some(s) = parts.first().and_then(|v| v.as_str())
        {
            result.push_str(s);
        }
    }
    result
}

fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push('%');
                out.push(char::from(HEX[(b >> 4) as usize]));
                out.push(char::from(HEX[(b & 0xf) as usize]));
            }
        }
    }
    out
}

const HEX: [u8; 16] = *b"0123456789ABCDEF";
