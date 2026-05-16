//! Platform-specific code.
//!
//! Cross-platform Tauri commands and types stay here. Per-OS implementations
//! live under `platform/{linux,windows}/`. `platform/shared.rs` holds helpers
//! reused across platforms.

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "windows")]
pub mod windows;

pub mod shared;

use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::State;

// --- Icon resolution (cross-platform Tauri command + cache) ---

pub struct IconCache(pub Mutex<HashMap<String, Option<String>>>);

impl IconCache {
    pub fn new() -> Self {
        Self(Mutex::new(HashMap::new()))
    }
}

#[derive(Serialize)]
pub struct IconResult {
    pub data_url: Option<String>,
}

#[tauri::command]
pub fn get_icon(
    cache: State<'_, IconCache>,
    kind: String,
    path: String,
    id: Option<String>,
) -> IconResult {
    let key = format!("{kind}:{path}");

    {
        let map = cache.0.lock().unwrap();
        if let Some(cached) = map.get(&key) {
            return IconResult {
                data_url: cached.clone(),
            };
        }
    }

    let data_url = resolve_icon(&kind, &path, id.as_deref());

    {
        let mut map = cache.0.lock().unwrap();
        map.insert(key, data_url.clone());
    }

    IconResult { data_url }
}

#[cfg(target_os = "linux")]
fn resolve_icon(kind: &str, path: &str, id: Option<&str>) -> Option<String> {
    match kind {
        "app" => linux::icons::resolve_app_icon(path, id),
        "folder" => linux::icons::resolve_themed_icon("folder"),
        _ => linux::icons::resolve_file_icon(path),
    }
}

#[cfg(not(target_os = "linux"))]
fn resolve_icon(_kind: &str, _path: &str, _id: Option<&str>) -> Option<String> {
    // TODO(M2): Windows icon extraction via SHGetFileInfoW / IShellItemImageFactory.
    None
}

// --- Platform info (cross-platform Tauri command) ---

#[derive(Serialize)]
pub struct PlatformInfo {
    pub os: String,
    pub has_compositor: bool,
    /// Compositor name when known ("hyprland", "sway", "gnome", "kde", ...).
    /// Exposed to the frontend so CSS can branch on compositor-specific bugs
    /// (e.g. WebKitGTK backdrop-filter glitches on Hyprland).
    pub compositor: Option<String>,
}

#[tauri::command]
pub fn get_platform() -> PlatformInfo {
    let os = std::env::consts::OS.to_string();

    #[cfg(target_os = "linux")]
    let has_compositor = linux::transparency::has_compositor();

    #[cfg(not(target_os = "linux"))]
    let has_compositor = true;

    #[cfg(target_os = "linux")]
    let compositor = linux::wm::detect_compositor();

    #[cfg(not(target_os = "linux"))]
    let compositor: Option<String> = None;

    PlatformInfo {
        os,
        has_compositor,
        compositor,
    }
}

// --- Window effects (Tauri command; dispatches per OS) ---

#[tauri::command]
pub fn set_window_effect(window: tauri::Window, effect: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        windows::effects::apply(window, &effect)
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (window, effect);
        Ok(())
    }
}
