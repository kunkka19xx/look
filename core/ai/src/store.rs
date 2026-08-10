//! Durability helpers for the small JSON stores: atomic replace (write a temp
//! file, then rename), and corrupt-file sidelining so a bad file is kept for
//! recovery instead of being silently overwritten as empty by the next save.

use std::fs;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;

/// Load a JSON list. A file that exists but does not parse is moved aside to
/// `<name>.corrupt` so a later save cannot destroy it.
pub(crate) fn load_list<T: DeserializeOwned>(path: &Path) -> Vec<T> {
    let Ok(data) = fs::read_to_string(path) else {
        return Vec::new();
    };
    match serde_json::from_str(&data) {
        Ok(list) => list,
        Err(_) => {
            let _ = fs::rename(path, suffixed(path, ".corrupt"));
            Vec::new()
        }
    }
}

/// Write via a temp file in the same directory + rename, so a crash mid-write
/// leaves the previous file intact instead of a truncated one.
pub(crate) fn write_atomic(path: &Path, data: &[u8]) -> bool {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let tmp = suffixed(path, &format!(".{}.tmp", std::process::id()));
    if fs::write(&tmp, data).is_err() {
        let _ = fs::remove_file(&tmp);
        return false;
    }
    fs::rename(&tmp, path).is_ok()
}

fn suffixed(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(suffix);
    path.with_file_name(name)
}
