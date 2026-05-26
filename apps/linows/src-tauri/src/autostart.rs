//! Cross-platform autostart Tauri commands. Real per-OS implementations live in
//! `platform::linux::autostart` and `platform::windows::autostart`.

#[tauri::command]
pub fn set_autostart(enabled: bool) -> Result<(), String> {
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

#[tauri::command]
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
