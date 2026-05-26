//! Cross-platform dispatch for the /sys command. Per-OS collectors live under
//! `platform/{linux,windows}/sysinfo.rs`.

use serde::Serialize;

#[derive(Serialize)]
pub struct SysInfoEntry {
    pub label: String,
    pub value: String,
}

#[tauri::command]
pub fn get_system_info() -> Vec<Vec<SysInfoEntry>> {
    #[cfg(target_os = "linux")]
    {
        crate::platform::linux::sysinfo::collect()
    }
    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::sysinfo::collect()
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        Vec::new()
    }
}
