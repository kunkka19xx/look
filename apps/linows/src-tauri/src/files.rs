use serde::Serialize;
use std::sync::atomic::Ordering;
use std::time::UNIX_EPOCH;
use tauri::Manager;

type FileDialog = tauri_plugin_dialog::FileDialogBuilder<tauri::Wry>;

/// RAII guard around a native picker. Sets crate::PICKING_FILE for as long as it
/// lives, so the focus-loss auto-hide skips while a dialog is on screen, and
/// hands back `None` when one is already open so a double-click cannot stack two
/// dialogs.
struct PickerGuard;
impl PickerGuard {
    fn acquire() -> Option<Self> {
        crate::PICKING_FILE
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .ok()
            .map(|_| Self)
    }
}
impl Drop for PickerGuard {
    fn drop(&mut self) {
        crate::PICKING_FILE.store(false, Ordering::SeqCst);
    }
}

/// The launcher is a topmost window, and on Windows a topmost window outranks a
/// non-topmost dialog even when it owns it. Drop the topmost bit for as long as
/// the picker is up so it can sit in front, then restore it (see issue #467).
/// A no-op off Windows, where the field stays `None`: there the launcher is a
/// layer-shell surface and Tauri's window a hidden husk, so owning the dialog to
/// it is a platform-specific follow-up.
struct TopmostGuard {
    window: Option<tauri::WebviewWindow>,
}
impl TopmostGuard {
    fn lower(app: &tauri::AppHandle) -> Self {
        let window = cfg!(target_os = "windows")
            .then(|| app.get_webview_window(crate::consts::MAIN_WINDOW))
            .flatten();
        if let Some(w) = &window {
            let _ = w.set_always_on_top(false);
        }
        Self { window }
    }

    /// Owner the dialog to the launcher, so the OS keeps a non-topmost dialog
    /// above it. Reuses the window already resolved by `lower`.
    fn parent(&self, builder: FileDialog) -> FileDialog {
        match &self.window {
            Some(w) => builder.set_parent(w),
            None => builder,
        }
    }
}
impl Drop for TopmostGuard {
    fn drop(&mut self) {
        if let Some(w) = &self.window {
            let _ = w.set_always_on_top(true);
        }
    }
}

#[derive(Serialize)]
pub struct FileMeta {
    pub size: Option<u64>,
    pub modified: Option<String>,
    pub is_image: bool,
    /// A block's row says nothing about what it points at (`format = "json"`
    /// carries a path and no kind), so the panel asks the filesystem which
    /// preview to draw.
    pub is_dir: bool,
}

const IMAGE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "bmp", "tiff", "tif", "webp", "svg", "ico", "heic",
];

#[tauri::command]
pub fn get_file_meta(path: String) -> FileMeta {
    let p = std::path::Path::new(&path);
    let meta = std::fs::metadata(p).ok();

    // A directory's own length is its entry table (4 KB on ext4, 0 on NTFS),
    // not its contents; folders show item counts instead.
    let size = meta.as_ref().filter(|m| !m.is_dir()).map(|m| m.len());

    let modified = meta.as_ref().and_then(|m| {
        let mod_time = m.modified().ok()?;
        let secs = mod_time.duration_since(UNIX_EPOCH).ok()?.as_secs();
        Some(time_from_unix(secs))
    });

    let is_image = p
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| IMAGE_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false);

    FileMeta {
        size,
        modified,
        is_image,
        is_dir: meta.as_ref().map(|m| m.is_dir()).unwrap_or(false),
    }
}

#[tauri::command]
pub fn get_app_version(path: String) -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        crate::platform::linux::version::read(&path)
    }
    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::version::read(&path)
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = path;
        None
    }
}

#[tauri::command]
pub fn is_dev_build() -> bool {
    cfg!(debug_assertions)
}

/// Source of truth is `tauri.conf.json` (exposed via `PackageInfo`).
/// Debug builds report a fixed `0.1.0` so the update check can be exercised
/// end-to-end against the latest GitHub release.
#[tauri::command]
pub fn get_lookapp_version(app: tauri::AppHandle) -> String {
    if cfg!(debug_assertions) {
        return "0.1.0".to_string();
    }
    app.package_info().version.to_string()
}

