use std::collections::HashSet;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::clipimage;

const MAX_ENTRY_BYTES: usize = 30_000;
const POLL_MS: u64 = 500;
/// Polls between image reads, at the fastest and (once the same picture keeps
/// coming back) at the slowest. Reading an image decodes it, which is not work
/// to do twice a second for as long as one sits on the clipboard.
const IMAGE_POLL_MIN_TICKS: u32 = 2;
const IMAGE_POLL_MAX_TICKS: u32 = 8;

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

/// One copied image. The pixels live in a file named after `hash`, which is
/// also the row's identity: a re-copy lands on the row it already had.
#[derive(Clone, Serialize, Deserialize)]
pub struct ClipboardImageEntry {
    pub hash: String,
    pub timestamp: u64,
    pub width: u32,
    pub height: u32,
    pub byte_size: usize,
    /// The app the copy came from, when the session will say. Pixels have no
    /// name, so this is what the row is called.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// The thumbnail path is derived rather than stored, so moving the image
/// directory cannot orphan the index. The picture itself travels as a data URL
/// (`clipboard_image_data_url`), not as a path.
#[derive(Serialize)]
pub struct ClipboardImageRow {
    #[serde(flatten)]
    entry: ClipboardImageEntry,
    thumb_path: String,
}

struct ClipboardState {
    entries: Vec<ClipboardEntry>,
    /// Apart from `entries`: the two share no search key and no row shape.
    images: Vec<ClipboardImageEntry>,
    last_text: String,
    /// Pixels already filed, so one left on the clipboard is read once rather
    /// than re-encoded on every poll.
    last_image_hash: String,
    max_entries: usize,
    max_images: usize,
}

static STATE: Mutex<Option<ClipboardState>> = Mutex::new(None);
/// When true, the next clipboard change is from Look itself - skip it.
static SKIP_NEXT: AtomicBool = AtomicBool::new(false);
/// The text an image copy publishes, waiting to be read back. Keyed by the
/// text, not a flag: the write may repeat what is already on the clipboard.
static PENDING_SELF_TEXT: Mutex<Option<String>> = Mutex::new(None);

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn data_path(name: &str) -> Option<std::path::PathBuf> {
    dirs::data_dir().map(|d| d.join("look").join(name))
}

fn load_json<T: serde::de::DeserializeOwned + Default>(name: &str) -> T {
    let Some(path) = data_path(name) else {
        return T::default();
    };
    let Ok(data) = std::fs::read_to_string(&path) else {
        return T::default();
    };
    serde_json::from_str(&data).unwrap_or_default()
}

fn save_json<T: Serialize>(name: &str, value: &T) {
    let Some(path) = data_path(name) else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string(value) {
        let _ = std::fs::write(&path, json);
    }
}

fn load_entries() -> Vec<ClipboardEntry> {
    load_json("clipboard.json")
}

fn save_entries(entries: &[ClipboardEntry]) {
    save_json("clipboard.json", &entries);
}

/// A row whose file is gone is dropped: the pixels are the clip.
fn load_images() -> Vec<ClipboardImageEntry> {
    let entries: Vec<ClipboardImageEntry> = load_json("clipboard-images.json");
    entries
        .into_iter()
        .filter(|entry| clipimage::file_path(&entry.hash).is_some_and(|path| path.exists()))
        .collect()
}

fn save_images(images: &[ClipboardImageEntry]) {
    save_json("clipboard-images.json", &images);
    clipimage::sweep_orphans(
        &images
            .iter()
            .map(|e| e.hash.clone())
            .collect::<HashSet<_>>(),
    );
}

/// Mark that Look is about to write to clipboard - monitor should skip the next change.
pub fn mark_self_write() {
    SKIP_NEXT.store(true, Ordering::Relaxed);
}

/// Start background clipboard polling thread.
pub fn start_monitor() {
    let max_entries = crate::config::clipboard_history_limit();
    let max_images = crate::config::clipboard_image_limit();
    let mut entries = load_entries();
    entries.truncate(max_entries);
    let mut images = load_images();
    images.truncate(max_images);
    let last_text = entries.first().map(|e| e.text.clone()).unwrap_or_default();
    save_images(&images);
    *STATE.lock().unwrap() = Some(ClipboardState {
        entries,
        images,
        last_text,
        last_image_hash: String::new(),
        max_entries,
        max_images,
    });

    std::thread::spawn(|| {
        let mut clipboard = match arboard::Clipboard::new() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[clipboard] failed to init: {e}");
                return;
            }
        };

        let mut image_backoff = IMAGE_POLL_MIN_TICKS;
        let mut image_wait = 0;

        loop {
            std::thread::sleep(std::time::Duration::from_millis(POLL_MS));

            // A copy carrying text is not an image copy, and the cheap read
            // is the one that runs every poll.
            if let Ok(text) = clipboard.get_text() {
                image_backoff = IMAGE_POLL_MIN_TICKS;
                image_wait = 0;
                capture_text(text);
                continue;
            }

            if image_wait > 0 {
                image_wait -= 1;
                continue;
            }
            image_backoff = if capture_image(&mut clipboard) {
                IMAGE_POLL_MIN_TICKS
            } else {
                (image_backoff * 2).min(IMAGE_POLL_MAX_TICKS)
            };
            image_wait = image_backoff;
        }
    });
}

