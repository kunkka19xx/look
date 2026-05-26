//! Windows autostart via `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
//!
//! Per-user, no admin — matches the WinUI3 reference and `%LOCALAPPDATA%`
//! install convention. The value name is "Look"; the data is the current
//! exe path wrapped in quotes so paths with spaces survive Run's command
//! parsing.

use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_SZ, RegCloseKey, RegDeleteValueW,
    RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
};
use windows::core::PCWSTR;

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE_NAME: &str = "Look";

pub(crate) fn set(enabled: bool) -> Result<(), String> {
    let key = open_run_key(enabled)?;
    let value_name = to_wide(VALUE_NAME);

    if enabled {
        let exe = std::env::current_exe()
            .map_err(|e| format!("current_exe: {e}"))?
            .to_string_lossy()
            .into_owned();
        let quoted = format!("\"{exe}\"");
        let value_wide = to_wide(&quoted);
        // REG_SZ wants the UTF-16 string including its trailing null. The
        // byte slice points at the wide buffer; len is units * 2.
        let bytes = unsafe {
            std::slice::from_raw_parts(value_wide.as_ptr() as *const u8, value_wide.len() * 2)
        };
        let err = unsafe {
            RegSetValueExW(
                key.0,
                PCWSTR(value_name.as_ptr()),
                None,
                REG_SZ,
                Some(bytes),
            )
        };
        if err.0 != 0 {
            return Err(format!("RegSetValueExW({VALUE_NAME}) failed: {}", err.0));
        }
    } else {
        let err = unsafe { RegDeleteValueW(key.0, PCWSTR(value_name.as_ptr())) };
        // ERROR_FILE_NOT_FOUND (2) — value already gone, treat as success.
        if err.0 != 0 && err.0 != 2 {
            return Err(format!("RegDeleteValueW({VALUE_NAME}) failed: {}", err.0));
        }
    }
    Ok(())
}

pub(crate) fn get() -> bool {
    let Ok(key) = open_run_key(false) else {
        return false;
    };
    let value_name = to_wide(VALUE_NAME);
    let err =
        unsafe { RegQueryValueExW(key.0, PCWSTR(value_name.as_ptr()), None, None, None, None) };
    err.0 == 0
}

struct OwnedHKey(HKEY);

impl Drop for OwnedHKey {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}

fn open_run_key(write: bool) -> Result<OwnedHKey, String> {
    // HKCU\…\Run is created by Windows itself and always present, so plain
    // RegOpenKeyExW is enough — no need for the Ex-Create variant (which
    // would drag in the Win32_Security feature for SECURITY_ATTRIBUTES).
    let subkey = to_wide(RUN_KEY);
    let access = if write { KEY_WRITE } else { KEY_READ };
    let mut hkey = HKEY::default();
    let err = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            None,
            access,
            &mut hkey,
        )
    };
    if err.0 != 0 {
        return Err(format!("RegOpenKeyExW({RUN_KEY}) failed: {}", err.0));
    }
    Ok(OwnedHKey(hkey))
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
