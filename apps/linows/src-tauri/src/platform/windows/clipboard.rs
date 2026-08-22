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
//!
//! CF_HDROP is invisible to text targets, so "Copy path" would paste nothing
//! into an editor. The same paths go on as CF_UNICODETEXT too. macOS gets this
//! for free: an `NSURL` on the pasteboard carries its own string form.

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
const CF_UNICODETEXT: u32 = 13;
// One path per line, the separator every Windows text target expects.
const TEXT_SEPARATOR: &str = "\r\n";

pub(crate) fn copy_files(paths: &[String]) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }

    // CF_HDROP wants Win32 paths with backslashes. Frontend hands us forward
    // slashes (engine normalizes paths that way); Explorer paste silently
    // does nothing if any path has a wrong separator.
    let native: Vec<String> = paths.iter().map(|p| p.replace('/', "\\")).collect();

    unsafe {
        // HWND::default() = null is a valid clipboard owner (system-wide handoff).
        OpenClipboard(Some(HWND::default())).map_err(|e| format!("OpenClipboard failed: {e}"))?;

        // Wrap so we always close, even on early return.
        let result = (|| -> Result<(), String> {
            EmptyClipboard().map_err(|e| format!("EmptyClipboard failed: {e}"))?;
            set_hdrop(&native)?;
            // Best-effort: the files are already on the clipboard, and losing
            // the text form is not worth failing a copy that did happen.
            if let Err(err) = set_text(&native.join(TEXT_SEPARATOR)) {
                eprintln!("[clipboard] path text unavailable: {err}");
            }
            Ok(())
        })();

        let _ = CloseClipboard();
        result
    }
}

/// The shell's file list: a DROPFILES header followed by null-terminated UTF-16
/// paths, capped by one more null.
unsafe fn set_hdrop(paths: &[String]) -> Result<(), String> {
    let utf16: Vec<Vec<u16>> = paths
        .iter()
        .map(|p| {
            let mut s: Vec<u16> = p.encode_utf16().collect();
            s.push(0); // per-path null terminator
            s
        })
        .collect();

    let drop_size = std::mem::size_of::<DROPFILES>();
    let paths_bytes: usize = utf16.iter().map(|s| s.len() * 2).sum();
    let trailing_null = 2; // u16 null caps the list
    unsafe {
        set_clipboard_data(CF_HDROP, drop_size + paths_bytes + trailing_null, |ptr| {
            // fWide = 1 → paths are UTF-16; pFiles = offset (bytes) from the
            // start of DROPFILES to the path list. The rest of the header is
            // POINT/BOOL fields the zero-init already left at 0/FALSE.
            let dropfiles = ptr as *mut DROPFILES;
            (*dropfiles).pFiles = drop_size as u32;
            (*dropfiles).fWide = true.into();

            let mut cursor = ptr.add(drop_size) as *mut u16;
            for path in &utf16 {
                std::ptr::copy_nonoverlapping(path.as_ptr(), cursor, path.len());
                cursor = cursor.add(path.len());
            }
        })
    }
}

/// The same paths as plain text, for every target that cannot read CF_HDROP.
unsafe fn set_text(text: &str) -> Result<(), String> {
    let utf16: Vec<u16> = text.encode_utf16().collect();
    // Zero-init supplies the terminator the format requires.
    let terminator = 2;
    unsafe {
        set_clipboard_data(CF_UNICODETEXT, utf16.len() * 2 + terminator, |ptr| {
            std::ptr::copy_nonoverlapping(utf16.as_ptr(), ptr as *mut u16, utf16.len());
        })
    }
}

/// Hands one format to the clipboard, which takes ownership of the allocation
/// on success. `fill` writes into `size` bytes of zeroed, suitably aligned
/// memory.
unsafe fn set_clipboard_data(
    format: u32,
    size: usize,
    fill: impl FnOnce(*mut u8),
) -> Result<(), String> {
    unsafe {
        // GHND = GMEM_MOVEABLE | GMEM_ZEROINIT.
        let hmem = GlobalAlloc(GHND, size).map_err(|e| format!("GlobalAlloc failed: {e}"))?;
        if hmem.is_invalid() {
            return Err("GlobalAlloc returned null".to_string());
        }

        let ptr = GlobalLock(hmem);
        if ptr.is_null() {
            let _ = GlobalFree(Some(hmem));
            return Err("GlobalLock returned null".to_string());
        }

        fill(ptr as *mut u8);

        let _ = GlobalUnlock(hmem); // returns BOOL; non-zero failure expected here

        // On success the clipboard owns hmem - DON'T free. On failure we must
        // free or leak the global handle.
        match SetClipboardData(format, Some(HANDLE(hmem.0))) {
            Ok(_) => Ok(()),
            Err(e) => {
                let _ = GlobalFree(Some(hmem));
                Err(format!("SetClipboardData failed: {e}"))
            }
        }
    }
}
