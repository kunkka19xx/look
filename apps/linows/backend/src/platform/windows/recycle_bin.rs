//! Recycle Bin query + empty via the Win32 Shell API.
//!
//! The `trash` crate's `os_limited` (list / purge) isn't implemented on
//! Windows, so counting and emptying go through `SHQueryRecycleBinW` /
//! `SHEmptyRecycleBinW` directly. A null root path targets the recycle bins of
//! every drive at once, matching Explorer's "Empty Recycle Bin".

use windows::Win32::UI::Shell::{
    SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND, SHEmptyRecycleBinW, SHQUERYRBINFO,
    SHQueryRecycleBinW,
};
use windows::core::PCWSTR;

/// Total number of items across every drive's recycle bin.
pub(crate) fn count() -> Result<usize, String> {
    let mut info = SHQUERYRBINFO {
        cbSize: std::mem::size_of::<SHQUERYRBINFO>() as u32,
        i64Size: 0,
        i64NumItems: 0,
    };
    unsafe {
        SHQueryRecycleBinW(PCWSTR::null(), &mut info).map_err(|err| err.to_string())?;
    }
    Ok(info.i64NumItems.max(0) as usize)
}

/// Empty every drive's recycle bin without confirmation, progress UI, or sound.
/// Returns the number of items that were present before emptying.
pub(crate) fn empty() -> Result<usize, String> {
    let count = count()?;
    if count == 0 {
        return Ok(0);
    }
    unsafe {
        SHEmptyRecycleBinW(
            None,
            PCWSTR::null(),
            SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND,
        )
        .map_err(|err| err.to_string())?;
    }
    Ok(count)
}
