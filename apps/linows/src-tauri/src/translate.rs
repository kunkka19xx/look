//! Tauri command over `look_answers::translate`. The translation logic lives in
//! the shared crate (so linows and macOS share one implementation); this just
//! adapts it to the linows wire shape, where `error` is a plain message string.

use serde::Serialize;

#[derive(Serialize)]
pub struct TranslateResult {
    pub original: String,
    pub translated: String,
    pub error: Option<String>,
}

#[tauri::command]
pub fn translate(text: String, target_lang: String) -> TranslateResult {
    let result = look_answers::translate(&text, &target_lang);
    TranslateResult {
        original: result.original,
        translated: result.translated,
        error: result.error.map(|e| e.message().to_string()),
    }
}
