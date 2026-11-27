//! Cross-platform autostart. Real per-OS implementations live in
//! `platform::linux::autostart` and `platform::windows::autostart`.
/// Windows debug builds never register: the Run entry would point at
/// `target\debug`, and the next login would start a dev build (dev config and
/// db) instead of the installed one. Settings saves reach here too, not only
/// `sync_integrations`.
pub fn set_autostart(enabled: bool) -> Result<(), String> {
    if cfg!(all(windows, debug_assertions)) {
        let _ = enabled;
        return Ok(());
    }
    #[cfg(target_os = "linux")]
    {
        crate::platform::linux::autostart::set(enabled)
    }

    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::autostart::set(enabled)
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = enabled;
        Ok(())
    }
}
pub fn get_autostart() -> bool {
    #[cfg(target_os = "linux")]
    {
        crate::platform::linux::autostart::get()
    }

    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::autostart::get()
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        false
    }
}
