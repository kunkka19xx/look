//! Windows autostart via `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
//!
//! Per-user, no admin - matches the `%LOCALAPPDATA%` install convention. The
//! value name is "Look"; the data is the current exe path wrapped in quotes so
//! paths with spaces survive Run's command parsing, then `--hidden` so login
//! starts it in the background.

use windows::Win32::System::Registry::{REG_SZ, RegDeleteValueW};
use windows::core::PCWSTR;

use super::registry::{ERROR_FILE_NOT_FOUND, open_hkcu, read_string, set_string, to_wide};
use crate::autostart::HIDDEN_ARG;

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE_NAME: &str = "Look";

pub(crate) fn set(enabled: bool) -> Result<(), String> {
    let key = open_hkcu(RUN_KEY, enabled)?;
    let value_name = to_wide(VALUE_NAME);

    if enabled {
        let exe = super::install_path::stable_exe()?
            .to_string_lossy()
            .into_owned();
        set_string(&key, VALUE_NAME, &format!("\"{exe}\" {HIDDEN_ARG}"), REG_SZ)?;
    } else {
        let err = unsafe { RegDeleteValueW(key.0, PCWSTR(value_name.as_ptr())) };
        // Already gone counts as removed.
        if err.0 != 0 && err.0 != ERROR_FILE_NOT_FOUND {
            return Err(format!("RegDeleteValueW({VALUE_NAME}) failed: {}", err.0));
        }
    }
    Ok(())
}

pub(crate) fn get() -> bool {
    registered_command().is_some()
}

/// The Run value, if there is one.
pub(crate) fn registered_command() -> Option<String> {
    let key = open_hkcu(RUN_KEY, false).ok()?;
    read_string(&key, VALUE_NAME)
        .ok()?
        .map(|(command, _)| command)
}
