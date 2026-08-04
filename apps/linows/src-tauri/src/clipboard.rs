use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

const MAX_ENTRY_BYTES: usize = 30_000;
const POLL_MS: u64 = 500;

#[derive(Clone, Serialize, Deserialize)]
pub struct ClipboardEntry {
    pub text: String,
    pub timestamp: u64,
    pub char_count: usize,
    pub line_count: usize,
    /// What re-copying puts on the clipboard, when that differs from the text
    /// shown in the list: `1/1000 = 0.001` in the list, `0.001` pasted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<String>,
}

struct ClipboardState {
    entries: Vec<ClipboardEntry>,
    last_text: String,
    max_entries: usize,
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
    let max_entries = crate::config::clipboard_history_limit();
    let mut entries = load_entries();
    entries.truncate(max_entries);
    let last_text = entries.first().map(|e| e.text.clone()).unwrap_or_default();
    *STATE.lock().unwrap() = Some(ClipboardState {
        entries,
        last_text,
        max_entries,
    });

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

            push_entry(state, text, None);
        }
    });
}

/// Insert `text` at the head of the history, dropping any earlier copy of it.
/// `payload` overrides what re-copying the entry writes to the clipboard.
fn push_entry(state: &mut ClipboardState, text: String, payload: Option<String>) {
    state.entries.retain(|e| e.text != text);
    state.entries.insert(
        0,
        ClipboardEntry {
            char_count: text.chars().count(),
            line_count: text.lines().count(),
            text,
            timestamp: now_secs(),
            payload,
        },
    );
    state.entries.truncate(state.max_entries);
    save_entries(&state.entries);
}

/// Re-reads the clipboard section of `~/.look.config` and applies it to the running
/// monitor (trimming and persisting any entries beyond a lowered limit), so file-only
/// clipboard settings take effect on config reload without a restart. One reload entry
/// point for the whole subsystem: adding a clipboard key means another apply line here,
/// not a new function wired into `reload_config`.
pub fn reload_from_config() {
    let mut lock = STATE.lock().unwrap();
    if let Some(state) = lock.as_mut() {
        state.max_entries = crate::config::clipboard_history_limit();
        if state.entries.len() > state.max_entries {
            state.entries.truncate(state.max_entries);
            save_entries(&state.entries);
        }
    }
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

fn remove_clipboard_entry(entries: &mut Vec<ClipboardEntry>, timestamp: u64, text: &str) -> bool {
    let Some(index) = entries
        .iter()
        .position(|entry| entry.timestamp == timestamp && entry.text == text)
    else {
        return false;
    };
    entries.remove(index);
    true
}

#[tauri::command]
pub fn delete_clipboard_entry(timestamp: u64, text: String) -> bool {
    let mut lock = STATE.lock().unwrap();
    let Some(state) = lock.as_mut() else {
        return false;
    };
    if !remove_clipboard_entry(&mut state.entries, timestamp, &text) {
        return false;
    }
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

/// Copy `text` but file it under `label` in the history. `last_text` is set so
/// the monitor doesn't race a second, unlabelled entry for the same write.
#[tauri::command]
pub fn copy_to_clipboard_labeled(text: String, label: String) -> Result<(), String> {
    copy_to_clipboard(text.clone())?;
    let mut lock = STATE.lock().unwrap();
    if let Some(state) = lock.as_mut() {
        state.last_text = text.clone();
        push_entry(state, label, Some(text));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ClipboardEntry, remove_clipboard_entry};

    fn entry(text: &str, timestamp: u64) -> ClipboardEntry {
        ClipboardEntry {
            text: text.to_owned(),
            timestamp,
            char_count: text.chars().count(),
            line_count: text.lines().count(),
            payload: None,
        }
    }

    #[test]
    fn removes_the_matching_entry_instead_of_a_filtered_position() {
        let mut entries = vec![entry("unrelated", 10), entry("needle", 20)];

        assert!(remove_clipboard_entry(&mut entries, 20, "needle"));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].text, "unrelated");
    }

    #[test]
    fn keeps_history_unchanged_when_identity_does_not_match() {
        let mut entries = vec![entry("same timestamp", 10)];

        assert!(!remove_clipboard_entry(&mut entries, 10, "different text"));
        assert_eq!(entries.len(), 1);
    }
}
