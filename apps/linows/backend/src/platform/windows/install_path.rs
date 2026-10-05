//! Where the install lives, as recorded outside the process (autostart entry,
//! PATH). Package managers like Scoop install to `<app>\<version>\` and keep a
//! `current` junction pointing at the active version, so the versioned path
//! goes stale on the next upgrade. Prefer the junction whenever one resolves
//! to our own directory.

use std::path::{Path, PathBuf};

const VERSION_LINK_DIR: &str = "current";

pub(crate) fn stable_exe() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    Ok(version_link_exe(&exe).unwrap_or(exe))
}

pub(crate) fn stable_dir() -> Result<PathBuf, String> {
    let exe = stable_exe()?;
    exe.parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| format!("Missing install directory for {}", exe.display()))
}

fn version_link_exe(exe: &Path) -> Option<PathBuf> {
    let dir = exe.parent()?;
    let file = exe.file_name()?;
    let linked = dir.parent()?.join(VERSION_LINK_DIR);
    let linked_exe = linked.join(file);
    (linked_exe.is_file() && resolves_to_same_dir(&linked, dir)).then_some(linked_exe)
}

/// Canonicalisation resolves reparse points, so a junction and its target
/// compare equal while a plain directory that happens to be named `current`
/// does not.
fn resolves_to_same_dir(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}
