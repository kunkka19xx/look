//! Cross-platform process listing / kill Tauri commands. Real per-OS
//! implementations live in `platform::linux::process` and
//! `platform::windows::process`.

use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct RunningApp {
    pub name: String,
    pub pid: u32,
    pub desktop_id: Option<String>,
    pub exec: Option<String>,
}

#[tauri::command]
pub fn list_processes() -> Vec<RunningApp> {
    #[cfg(target_os = "linux")]
    {
        crate::platform::linux::process::list()
    }

    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::process::list()
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        Vec::new()
    }
}

#[tauri::command]
pub fn list_processes_on_port(port: u16) -> Vec<RunningApp> {
    #[cfg(target_os = "linux")]
    {
        crate::platform::linux::process::list_on_port(port)
    }

    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::process::list_on_port(port)
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = port;
        Vec::new()
    }
}

#[tauri::command]
pub fn kill_process(pid: u32) -> Result<String, String> {
    #[cfg(target_os = "linux")]
    {
        crate::platform::linux::process::kill(pid)
    }

    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::process::kill(pid)
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = pid;
        Err("kill not supported on this platform".to_string())
    }
}
