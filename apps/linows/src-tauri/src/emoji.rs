//! Tauri surface for `/emoji`. Search lives in `core/emoji` (`look-emoji`) so
//! linows and macOS can't drift apart.

#[tauri::command]
pub fn emoji_search(query: String, limit: usize) -> Vec<look_emoji::EmojiHit> {
    look_emoji::search(&query, limit)
}