fn capture_text(text: String) {
    if text.is_empty() || text.len() > MAX_ENTRY_BYTES {
        return;
    }

    // Before the `last_text` check: a repeat copy publishes the same path.
    let own_write = claim_self_text(&text);

    let mut lock = STATE.lock().unwrap();
    let Some(state) = lock.as_mut() else { return };

    if text == state.last_text {
        return;
    }

    state.last_text = text.clone();

    // Skip if this was Look's own write
    if own_write || SKIP_NEXT.swap(false, Ordering::Relaxed) {
        return;
    }

    push_entry(state, text, None);
}

fn claim_self_text(text: &str) -> bool {
    let mut pending = PENDING_SELF_TEXT.lock().unwrap();
    if pending.as_deref() == Some(text) {
        *pending = None;
        return true;
    }
    false
}

/// Whether a new picture was filed, which decides how soon to look again. The
/// lock is not held across the encode: that would stall a `ci"` query.
fn capture_image(clipboard: &mut arboard::Clipboard) -> bool {
    let Ok(image) = clipboard.get_image() else {
        return false;
    };
    if image.width == 0 || image.height == 0 || image.width * image.height > clipimage::MAX_PIXELS {
        return false;
    }
    let width = image.width as u32;
    let height = image.height as u32;
    let hash = clipimage::hash_pixels(width, height, &image.bytes);

    match STATE.lock().unwrap().as_ref() {
        Some(state) if state.last_image_hash == hash => return false,
        Some(_) => {}
        None => return false,
    }

    let Some(stored) = clipimage::store(width, height, &image.bytes) else {
        return false;
    };
    // Before the lock: it talks to the compositor.
    let source = focused_app();

    let mut lock = STATE.lock().unwrap();
    let Some(state) = lock.as_mut() else {
        return false;
    };
    state.last_image_hash = stored.hash.clone();
    insert_image(
        &mut state.images,
        ClipboardImageEntry {
            hash: stored.hash,
            timestamp: now_secs(),
            width: stored.width,
            height: stored.height,
            byte_size: stored.byte_size,
            source,
        },
        state.max_images,
    );
    save_images(&state.images);
    true
}

/// `None` on a session that will not say (GNOME and KDE Wayland); the row
/// words that as "screen", as macOS does.
fn focused_app() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        crate::platform::linux::focused_app::focused_app_name()
    }

    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::process::focused_app_name()
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        None
    }
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

/// Drops any older row for the same pixels: the file is named after them, so
/// two rows would point at one picture.
fn insert_image(images: &mut Vec<ClipboardImageEntry>, entry: ClipboardImageEntry, limit: usize) {
    images.retain(|e| e.hash != entry.hash);
    images.insert(0, entry);
    images.truncate(limit);
}

