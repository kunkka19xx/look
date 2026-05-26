//! Enumerate fixed drives on Windows so the settings UI can offer them as
//! one-click scan-root targets. Mirrors `DriveDiscoveryService` from the
//! WinUI3 reference (apps/windows/LauncherApp/Services/DriveDiscoveryService.cs).

use serde::Serialize;
use std::path::Path;

#[derive(Serialize, Clone)]
pub struct CandidateDrive {
    /// "D", "E", ... (single uppercase letter).
    pub letter: String,
    /// "D:\\".
    pub root: String,
}

/// Returns fixed drives other than C: that exist and are readable.
/// The system drive is excluded because users get `~/Desktop`, `~/Documents`,
/// `~/Downloads` for free via the default file_scan_roots.
pub(crate) fn enumerate_candidates() -> Vec<CandidateDrive> {
    let mut out = Vec::new();
    for ch in b'A'..=b'Z' {
        if ch == b'C' {
            continue;
        }
        let letter = char::from(ch).to_string();
        let root = format!("{letter}:\\");
        // Path::new(&root).exists() returns true for drives that are mounted
        // AND the current process can stat. Removable media without a disk
        // (empty card readers, etc.) fails this check, which is what we want.
        if Path::new(&root).exists() {
            out.push(CandidateDrive { letter, root });
        }
    }
    out
}
