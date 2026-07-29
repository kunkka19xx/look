//! Elevated launch via the shell `runas` verb, which triggers the UAC prompt.
//! `open::that` uses the default `open` verb; only `ShellExecuteW` with `runas`
//! requests elevation for the target.

use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{HSTRING, PCWSTR};

/// Launch `path` elevated. Returns `Err` when the user declines the UAC prompt
/// (ERROR_CANCELLED) or the shell fails to start the target.
pub fn run_as_admin(path: &str) -> Result<(), String> {
    let verb = HSTRING::from("runas");
    let file = HSTRING::from(path);
    // ShellExecuteW returns an HINSTANCE that is only an error indicator: a value
    // greater than 32 means success, anything else is a failure code.
    let hinst = unsafe {
        ShellExecuteW(
            None,
            &verb,
            &file,
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    if hinst.0 as isize > 32 {
        Ok(())
    } else {
        Err(format!("runas failed (code {})", hinst.0 as isize))
    }
}
