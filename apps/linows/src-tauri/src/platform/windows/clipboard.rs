//! Windows file-clipboard via the shell's CF_HDROP format.
//!
//! Used by Ctrl+P / Ctrl+C in the launcher so picked files can be pasted into
//! Explorer or any other shell-aware target. Mirrors the macOS path on
//! `NSPasteboard.writeObjects([NSURL])` and the Linux `xclip` /
//! `wl-copy --type "text/uri-list"` fallback in `platform/linux/clipboard.rs`.
//!
//! Layout of the clipboard payload:
//!
//! ```text
//! +---------------------------+
//! | DROPFILES { pFiles, … }   |  pFiles = sizeof(DROPFILES)
//! +---------------------------+
//! | path1\0path2\0…\0         |  UTF-16, each path null-terminated
//! +---------------------------+
//! | \0                        |  extra null marks end of list
//! +---------------------------+
//! ```
//!
//! Allocated with `GMEM_MOVEABLE` because `SetClipboardData` takes ownership -
//! we must NOT free on success, but MUST free on failure (otherwise the
//! allocation leaks across the process).

use windows::Win32::Foundation::{GlobalFree, HANDLE, HWND};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GHND, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::UI::Shell::DROPFILES;

// CF_HDROP. Defined in winuser.h as `15`; the windows crate exposes it via
// Win32_System_Ole / Win32_System_SystemServices, but it's just a constant -
// we use the raw value to keep the feature surface tight.
const CF_HDROP: u32 = 15;

pub(crate) fn copy_files(paths: &[String]) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }

    // CF_HDROP wants Win32 paths with backslashes. Frontend hands us forward
    // slashes (engine normalizes paths that way); Explorer paste silently
    // does nothing if any path has a wrong separator.
    let utf16_paths: Vec<Vec<u16>> = paths
        .iter()
        .map(|p| {
            let mut s: Vec<u16> = p.replace('/', "\\").encode_utf16().collect();
            s.push(0); // per-path null terminator
            s
        })
        .collect();

    let drop_size = std::mem::size_of::<DROPFILES>();
    let paths_bytes: usize = utf16_paths.iter().map(|s| s.len() * 2).sum();
    let trailing_null = 2; // u16 null caps the list
    let total = drop_size + paths_bytes + trailing_null;

    unsafe {
        // HWND::default() = null is a valid clipboard owner (system-wide handoff).
        OpenClipboard(Some(HWND::default())).map_err(|e| format!("OpenClipboard failed: {e}"))?;

        // Wrap so we always close, even on early return.
        let result = (|| -> Result<(), String> {
            EmptyClipboard().map_err(|e| format!("EmptyClipboard failed: {e}"))?;

            // GHND = GMEM_MOVEABLE | GMEM_ZEROINIT. Zero-init lets us skip
            // writing the POINT/BOOL fields of DROPFILES (they're already 0/FALSE).
            let hmem = GlobalAlloc(GHND, total).map_err(|e| format!("GlobalAlloc failed: {e}"))?;
            if hmem.is_invalid() {
                return Err("GlobalAlloc returned null".to_string());
            }

            let ptr = GlobalLock(hmem);
            if ptr.is_null() {
                let _ = GlobalFree(Some(hmem));
                return Err("GlobalLock returned null".to_string());
            }

            // Write DROPFILES header. fWide = 1 → paths are UTF-16; pFiles =
            // offset (bytes) from the start of DROPFILES to the path list.
            let dropfiles = ptr as *mut DROPFILES;
            (*dropfiles).pFiles = drop_size as u32;
            (*dropfiles).fWide = true.into();

            // Write each UTF-16 path immediately after the header. The buffer
            // is zero-initialized, so the trailing extra-null is already there.
            let mut cursor = (ptr as *mut u8).add(drop_size) as *mut u16;
            for path in &utf16_paths {
                std::ptr::copy_nonoverlapping(path.as_ptr(), cursor, path.len());
                cursor = cursor.add(path.len());
            }

            let _ = GlobalUnlock(hmem); // returns BOOL; non-zero failure expected here

            // On success the clipboard owns hmem - DON'T free. On failure we
            // must free or leak the global handle.
            match SetClipboardData(CF_HDROP, Some(HANDLE(hmem.0))) {
                Ok(_) => Ok(()),
                Err(e) => {
                    let _ = GlobalFree(Some(hmem));
                    Err(format!("SetClipboardData failed: {e}"))
                }
            }
        })();

        let _ = CloseClipboard();
        result
    }
}