/// Re-reads the clipboard section of `~/.look/config` and applies it to the running
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
        state.max_images = crate::config::clipboard_image_limit();
        if state.images.len() > state.max_images {
            state.images.truncate(state.max_images);
            save_images(&state.images);
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

/// Every copied image, newest first. Capped at tens of rows, so it travels
/// whole and `ci"word` filters it where the row's words are written.
#[tauri::command]
pub fn get_clipboard_images() -> Vec<ClipboardImageRow> {
    let lock = STATE.lock().unwrap();
    let Some(state) = lock.as_ref() else {
        return vec![];
    };
    state
        .images
        .iter()
        .filter_map(|entry| {
            Some(ClipboardImageRow {
                thumb_path: clipimage::thumbnail_path(&entry.hash)?
                    .to_string_lossy()
                    .into_owned(),
                entry: entry.clone(),
            })
        })
        .collect()
}

#[tauri::command]
pub fn delete_clipboard_image(hash: String) -> bool {
    let mut lock = STATE.lock().unwrap();
    let Some(state) = lock.as_mut() else {
        return false;
    };
    let before = state.images.len();
    state.images.retain(|entry| entry.hash != hash);
    if state.images.len() == before {
        return false;
    }
    // Saving sweeps, so the pixels go with the row.
    save_images(&state.images);
    true
}

/// The picture as a data URL: the asset protocol does not serve these files in
/// the WebKitGTK webview, so images arrive the way icons do.
#[tauri::command]
pub fn clipboard_image_data_url(hash: String) -> Option<String> {
    let path = clipimage::file_path(&hash)?;
    crate::platform::shared::read_icon_file(&path.to_string_lossy())
}

/// Back on the clipboard as pixels and as a file, so it pastes into an editor
/// and into a file manager alike.
#[tauri::command]
pub fn copy_clipboard_image(hash: String) -> Result<(), String> {
    let path = clipimage::file_path(&hash).ok_or("no clipboard image directory")?;
    if !path.exists() {
        return Err("the image is no longer on disk".to_string());
    }
    // The pixels are filed already, so the monitor must not file them again.
    let previous = swap_last_image_hash(hash);
    // Before the write: the Linux grab publishes the path as it takes.
    set_pending_self_text(Some(path.to_string_lossy().into_owned()));

    match write_image(&path) {
        Ok(true) => Ok(()),
        // Pixels alone: nothing will ever spend the claim.
        Ok(false) => {
            set_pending_self_text(None);
            Ok(())
        }
        Err(e) => {
            set_pending_self_text(None);
            swap_last_image_hash(previous);
            Err(e)
        }
    }
}

fn set_pending_self_text(text: Option<String>) {
    *PENDING_SELF_TEXT.lock().unwrap() = text;
}

/// Whether the write published the path as text alongside the pixels.
fn write_image(path: &std::path::Path) -> Result<bool, String> {
    #[cfg(target_os = "linux")]
    {
        crate::platform::linux::clipboard::copy_image(path)
    }

    #[cfg(target_os = "windows")]
    {
        let (width, height, rgba) =
            clipimage::read_rgba(path).ok_or("the image could not be read")?;
        let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
        clipboard
            .set_image(arboard::ImageData {
                width: width as usize,
                height: height as usize,
                bytes: rgba.into(),
            })
            .map_err(|e| e.to_string())?;
        // Windows replaces the clipboard with the pixels alone.
        Ok(false)
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = path;
        Err("image clipboard not supported on this platform".to_string())
    }
}

/// Hands back the previous hash, so a failed write can put it back.
fn swap_last_image_hash(hash: String) -> String {
    let mut lock = STATE.lock().unwrap();
    match lock.as_mut() {
        Some(state) => std::mem::replace(&mut state.last_image_hash, hash),
        None => String::new(),
    }
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
        // The entry is recorded here, so the monitor has nothing left to skip.
        // Leaving the flag armed would swallow the next external copy instead.
        SKIP_NEXT.store(false, Ordering::Relaxed);
        push_entry(state, label, Some(text));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ClipboardEntry, ClipboardImageEntry, insert_image, remove_clipboard_entry};

    fn image(hash: &str, timestamp: u64) -> ClipboardImageEntry {
        ClipboardImageEntry {
            hash: hash.to_owned(),
            timestamp,
            width: 8,
            height: 8,
            byte_size: 64,
            source: None,
        }
    }

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

    /// One row per picture, back on top when it returns, and the list stays
    /// within its limit: the file is named after the pixels, so two rows would
    /// point at one picture.
    #[test]
    fn an_image_list_dedupes_by_hash_and_stays_bounded() {
        let mut images = vec![image("aaaa", 10)];
        insert_image(&mut images, image("bbbb", 20), 3);
        insert_image(&mut images, image("aaaa", 30), 3);

        assert_eq!(images.len(), 2);
        assert_eq!(images[0].hash, "aaaa");
        assert_eq!(images[0].timestamp, 30);

        for i in 0..3 {
            insert_image(&mut images, image(&format!("hash{i}"), i), 3);
        }
        // The oldest went, not the newest: each row is a file on disk.
        assert_eq!(images.len(), 3);
        assert_eq!(images[0].hash, "hash2");
        assert!(!images.iter().any(|e| e.hash == "bbbb"));
    }
}
