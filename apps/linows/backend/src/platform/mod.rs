//! Platform-specific code.
//!
//! Cross-platform entry points and types stay here. Per-OS implementations
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

// --- Icon resolution (cross-platform entry point + cache) ---

#[derive(Default)]
pub struct IconCache(pub Mutex<HashMap<String, Option<String>>>);

impl IconCache {
    pub fn new() -> Self {
        Self::default()
    }
}

/// The kind the frontend asks with for an image a block or row named.
const DECLARED_ICON_KIND: &str = "declared";

#[derive(Serialize)]
pub struct IconResult {
    pub data_url: Option<String>,
}
pub fn get_icon(cache: &IconCache, kind: &str, path: &str, id: Option<&str>) -> IconResult {
    let key = format!("{kind}:{path}");

    {
        let map = cache.0.lock().unwrap();
        if let Some(cached) = map.get(&key) {
            return IconResult {
                data_url: cached.clone(),
            };
        }
    }

    let data_url = if kind == DECLARED_ICON_KIND {
        // The named image IS the icon, so read it rather than ask the theme.
        shared::read_icon_file(path)
    } else {
        resolve_icon(kind, path, id)
    };

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

#[cfg(target_os = "windows")]
fn resolve_icon(kind: &str, path: &str, _id: Option<&str>) -> Option<String> {
    windows::icons::resolve(kind, path)
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn resolve_icon(_kind: &str, _path: &str, _id: Option<&str>) -> Option<String> {
    None
}

/// One blurred rectangle in window-local logical pixels. The UI sends these:
/// only it knows which surfaces are painted (the window at inner-gap 0, each
/// tile once the panes float). Lives here, not under `linux`, because every
/// shell on every OS spells its request with it.
#[derive(serde::Deserialize, Clone, Copy)]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub struct BlurRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

// --- Drive enumeration (Windows-only payload; stub elsewhere) ---

#[derive(Serialize)]
pub struct CandidateDrive {
    pub letter: String,
    pub root: String,
}
pub fn list_candidate_drives() -> Vec<CandidateDrive> {
    #[cfg(target_os = "windows")]
    {
        windows::drives::enumerate_candidates()
            .into_iter()
            .map(|d| CandidateDrive {
                letter: d.letter,
                root: d.root,
            })
            .collect()
    }
    #[cfg(not(target_os = "windows"))]
    {
        Vec::new()
    }
}
