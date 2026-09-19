//! Puts the install directory on the user PATH so `lookapp <mode>` works from a
//! terminal, the way the macOS install symlinks it into `/usr/local/bin`.
//!
//! `HKCU\Environment` only: per-user, no admin, and never touches the machine
//! PATH. The value is read and rewritten through the registry rather than
//! through `%PATH%`: the process environment holds the *merged* machine + user
//! PATH, so writing that back would copy every machine entry into the user's.

use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::System::Registry::{REG_EXPAND_SZ, REG_VALUE_TYPE, RegQueryValueExW};
use windows::Win32::UI::WindowsAndMessaging::{
    HWND_BROADCAST, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_SETTINGCHANGE,
};
use windows::core::PCWSTR;

use super::registry::{OwnedHKey, open_hkcu, set_string, to_wide};

const ENV_KEY: &str = "Environment";
const VALUE_NAME: &str = "Path";
/// Explorer and already-open shells reload the environment on this broadcast.
/// Hung windows must not stall our startup sync, hence the timeout.
const BROADCAST_TIMEOUT_MS: u32 = 1_000;

pub(crate) fn set(enabled: bool) -> Result<(), String> {
    let dir = super::install_path::stable_dir()?
        .to_string_lossy()
        .into_owned();
    let key = open_hkcu(ENV_KEY, true)?;
    let (current, value_type) = read_path(&key)?;
    let next = if enabled {
        with_dir(&current, &dir)
    } else {
        without_dir(&current, &dir)
    };
    if next == current {
        return Ok(());
    }
    set_string(&key, VALUE_NAME, &next, value_type)?;
    broadcast_change();
    Ok(())
}

pub(crate) fn get() -> bool {
    let Ok(dir) = super::install_path::stable_dir() else {
        return false;
    };
    let Ok(key) = open_hkcu(ENV_KEY, false) else {
        return false;
    };
    let Ok((current, _)) = read_path(&key) else {
        return false;
    };
    contains_dir(&current, &dir.to_string_lossy())
}

/// Returns the user PATH and the type to write it back as. A missing value is
/// an empty `REG_EXPAND_SZ`: the entries Windows itself seeds there use
/// `%USERPROFILE%`, so expansion has to survive our rewrite.
fn read_path(key: &OwnedHKey) -> Result<(String, REG_VALUE_TYPE), String> {
    let name = to_wide(VALUE_NAME);
    let mut value_type = REG_VALUE_TYPE::default();
    let mut size: u32 = 0;
    let err = unsafe {
        RegQueryValueExW(
            key.0,
            PCWSTR(name.as_ptr()),
            None,
            Some(&mut value_type),
            None,
            Some(&mut size),
        )
    };
    // ERROR_FILE_NOT_FOUND (2): no user PATH yet.
    if err.0 == 2 {
        return Ok((String::new(), REG_EXPAND_SZ));
    }
    if err.0 != 0 {
        return Err(format!("RegQueryValueExW({VALUE_NAME}) failed: {}", err.0));
    }
    let mut bytes = vec![0u8; size as usize];
    let err = unsafe {
        RegQueryValueExW(
            key.0,
            PCWSTR(name.as_ptr()),
            None,
            Some(&mut value_type),
            Some(bytes.as_mut_ptr()),
            Some(&mut size),
        )
    };
    if err.0 != 0 {
        return Err(format!("RegQueryValueExW({VALUE_NAME}) failed: {}", err.0));
    }
    bytes.truncate(size as usize);
    Ok((decode_wide(&bytes), value_type))
}

fn broadcast_change() {
    let param = to_wide(ENV_KEY);
    unsafe {
        let _ = SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            WPARAM(0),
            LPARAM(param.as_ptr() as isize),
            SMTO_ABORTIFHUNG,
            BROADCAST_TIMEOUT_MS,
            None,
        );
    }
}

fn decode_wide(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .take_while(|unit| *unit != 0)
        .collect();
    String::from_utf16_lossy(&units)
}

/// Case- and trailing-separator-insensitive: `C:\X`, `c:\x\` and `C:\X ` are
/// the same entry to Windows.
fn norm(s: &str) -> String {
    s.trim()
        .trim_end_matches(['\\', '/'])
        .to_lowercase()
        .replace('/', "\\")
}

fn contains_dir(path: &str, dir: &str) -> bool {
    let dir = norm(dir);
    path.split(';').any(|entry| norm(entry) == dir)
}

fn with_dir(path: &str, dir: &str) -> String {
    if contains_dir(path, dir) {
        return path.to_string();
    }
    if path.trim().is_empty() {
        return dir.to_string();
    }
    // Trailing `;` is legal and common; appending after one would leave an
    // empty entry, which Windows reads as "the current directory".
    format!("{};{dir}", path.trim_end_matches(';'))
}

fn without_dir(path: &str, dir: &str) -> String {
    let dir = norm(dir);
    path.split(';')
        .filter(|entry| norm(entry) != dir)
        .collect::<Vec<_>>()
        .join(";")
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIR: &str = r"C:\Users\me\AppData\Local\Programs\Look";

    #[test]
    fn appends_once() {
        let path = r"C:\Windows;C:\Windows\System32";
        let added = with_dir(path, DIR);
        assert_eq!(added, format!("{path};{DIR}"));
        assert_eq!(with_dir(&added, DIR), added);
    }

    #[test]
    fn matches_case_and_trailing_separator() {
        let path = r"C:\Windows;c:\users\me\appdata\local\programs\look\";
        assert!(contains_dir(path, DIR));
        assert_eq!(with_dir(path, DIR), path);
        assert_eq!(without_dir(path, DIR), r"C:\Windows");
    }

    #[test]
    fn no_empty_entry_after_trailing_semicolon() {
        assert_eq!(with_dir(r"C:\Windows;", DIR), format!(r"C:\Windows;{DIR}"));
        assert_eq!(with_dir("", DIR), DIR);
    }

    #[test]
    fn removes_only_our_entry() {
        let path = format!(r"C:\Windows;{DIR};C:\Tools");
        assert_eq!(without_dir(&path, DIR), r"C:\Windows;C:\Tools");
        assert_eq!(without_dir(r"C:\Windows", DIR), r"C:\Windows");
    }

    #[test]
    fn leaves_expandable_entries_alone() {
        let path = r"%USERPROFILE%\bin;C:\Windows";
        assert_eq!(with_dir(path, DIR), format!(r"{path};{DIR}"));
    }
}
