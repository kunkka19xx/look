//! Durability helpers for the small JSON stores: atomic replace (write a temp
//! file, then rename), and corrupt-file sidelining so a bad file is kept for
//! recovery instead of being silently overwritten as empty by the next save.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};

use serde::de::DeserializeOwned;

/// Serializes the load-modify-write of one store file. The macOS shell only
/// touches these from the main actor today, but the FFI is callable from any
/// thread and a second shell (linows) shares the format, so an interleaved
/// read-modify-write would silently drop conversations.
///
/// One global lock rather than per-path: there are two small stores, both
/// written rarely (on turn completion), so contention is irrelevant and a
/// path-keyed map would be more machinery than the problem deserves.
pub(crate) fn write_guard() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

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
    // Unique per writer, not just per process: two threads writing the same
    // store would otherwise share one temp path and clobber each other.
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let tmp = suffixed(
        path,
        &format!(
            ".{}.{}.tmp",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ),
    );
    if fs::write(&tmp, data).is_err() {
        let _ = fs::remove_file(&tmp);
        return false;
    }
    if fs::rename(&tmp, path).is_ok() {
        return true;
    }
    // Every write mints a fresh temp name, so a failing rename (permissions, a
    // full disk, a path that vanished) would otherwise litter one file per
    // attempt next to the store.
    let _ = fs::remove_file(&tmp);
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_rename_leaves_no_temp_file() {
        // Renaming onto a directory fails, so this exercises the cleanup path.
        let dir = std::env::temp_dir().join(format!("look-store-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("make dir");

        assert!(
            !write_atomic(&dir, b"payload"),
            "rename onto a dir must fail"
        );

        let leftovers: Vec<_> = fs::read_dir(dir.parent().expect("parent"))
            .expect("read temp dir")
            .filter_map(Result::ok)
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with(&format!("look-store-test-{}", std::process::id()))
                    && e.file_name().to_string_lossy().ends_with(".tmp")
            })
            .collect();
        assert!(leftovers.is_empty(), "temp files left: {leftovers:?}");

        let _ = fs::remove_dir_all(&dir);
    }
}

fn suffixed(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(suffix);
    path.with_file_name(name)
}
