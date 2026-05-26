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

// SLGP_RAWPATH flag for IShellLinkW::GetPath — returns the path as stored in
// the shortcut without environment-variable expansion.
const SLGP_RAWPATH: u32 = 0x04;

/// Windows long-path support lets paths reach ~32K UTF-16 code units.
/// We allocate a heap buffer large enough to hold any valid path the
/// Shell API returns, which also avoids the historic 260-char (MAX_PATH)
/// stack buffer that silently truncated long paths.
const LNK_BUF_LEN: usize = 32767;

/// Read the absolute target path from a `.lnk` shortcut. Returns None when
/// the file isn't a valid shell link, isn't readable, or has no target
/// (rare — most .lnk files do).
///
/// Uses `IShellLinkW::GetPath` with `SLGP_RAWPATH` to get the stored path
/// without environment-variable expansion. The buffer is heap-allocated
/// (32767 UTF-16 code units) to support long paths beyond MAX_PATH.
pub(crate) fn resolve_target(lnk_path: &str) -> Option<String> {
    unsafe {
        // Idempotent across calls; RPC_E_CHANGED_MODE is harmless if another
        // crate (notably the icon resolver) already CoInit'd this thread.
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
        let persist: IPersistFile = link.cast().ok()?;
        persist.Load(&HSTRING::from(lnk_path), STGM_READ).ok()?;

        let mut buf = vec![0u16; LNK_BUF_LEN];
        link.GetPath(&mut buf, std::ptr::null_mut(), SLGP_RAWPATH)
            .ok()?;

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