#[tauri::command]
pub fn copy_files_to_clipboard(paths: Vec<String>) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    crate::clipboard::mark_self_write();

    #[cfg(target_os = "linux")]
    {
        crate::platform::linux::clipboard::copy_files(&paths)
    }

    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::clipboard::copy_files(&paths)
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = paths;
        Err("file clipboard not supported on this platform".to_string())
    }
}

#[tauri::command]
pub fn get_home_dir() -> Option<String> {
    // Windows has USERPROFILE, not HOME - prefer it there so JS-side quick
    // folders (Desktop/Documents/…) resolve. Fall back to HOME for Linux/macOS.
    #[cfg(target_os = "windows")]
    {
        std::env::var("USERPROFILE")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .or_else(|| std::env::var("HOME").ok())
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::env::var("HOME").ok()
    }
}

#[derive(Serialize)]
pub struct QuickFolder {
    pub title: String,
    pub path: String,
}

/// Resolve the user's "quick" home folders for the search-time pin list.
/// On Windows, uses `SHGetKnownFolderPath` because Desktop/Documents/etc.
/// are routinely redirected (OneDrive, Group Policy) and the `~\Desktop`
/// guess is unreliable. On Linux/macOS, falls back to `$HOME/<name>` and
/// drops folders that don't exist.
#[tauri::command]
pub fn get_quick_folders() -> Vec<QuickFolder> {
    #[cfg(target_os = "windows")]
    {
        let mut folders: Vec<QuickFolder> = crate::platform::windows::known_folders::list()
            .into_iter()
            .map(|(title, path)| QuickFolder { title, path })
            .collect();
        // The Recycle Bin is a shell namespace, not a real directory, so it's
        // pinned with a `shell:` location that `open_path` hands to Explorer -
        // the Windows analogue of Linux's pinned Trash (Ctrl+D empties it).
        folders.push(QuickFolder {
            title: "Recycle Bin".to_string(),
            path: "shell:RecycleBinFolder".to_string(),
        });
        folders
    }
    #[cfg(not(target_os = "windows"))]
    {
        let Some(home) = std::env::var("HOME").ok().filter(|v| !v.is_empty()) else {
            return Vec::new();
        };
        // macOS uses "Movies" where Windows/Linux use "Videos"; pick the one
        // the platform's native file manager shows so typing what the user
        // sees pins it.
        #[cfg(target_os = "macos")]
        let names: &[&str] = &[
            "Desktop",
            "Documents",
            "Downloads",
            "Pictures",
            "Movies",
            "Music",
        ];
        #[cfg(not(target_os = "macos"))]
        let names: &[&str] = &[
            "Desktop",
            "Documents",
            "Downloads",
            "Pictures",
            "Videos",
            "Music",
        ];

        let mut folders: Vec<QuickFolder> = names
            .iter()
            .filter_map(|n| {
                let path = format!("{home}/{n}");
                std::path::Path::new(&path).is_dir().then(|| QuickFolder {
                    title: (*n).to_string(),
                    path,
                })
            })
            .collect();

        #[cfg(target_os = "linux")]
        if let Some(trash_dir) = crate::trash::linux_trash_dir() {
            let files_dir = trash_dir.join("files");
            if files_dir.is_dir() {
                folders.push(QuickFolder {
                    title: "Trash".to_string(),
                    path: files_dir.to_string_lossy().into_owned(),
                });
            }
        }

        folders
    }
}

const AUDIO_EXTENSIONS: &[&str] = &["mp3", "m4a", "wav", "aac", "flac", "ogg", "aiff", "alac"];

#[tauri::command]
pub fn scan_music_folder(folder: String) -> Vec<String> {
    let dir = std::path::Path::new(&folder);
    let Ok(entries) = std::fs::read_dir(dir) else {
        return vec![];
    };
    let mut files: Vec<String> = entries
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            if !path.is_file() {
                return None;
            }
            let ext = path.extension()?.to_str()?.to_lowercase();
            if AUDIO_EXTENSIONS.contains(&ext.as_str()) {
                Some(path.to_string_lossy().to_string())
            } else {
                None
            }
        })
        .collect();
    files.sort();
    files
}

