//! Cross-platform autostart. Real per-OS implementations live in
//! `platform::linux::autostart` and `platform::windows::autostart`.

use std::path::Path;

#[cfg(target_os = "linux")]
use crate::platform::linux::autostart as os;
#[cfg(target_os = "windows")]
use crate::platform::windows::autostart as os;

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
mod os {
    pub(crate) fn set(_enabled: bool) -> Result<(), String> {
        Ok(())
    }

    pub(crate) fn get() -> bool {
        false
    }

    pub(crate) fn registered_command() -> Option<String> {
        None
    }
}

/// Passed by the login entry: start in the background, no window until the
/// hotkey asks for one.
pub const HIDDEN_ARG: &str = "--hidden";

/// Debug builds never register: the entry would point at `target/debug` and
/// the next login would start the dev build. Turning it off still goes
/// through.
pub fn set_autostart(enabled: bool) -> Result<(), String> {
    if enabled && cfg!(debug_assertions) {
        return Ok(());
    }
    os::set(enabled)
}

pub fn get_autostart() -> bool {
    os::get()
}

/// Drops a login entry that starts a binary from this one's directory. Dev
/// builds call it at startup: one that registered itself before the guard in
/// `set_autostart` would otherwise come back at every login.
pub fn unregister_own_dir() {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let own = exe.parent().is_some_and(|dir| {
        os::registered_command().is_some_and(|command| command_in_dir(&command, dir))
    });
    if own {
        let _ = os::set(false);
    }
}

/// `command` is the entry as registered: the binary, quoted when its path
/// needs it, then arguments. Windows paths compare without case.
fn command_in_dir(command: &str, dir: &Path) -> bool {
    let command = command.trim_start_matches('"');
    if cfg!(windows) {
        Path::new(&command.to_lowercase()).starts_with(dir.to_string_lossy().to_lowercase())
    } else {
        Path::new(command).starts_with(dir)
    }
}

#[cfg(test)]
mod tests {
    use super::command_in_dir;
    use std::path::Path;

    const DIR: &str = "/home/u/my look/target/debug";

    #[test]
    fn matches_binaries_in_the_dir() {
        let dir = Path::new(DIR);
        assert!(command_in_dir(&format!("{DIR}/lookapp"), dir));
        assert!(command_in_dir(
            &format!("\"{DIR}/lookapp-tauri\" --hidden"),
            dir
        ));
    }

    #[test]
    fn ignores_other_dirs() {
        let dir = Path::new(DIR);
        assert!(!command_in_dir("/usr/bin/lookapp --hidden", dir));
        assert!(!command_in_dir(&format!("{DIR}-old/lookapp"), dir));
    }
}
