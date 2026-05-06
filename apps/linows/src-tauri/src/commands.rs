use crate::state::AppState;
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::State;

#[derive(Serialize)]
pub struct SearchResult {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub path: String,
    pub score: i64,
}

#[derive(Serialize)]
pub struct SearchPayload {
    pub count: usize,
    pub results: Vec<SearchResult>,
}

#[derive(Serialize)]
pub struct UsageResult {
    pub ok: bool,
    pub error: Option<String>,
}

#[tauri::command]
pub fn search(state: State<'_, AppState>, query: String, limit: u32) -> SearchPayload {
    let max = if limit == 0 { 40 } else { limit.min(100) } as usize;

    let scored = state.with_engine(|engine| engine.search_scored(&query, max));

    let results: Vec<SearchResult> = scored
        .into_iter()
        .map(|(candidate, score)| SearchResult {
            id: candidate.id.to_string(),
            kind: candidate.kind.as_str().to_string(),
            title: candidate.title.to_string(),
            subtitle: candidate.subtitle.as_deref().map(str::to_string),
            path: candidate.path.to_string(),
            score,
        })
        .collect();

    SearchPayload {
        count: results.len(),
        results,
    }
}

#[tauri::command]
pub fn record_usage(state: State<'_, AppState>, candidate_id: String, action: String) -> UsageResult {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let valid_actions = ["open_app", "open_file", "open_folder"];
    if !valid_actions.contains(&action.as_str()) {
        return UsageResult {
            ok: false,
            error: Some(format!("Invalid action: {action}")),
        };
    }

    let found = state.with_engine_mut(|engine| engine.record_usage_in_memory(&candidate_id, now));

    if found {
        // Also persist to SQLite
        let db_path = crate::state::default_db_path();
        if let Ok(store) = look_storage::SqliteStore::open(&db_path) {
            let _ = store.record_usage_event(&candidate_id, &action);
        }
    }

    UsageResult {
        ok: found,
        error: if found {
            None
        } else {
            Some(format!("Candidate not found: {candidate_id}"))
        },
    }
}

#[tauri::command]
pub fn open_path(path: String, kind: Option<String>) -> Result<(), String> {
    if kind.as_deref() == Some("app") {
        launch_app(&path)
    } else {
        open::that(&path).map_err(|e| format!("Failed to open: {e}"))
    }
}

fn launch_app(exec: &str) -> Result<(), String> {
    let mut parts = exec.split_whitespace();
    let cmd = parts.next().ok_or("Empty exec command")?;
    let args: Vec<&str> = parts.collect();

    std::process::Command::new(cmd)
        .args(&args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to launch {cmd}: {e}"))?;

    Ok(())
}

#[tauri::command]
pub fn reveal_path(path: String) -> Result<(), String> {
    let path_ref = std::path::Path::new(&path);

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer.exe")
            .arg("/select,")
            .arg(path_ref)
            .spawn()
            .map_err(|e| format!("Failed to reveal: {e}"))?;
    }

    #[cfg(target_os = "linux")]
    {
        // Try xdg-open on the parent directory
        let dir = if path_ref.is_file() {
            path_ref
                .parent()
                .unwrap_or(path_ref)
                .to_string_lossy()
                .to_string()
        } else {
            path.clone()
        };
        std::process::Command::new("xdg-open")
            .arg(&dir)
            .spawn()
            .map_err(|e| format!("Failed to reveal: {e}"))?;
    }

    Ok(())
}

#[tauri::command]
pub fn reload_config(state: State<'_, AppState>) -> bool {
    // Reload triggers engine refresh with new config
    state.request_index_refresh()
}

#[tauri::command]
pub fn request_index_refresh(state: State<'_, AppState>) -> bool {
    state.request_index_refresh()
}

#[tauri::command]
pub fn toggle_window(window: tauri::WebviewWindow) {
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
    } else {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[tauri::command]
pub fn hide_window(window: tauri::WebviewWindow) {
    let _ = window.hide();
}

#[derive(Serialize)]
pub struct FileMeta {
    pub size: Option<u64>,
    pub modified: Option<String>,
    pub is_image: bool,
}

const IMAGE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "bmp", "tiff", "tif", "webp", "svg", "ico", "heic",
];

#[tauri::command]
pub fn get_file_meta(path: String) -> FileMeta {
    let p = std::path::Path::new(&path);
    let meta = std::fs::metadata(p).ok();

    let size = meta.as_ref().map(|m| m.len());

    let modified = meta.as_ref().and_then(|m| {
        let mod_time = m.modified().ok()?;
        let secs = mod_time.duration_since(UNIX_EPOCH).ok()?.as_secs();
        // Format as ISO-ish date for JS to parse
        let dt = time_from_unix(secs);
        Some(dt)
    });

    let is_image = p
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| IMAGE_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false);

    FileMeta {
        size,
        modified,
        is_image,
    }
}

#[tauri::command]
pub fn get_app_version(path: String) -> Option<String> {
    let bin = path.split_whitespace().next()?;

    // Resolve symlinks to find real path (e.g., nix store path with version)
    if let Ok(real) = std::fs::canonicalize(bin) {
        let real_str = real.to_string_lossy();
        // Nix store: /nix/store/hash-name-1.2.3/bin/foo
        if let Some(caps) = extract_nix_version(&real_str) {
            return Some(caps);
        }
    }

    None
}

fn extract_nix_version(path: &str) -> Option<String> {
    // Match /nix/store/<hash>-<name>-<version>/...
    let store_prefix = "/nix/store/";
    let rest = path.strip_prefix(store_prefix)?;
    let dir_part = rest.split('/').next()?;
    // Skip the hash (32 chars + dash)
    let after_hash = dir_part.get(33..)?;
    // Find last dash followed by a digit → version starts there
    let mut version_start = None;
    for (i, _) in after_hash.match_indices('-') {
        if after_hash.get(i + 1..i + 2).map(|c| c.chars().next().unwrap_or(' ').is_ascii_digit()).unwrap_or(false) {
            version_start = Some(i + 1);
        }
    }
    let start = version_start?;
    Some(after_hash[start..].to_string())
}

fn time_from_unix(secs: u64) -> String {
    // Simple UTC formatting without extra deps
    let days = secs / 86400;
    let time_secs = secs % 86400;
    let hours = time_secs / 3600;
    let minutes = (time_secs % 3600) / 60;

    // Days since 1970-01-01
    let (year, month, day) = civil_from_days(days as i64);
    format!("{year:04}-{month:02}-{day:02} {hours:02}:{minutes:02}")
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    // Algorithm from Howard Hinnant
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}