/// Shared picker plumbing: refuse a second dialog, lower the launcher's topmost
/// bit, own the dialog to it, then block until the callback reports back.
/// `show` picks the dialog flavour and sends the chosen path down `tx`.
fn run_picker<F>(app: &tauri::AppHandle, show: F) -> Option<String>
where
    F: FnOnce(FileDialog, std::sync::mpsc::Sender<Option<String>>),
{
    use tauri_plugin_dialog::DialogExt;
    let _guard = PickerGuard::acquire()?;
    // Dropped before `_guard`, so the topmost bit is back before the
    // focus-loss auto-hide is armed again.
    let topmost = TopmostGuard::lower(app);
    let (tx, rx) = std::sync::mpsc::channel();
    show(topmost.parent(app.dialog().file()), tx);
    rx.recv().ok().flatten()
}

#[tauri::command]
pub async fn pick_folder(app: tauri::AppHandle) -> Option<String> {
    run_picker(&app, |builder, tx| {
        builder
            .set_title("Choose Music Folder")
            .pick_folder(move |folder| {
                let _ = tx.send(folder.map(|f| f.to_string()));
            });
    })
}

#[tauri::command]
pub fn list_fonts() -> Vec<String> {
    #[cfg(target_os = "linux")]
    {
        crate::platform::linux::fonts::list()
    }
    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::fonts::list()
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        Vec::new()
    }
}

#[tauri::command]
pub async fn pick_image(app: tauri::AppHandle) -> Option<String> {
    run_picker(&app, |builder, tx| {
        builder
            .set_title("Choose Background Image")
            .add_filter(
                "Images",
                &["png", "jpg", "jpeg", "webp", "bmp", "gif", "svg"],
            )
            .pick_file(move |file| {
                let _ = tx.send(file.map(|f| f.to_string()));
            });
    })
}

/// List the contents of a directory for folder preview.
/// Returns items (folders first, then files, alphabetically), capped at 30.
const LIST_FOLDER_CAP: usize = 30;

#[derive(Serialize)]
pub struct FolderEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: Option<u64>,
}

#[derive(Serialize)]
pub struct FolderListing {
    pub items: Vec<FolderEntry>,
    pub file_count: usize,
    pub folder_count: usize,
    pub truncated: bool,
}

#[tauri::command]
pub fn list_folder(path: String) -> Option<FolderListing> {
    let entries = std::fs::read_dir(&path).ok()?;

    let mut folders: Vec<String> = Vec::new();
    let mut files: Vec<String> = Vec::new();

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            folders.push(name);
        } else {
            files.push(name);
        }
    }

    folders.sort_by_key(|a| a.to_ascii_lowercase());
    files.sort_by_key(|a| a.to_ascii_lowercase());

    let folder_count = folders.len();
    let file_count = files.len();
    let total = folder_count + file_count;

    let mut items: Vec<FolderEntry> = Vec::with_capacity(total.min(LIST_FOLDER_CAP));
    for name in &folders {
        if items.len() >= LIST_FOLDER_CAP {
            break;
        }
        items.push(FolderEntry {
            name: name.clone(),
            is_dir: true,
            size: None,
        });
    }
    for name in &files {
        if items.len() >= LIST_FOLDER_CAP {
            break;
        }
        let file_size = std::fs::metadata(std::path::Path::new(&path).join(name))
            .ok()
            .map(|m| m.len());
        items.push(FolderEntry {
            name: name.clone(),
            is_dir: false,
            size: file_size,
        });
    }

    Some(FolderListing {
        truncated: total > LIST_FOLDER_CAP,
        items,
        file_count,
        folder_count,
    })
}

const SECS_PER_DAY: u64 = 86400;
const SECS_PER_HOUR: u64 = 3600;
const SECS_PER_MINUTE: u64 = 60;

fn time_from_unix(secs: u64) -> String {
    let days = secs / SECS_PER_DAY;
    let time_secs = secs % SECS_PER_DAY;
    let hours = time_secs / SECS_PER_HOUR;
    let minutes = (time_secs % SECS_PER_HOUR) / SECS_PER_MINUTE;

    let (year, month, day) = civil_from_days(days as i64);
    format!("{year:04}-{month:02}-{day:02} {hours:02}:{minutes:02}")
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picker_guard_allows_only_one_at_a_time() {
        // Nothing else in the crate touches PICKING_FILE, so it starts clear.
        crate::PICKING_FILE.store(false, Ordering::SeqCst);
        let first = PickerGuard::acquire();
        assert!(first.is_some(), "the first picker should be allowed");
        assert!(
            PickerGuard::acquire().is_none(),
            "a second picker while one is open should be refused"
        );
        drop(first);
        assert!(
            PickerGuard::acquire().is_some(),
            "dropping the guard should free the next picker"
        );
    }
}
