//! Move-to-Trash + Empty-Trash IPC. Empty-trash is Linux-only: the `trash`
//! crate's `os_limited` module isn't implemented on Windows.

use std::path::{Path, PathBuf};

#[cfg(target_os = "linux")]
const HOME_PIN_SUFFIXES: &[&str] = &[
    "Desktop",
    "Documents",
    "Downloads",
    "Pictures",
    "Videos",
    "Music",
    "Public",
    "Templates",
];
#[cfg(target_os = "windows")]
const HOME_PIN_SUFFIXES: &[&str] = &[
    "Desktop",
    "Documents",
    "Downloads",
    "Pictures",
    "Videos",
    "Music",
];

#[cfg(target_os = "linux")]
pub fn linux_trash_dir() -> Option<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        let trimmed = xdg.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed).join("Trash"));
        }
    }
    let home = std::env::var("HOME").ok().filter(|s| !s.is_empty())?;
    Some(PathBuf::from(home).join(".local/share/Trash"))
}

#[cfg(target_os = "linux")]
fn is_inside_trash_dir(path: &Path) -> bool {
    let Some(trash) = linux_trash_dir() else {
        return false;
    };
    path == trash || path.starts_with(&trash)
}

#[cfg(not(target_os = "linux"))]
fn is_inside_trash_dir(_path: &Path) -> bool {
    false
}

fn home_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var("USERPROFILE")
            .ok()
            .filter(|v| !v.is_empty())
            .or_else(|| std::env::var("HOME").ok().filter(|v| !v.is_empty()))
            .map(PathBuf::from)
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::env::var("HOME")
            .ok()
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    }
}

fn is_safe_to_trash(path: &Path) -> Result<(), String> {
    let path_str = path.to_string_lossy();
    if path_str.trim().is_empty() {
        return Err("empty path".into());
    }
    if path_str.contains("://") {
        return Err("not a filesystem path".into());
    }
    if path == Path::new("/") {
        return Err("cannot trash filesystem root".into());
    }
    #[cfg(target_os = "windows")]
    if path_str.len() <= 3 && path_str.ends_with(":\\") {
        return Err("cannot trash drive root".into());
    }
    if let Some(home) = home_dir() {
        if path == home {
            return Err("cannot trash home directory".into());
        }
        for suffix in HOME_PIN_SUFFIXES {
            if path == home.join(suffix) {
                return Err(format!("cannot trash home shortcut: {suffix}"));
            }
        }
    }
    if is_inside_trash_dir(path) {
        return Err("cannot trash items already in Trash".into());
    }
    if !path.exists() {
        return Err("path no longer exists".into());
    }
    Ok(())
}

#[derive(serde::Serialize)]
pub struct TrashOutcome {
    pub trashed: usize,
    pub failed: Vec<TrashFailure>,
}

#[derive(serde::Serialize)]
pub struct TrashFailure {
    pub path: String,
    pub reason: String,
}

#[tauri::command]
pub fn trash_paths(paths: Vec<String>) -> TrashOutcome {
    let mut trashed = 0usize;
    let mut failed: Vec<TrashFailure> = Vec::new();
    for raw in paths {
        let path = PathBuf::from(&raw);
        if let Err(reason) = is_safe_to_trash(&path) {
            failed.push(TrashFailure { path: raw, reason });
            continue;
        }
        match trash::delete(&path) {
            Ok(()) => trashed += 1,
            Err(err) => failed.push(TrashFailure {
                path: raw,
                reason: err.to_string(),
            }),
        }
    }
    TrashOutcome { trashed, failed }
}

#[tauri::command]
pub fn count_trash_items() -> Result<usize, String> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        trash::os_limited::list()
            .map(|items| items.len())
            .map_err(|err| err.to_string())
    }
    #[cfg(target_os = "windows")]
    {
        Err("not supported on Windows".into())
    }
}

#[tauri::command]
pub fn empty_trash() -> Result<usize, String> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let items = trash::os_limited::list().map_err(|err| err.to_string())?;
        let count = items.len();
        if count == 0 {
            return Ok(0);
        }
        trash::os_limited::purge_all(items).map_err(|err| err.to_string())?;
        Ok(count)
    }
    #[cfg(target_os = "windows")]
    {
        Err("not supported on Windows".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_empty_url_and_root() {
        assert!(is_safe_to_trash(Path::new("")).is_err());
        assert!(is_safe_to_trash(Path::new("settings://network")).is_err());
        assert!(is_safe_to_trash(Path::new("https://example.com")).is_err());
        assert!(is_safe_to_trash(Path::new("/")).is_err());
    }

    #[test]
    fn refuses_home_and_pins() {
        let Some(home) = home_dir() else {
            return;
        };
        assert!(is_safe_to_trash(&home).is_err());
        for suffix in HOME_PIN_SUFFIXES {
            assert!(is_safe_to_trash(&home.join(suffix)).is_err());
        }
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn refuses_paths_inside_trash() {
        let Some(trash) = linux_trash_dir() else {
            return;
        };
        assert!(is_safe_to_trash(&trash).is_err());
        assert!(is_safe_to_trash(&trash.join("files/some-old-file.txt")).is_err());
    }
}
