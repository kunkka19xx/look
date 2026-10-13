//! Shared HKCU registry plumbing for the per-user integrations (autostart, PATH).

use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_VALUE_TYPE, RegCloseKey, RegOpenKeyExW,
    RegSetValueExW,
};
use windows::core::PCWSTR;

pub(crate) struct OwnedHKey(pub(crate) HKEY);

impl Drop for OwnedHKey {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}

/// Both keys we touch are created by Windows itself and always present, so
/// plain RegOpenKeyExW is enough, no need for the Ex-Create variant (which
/// would drag in SECURITY_ATTRIBUTES).
pub(crate) fn open_hkcu(subkey: &str, write: bool) -> Result<OwnedHKey, String> {
    let wide = to_wide(subkey);
    let access = if write {
        KEY_READ | KEY_WRITE
    } else {
        KEY_READ
    };
    let mut hkey = HKEY::default();
    let err = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(wide.as_ptr()),
            None,
            access,
            &mut hkey,
        )
    };
    if err.0 != 0 {
        return Err(format!("RegOpenKeyExW({subkey}) failed: {}", err.0));
    }
    Ok(OwnedHKey(hkey))
}

/// Writes a string value; `value_type` is REG_SZ or REG_EXPAND_SZ. The data is
/// the UTF-16 string including its trailing null.
pub(crate) fn set_string(
    key: &OwnedHKey,
    name: &str,
    value: &str,
    value_type: REG_VALUE_TYPE,
) -> Result<(), String> {
    let name_w = to_wide(name);
    let wide = to_wide(value);
    let bytes = unsafe { std::slice::from_raw_parts(wide.as_ptr() as *const u8, wide.len() * 2) };
    let err = unsafe {
        RegSetValueExW(
            key.0,
            PCWSTR(name_w.as_ptr()),
            None,
            value_type,
            Some(bytes),
        )
    };
    if err.0 != 0 {
        return Err(format!("RegSetValueExW({name}) failed: {}", err.0));
    }
    Ok(())
}

pub(crate) fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
