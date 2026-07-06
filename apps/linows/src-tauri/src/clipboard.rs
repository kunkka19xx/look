use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

const MAX_ENTRIES: usize = 10;
const MAX_ENTRY_BYTES: usize = 30_000;
const POLL_MS: u64 = 500;

#[derive(Clone, Serialize, Deserialize)]
pub struct ClipboardEntry {
    pub text: String,
    pub timestamp: u64,
    pub char_count: usize,
    pub line_count: usize,
}

struct ClipboardState {
    entries: Vec<ClipboardEntry>,
    last_text: String,
}

static STATE: Mutex<Option<ClipboardState>> = Mutex::new(None);
/// When true, the next clipboard change is from Look itself - skip it.
static SKIP_NEXT: AtomicBool = AtomicBool::new(false);

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn data_path() -> Option<std::path::PathBuf> {
    dirs::data_dir().map(|d| d.join("look").join("clipboard.json"))
}

fn load_entries() -> Vec<ClipboardEntry> {
    let Some(path) = data_path() else {
        return vec![];
    };
    let Ok(data) = std::fs::read_to_string(&path) else {
        return vec![];
    };
    serde_json::from_str(&data).unwrap_or_default()
}

fn save_entries(entries: &[ClipboardEntry]) {
    let Some(path) = data_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string(entries) {
        let _ = std::fs::write(&path, json);
    }
}

/// Mark that Look is about to write to clipboard - monitor should skip the next change.
pub fn mark_self_write() {
    SKIP_NEXT.store(true, Ordering::Relaxed);
}

/// Start background clipboard polling thread.
pub fn start_monitor() {
    let entries = load_entries();
    let last_text = entries.first().map(|e| e.text.clone()).unwrap_or_default();
    *STATE.lock().unwrap() = Some(ClipboardState { entries, last_text });

    std::thread::spawn(|| {
        let mut clipboard = match arboard::Clipboard::new() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[clipboard] failed to init: {e}");
                return;
            }
        };

        loop {
            std::thread::sleep(std::time::Duration::from_millis(POLL_MS));

            let text = match clipboard.get_text() {
                Ok(t) => t,
                Err(_) => continue,
            };

            if text.is_empty() || text.len() > MAX_ENTRY_BYTES {
                continue;
            }

            let mut lock = STATE.lock().unwrap();
            let state = match lock.as_mut() {
                Some(s) => s,
                None => continue,
            };

            if text == state.last_text {
                continue;
            }

            state.last_text = text.clone();

            // Skip if this was Look's own write
            if SKIP_NEXT.swap(false, Ordering::Relaxed) {
                continue;
            }

            // Deduplicate: remove existing entry with same text
            state.entries.retain(|e| e.text != text);

            let entry = ClipboardEntry {
                char_count: text.chars().count(),
                line_count: text.lines().count(),
                text,
                timestamp: now_secs(),
            };

            state.entries.insert(0, entry);
            state.entries.truncate(MAX_ENTRIES);
            save_entries(&state.entries);
        }
    });
}

#[tauri::command]
pub fn get_clipboard_history(query: String) -> Vec<ClipboardEntry> {
    let lock = STATE.lock().unwrap();
    let Some(state) = lock.as_ref() else {
        return vec![];
    };
    if query.is_empty() {
        return state.entries.clone();
    }
    let q = query.to_lowercase();
    state
        .entries
        .iter()
        .filter(|e| e.text.to_lowercase().contains(&q))
        .cloned()
        .collect()
}

#[tauri::command]
pub fn delete_clipboard_entry(index: usize) -> bool {
    let mut lock = STATE.lock().unwrap();
    let Some(state) = lock.as_mut() else {
        return false;
    };
    if index >= state.entries.len() {
        return false;
    }
    state.entries.remove(index);
    save_entries(&state.entries);
    true
}

#[tauri::command]
pub fn copy_to_clipboard(text: String) -> Result<(), String> {
    mark_self_write();
    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    clipboard.set_text(&text).map_err(|e| e.to_string())?;
    Ok(())
}
