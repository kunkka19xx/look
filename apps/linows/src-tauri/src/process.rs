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

/// List running GUI apps (apps with visible windows).
/// Unlike `list_processes` (used by /kill), this filters out background
/// services, terminal apps, and input methods - only switchable apps remain.
#[tauri::command]
pub fn list_running_apps() -> Vec<RunningApp> {
    #[cfg(target_os = "linux")]
    {
        crate::platform::linux::process::list_gui()
    }

    #[cfg(target_os = "windows")]
    {
        // Switcher-specific view: includes UWP apps (Settings, Calculator, …)
        // hosted by ApplicationFrameHost that list()/kill deliberately hides.
        crate::platform::windows::process::list_gui()
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        Vec::new()
    }
}

/// Activate (focus) a running app's window. On Linux, dispatches to the
/// same try_focus_existing chain used by open_path (compositor-aware).
/// On Windows, uses SetForegroundWindow via the window_focus module.
#[tauri::command]
pub fn activate_running_app(
    window: tauri::WebviewWindow,
    pid: u32,
    desktop_id: Option<String>,
    exec: Option<String>,
) -> Result<bool, String> {
    let _ = (&pid, &exec); // may be unused on some platforms

    #[cfg(target_os = "linux")]
    {
        // Try focus via desktop file metadata (WM_CLASS, app_id, etc.)
        if let Some(ref id) = desktop_id
            && let Some(desktop_path) = id.strip_prefix("app:")
            && crate::commands::try_focus_existing_pub(desktop_path)
        {
            let _ = window.hide();
            return Ok(true);
        }
        // Fallback: try focusing by exec binary name
        if let Some(ref exec_str) = exec {
            let bin = std::path::Path::new(
                exec_str
                    .split_whitespace()
                    .find(|t| !t.contains('='))
                    .unwrap_or(exec_str),
            )
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("");
            if !bin.is_empty() && crate::commands::try_focus_window_pub(bin) {
                let _ = window.hide();
                return Ok(true);
            }
        }
        Ok(false)
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(ref id) = desktop_id {
            // UWP windows (Settings, …) are addressed by HWND: several share one
            // ApplicationFrameHost PID, so exe-path matching can't disambiguate.
            if let Some(raw) = id.strip_prefix("hwnd:") {
                if let Ok(h) = raw.parse::<isize>()
                    && crate::platform::windows::window_focus::focus_hwnd(h)
                {
                    let _ = window.hide();
                    return Ok(true);
                }
                return Ok(false);
            }
            if let Some(path) = id.strip_prefix("app:")
                && crate::platform::windows::window_focus::try_focus_existing(path)
            {
                let _ = window.hide();
                return Ok(true);
            }
        }
        Ok(false)
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = (window, desktop_id);
        Ok(false)
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
