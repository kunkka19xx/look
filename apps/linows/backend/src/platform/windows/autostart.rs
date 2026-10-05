//! Windows autostart via `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
//!
//! Per-user, no admin - matches the `%LOCALAPPDATA%` install convention. The
//! value name is "Look"; the data is the current exe path wrapped in quotes so
//! paths with spaces survive Run's command parsing.

use windows::Win32::System::Registry::{REG_SZ, RegDeleteValueW, RegQueryValueExW};
use windows::core::PCWSTR;

use super::registry::{open_hkcu, set_string, to_wide};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE_NAME: &str = "Look";

pub(crate) fn set(enabled: bool) -> Result<(), String> {
    let key = open_hkcu(RUN_KEY, enabled)?;
    let value_name = to_wide(VALUE_NAME);

    if enabled {
        let exe = super::install_path::stable_exe()?
            .to_string_lossy()
            .into_owned();
        set_string(&key, VALUE_NAME, &format!("\"{exe}\""), REG_SZ)?;
    } else {
        let err = unsafe { RegDeleteValueW(key.0, PCWSTR(value_name.as_ptr())) };
        // ERROR_FILE_NOT_FOUND (2) - value already gone, treat as success.
        if err.0 != 0 && err.0 != 2 {
            return Err(format!("RegDeleteValueW({VALUE_NAME}) failed: {}", err.0));
        }
    }
    Ok(())
}

pub(crate) fn get() -> bool {
    let Ok(key) = open_hkcu(RUN_KEY, false) else {
        return false;
    };
    let value_name = to_wide(VALUE_NAME);
    let err =
        unsafe { RegQueryValueExW(key.0, PCWSTR(value_name.as_ptr()), None, None, None, None) };
    err.0 == 0
}
