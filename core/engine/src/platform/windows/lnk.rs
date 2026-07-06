//! Resolve a `.lnk` file's target executable path via the Shell COM API.
//!
//! Used by `apps.rs` to dedupe the fallback executable scan: when a Start
//! Menu shortcut points to `C:\Program Files\X\app.exe`, we don't want the
//! fallback walk to emit the same `app.exe` again as a separate candidate.

use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, IPersistFile,
    STGM_READ,
};
use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
use windows::core::{HSTRING, Interface};

/// Read the absolute target path from a `.lnk` shortcut. Returns None when
/// the file isn't a valid shell link, isn't readable, or has no target
/// (rare - most .lnk files do).
pub(crate) fn resolve_target(lnk_path: &str) -> Option<String> {
    unsafe {
        // Idempotent across calls; RPC_E_CHANGED_MODE is harmless if another
        // crate (notably the icon resolver) already CoInit'd this thread.
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
        let persist: IPersistFile = link.cast().ok()?;
        persist.Load(&HSTRING::from(lnk_path), STGM_READ).ok()?;

        // MAX_PATH = 260 wide chars. SLGP_RAWPATH = 4 returns the path as
        // stored, skipping environment expansion - fine because Windows
        // shortcuts always store absolute paths.
        let mut buf = [0u16; 260];
        link.GetPath(&mut buf, std::ptr::null_mut(), 0).ok()?;
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        if len == 0 {
            return None;
        }
        Some(String::from_utf16_lossy(&buf[..len]))
    }
}

/// Normalize a Windows file path for case-insensitive dedup comparison.
/// Collapses `/` ↔ `\`, lowercases, and strips trailing separators.
pub(crate) fn normalize_for_compare(path: &str) -> String {
    path.replace('/', "\\").to_lowercase()
}
