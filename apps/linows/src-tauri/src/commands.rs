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
pub fn record_usage(
    state: State<'_, AppState>,
    candidate_id: String,
    action: String,
) -> UsageResult {
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
pub fn open_path(
    window: tauri::WebviewWindow,
    path: String,
    kind: Option<String>,
    id: Option<String>,
) -> Result<(), String> {
    // Linux system settings: settings://panel → gnome-control-center panel
    #[cfg(target_os = "linux")]
    if let Some(panel) = path.strip_prefix("settings://") {
        let _ = window.hide();
        let panel = panel.to_string();
        std::thread::spawn(move || {
            // D-Bus activation: works on GNOME, properly focuses the window.
            let dbus_ok = std::process::Command::new("gdbus")
                .args([
                    "call",
                    "--session",
                    "--dest",
                    "org.gnome.Settings",
                    "--object-path",
                    "/org/gnome/Settings",
                    "--method",
                    "org.freedesktop.Application.ActivateAction",
                    "launch-panel",
                    &format!("[<'{panel}'>, <@av []>]"),
                    "{}",
                ])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false);

            // Fallback: direct command (KDE, non-GNOME desktops)
            if !dbus_ok {
                let _ = std::process::Command::new("gnome-control-center")
                    .arg(&panel)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();
            }
        });
        return Ok(());
    }

    if kind.as_deref() == Some("app") && !path.contains("://") {
        let result = launch_app(&path, id.as_deref());
        if result.is_ok() {
            let _ = window.hide();
        }
        result
    } else if kind.as_deref() == Some("browser") {
        let _ = window.hide();
        std::thread::spawn(move || {
            let _ = open::that(&path);
            for class in &["Brave-browser", "firefox", "chromium", "Google-chrome"] {
                if try_focus_window(class) {
                    break;
                }
            }
        });
        Ok(())
    } else {
        let _ = window.hide();
        std::thread::spawn(move || {
            #[cfg(target_os = "linux")]
            {
                // Clear LD_LIBRARY_PATH so child processes use system libraries.
                // On NixOS, the Tauri dev shell may set paths that conflict with
                // system apps (glibc version mismatch).
                let _ = std::process::Command::new("xdg-open")
                    .arg(&path)
                    .env_remove("LD_LIBRARY_PATH")
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();
            }
            #[cfg(not(target_os = "linux"))]
            {
                let _ = open::that(&path);
            }
        });
        Ok(())
    }
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
    state.request_index_refresh()
}

#[tauri::command]
pub fn request_index_refresh(state: State<'_, AppState>) -> bool {
    state.request_index_refresh()
}

#[tauri::command]
pub fn force_index_refresh(state: State<'_, AppState>) -> bool {
    state.force_index_refresh()
}

#[tauri::command]
pub fn quit_app(app: tauri::AppHandle) {
    eprintln!("look: quit via Alt+Shift+Q");
    app.exit(0);
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

// --- App launching helpers ---

fn launch_app(exec: &str, id: Option<&str>) -> Result<(), String> {
    let desktop_file = id
        .and_then(|id| id.strip_prefix("app:"))
        .and_then(find_desktop_file);

    // Try to focus an existing window before launching a new instance.
    if let Some(ref real_path) = desktop_file
        && try_focus_existing(real_path)
    {
        return Ok(());
    }

    // Build the launch chain: gtk-launch → gio launch → direct exec.
    // gtk-launch is preferred because gio launch uses D-Bus activation
    // which can silently fail to show a window on first invocation.
    let desktop_path = desktop_file.clone();
    let desktop_name = id
        .and_then(|id| id.strip_prefix("app:"))
        .and_then(|p| std::path::Path::new(p).file_name())
        .and_then(|f| f.to_str())
        .and_then(|f| f.strip_suffix(".desktop"))
        .map(String::from);
    let exec_cmd = exec.to_string();

    std::thread::spawn(move || {
        if let Some(ref name) = desktop_name {
            eprintln!("[launch] trying gtk-launch {name}");
            let result = std::process::Command::new("gtk-launch")
                .arg(name)
                .env_remove("LD_LIBRARY_PATH")
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::piped())
                .output();
            match &result {
                Ok(output) if output.status.success() => {
                    eprintln!("[launch] gtk-launch succeeded");
                    return;
                }
                Ok(output) => {
                    let err = String::from_utf8_lossy(&output.stderr);
                    eprintln!("[launch] gtk-launch failed (exit {}): {err}", output.status);
                }
                Err(e) => eprintln!("[launch] gtk-launch not found: {e}"),
            }
        }

        if let Some(ref real_path) = desktop_path {
            eprintln!("[launch] trying gio launch {real_path}");
            let result = std::process::Command::new("gio")
                .args(["launch", real_path])
                .env_remove("LD_LIBRARY_PATH")
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::piped())
                .output();
            match &result {
                Ok(output) if output.status.success() => {
                    eprintln!("[launch] gio launch succeeded");
                    return;
                }
                Ok(output) => {
                    let err = String::from_utf8_lossy(&output.stderr);
                    eprintln!("[launch] gio launch failed (exit {}): {err}", output.status);
                }
                Err(e) => eprintln!("[launch] gio not found: {e}"),
            }
        }

        let mut parts = exec_cmd.split_whitespace();
        if let Some(cmd) = parts.next() {
            let args: Vec<&str> = parts.filter(|s| !s.starts_with('%')).collect();
            eprintln!("[launch] trying direct exec: {cmd} {}", args.join(" "));
            match std::process::Command::new(cmd)
                .args(&args)
                .env_remove("LD_LIBRARY_PATH")
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
            {
                Ok(_) => eprintln!("[launch] direct exec spawned"),
                Err(e) => eprintln!("[launch] direct exec failed: {e}"),
            }
        }
    });

    Ok(())
}

fn try_focus_window(wm_class: &str) -> bool {
    // i3 window manager — use i3-msg exclusively (i3 ignores raw X11
    // _NET_ACTIVE_WINDOW messages, so the x11rb fallback would report
    // success without actually focusing).  Try both class and instance
    // criteria: GTK apps often set instance to the reverse-DNS app ID
    // (e.g. "org.pwmt.zathura") while class is the short name ("Zathura").
    if std::env::var("I3SOCK").is_ok() {
        for criterion in [
            format!("[class=\"(?i){wm_class}\"] focus"),
            format!("[instance=\"(?i){wm_class}\"] focus"),
        ] {
            if let Ok(output) = std::process::Command::new("i3-msg")
                .arg(&criterion)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .output()
            {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if stdout.contains("\"success\":true") {
                    return true;
                }
            }
        }
        return false;
    }

    // Non-i3: try i3-msg anyway (might be running), then x11rb fallback.
    if let Ok(output) = std::process::Command::new("i3-msg")
        .arg(format!("[class=\"(?i){wm_class}\"] focus"))
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("\"success\":true") {
            return true;
        }
    }

    // Linux: x11rb _NET_ACTIVE_WINDOW (covers GNOME, KDE, etc.)
    #[cfg(target_os = "linux")]
    if crate::platform::linux::window_focus::try_focus(wm_class) {
        return true;
    }

    false
}

/// Try to focus an existing window for a desktop file.
/// Dispatches to the appropriate method based on display server / compositor.
fn try_focus_existing(desktop_path: &str) -> bool {
    let wm_class = parse_desktop_field(desktop_path, "StartupWMClass");
    let stem = std::path::Path::new(desktop_path)
        .file_stem()
        .and_then(|f| f.to_str())
        .map(String::from);

    // For reverse-DNS stems like "org.pwmt.zathura", also try the last
    // segment ("zathura") — many apps use the short name as WM_CLASS even
    // when the desktop file uses the full reverse-DNS ID.
    let short_name = stem.as_deref().and_then(|s| {
        if s.contains('.') {
            s.rsplit('.').next().map(String::from)
        } else {
            None
        }
    });

    let mut candidates: Vec<&str> = [wm_class.as_deref(), stem.as_deref(), short_name.as_deref()]
        .into_iter()
        .flatten()
        .collect();
    candidates.dedup();
    eprintln!("[focus] try_focus_existing desktop={desktop_path} candidates={candidates:?}");

    #[cfg(target_os = "linux")]
    if crate::platform::linux::transparency::is_wayland() {
        return try_focus_wayland(desktop_path, &candidates);
    }

    for id in &candidates {
        if try_focus_window(id) {
            return true;
        }
    }
    false
}

/// Wayland focus: dispatch to the active compositor's IPC.
#[cfg(target_os = "linux")]
fn try_focus_wayland(desktop_path: &str, candidates: &[&str]) -> bool {
    if std::env::var("SWAYSOCK").is_ok() {
        return candidates.iter().any(|id| try_focus_sway(id));
    }
    if std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok() {
        return candidates.iter().any(|id| try_focus_hyprland(id));
    }
    // GNOME Wayland: use GNOME Shell extension
    let desktop_id = std::path::Path::new(desktop_path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("");
    !desktop_id.is_empty() && crate::platform::linux::gnome_ext::try_focus_app(desktop_id)
}

#[cfg(target_os = "linux")]
fn try_focus_sway(app_id: &str) -> bool {
    // Try the native wlr-foreign-toplevel protocol first (works for any
    // wlroots compositor); fall back to sway IPC if the protocol isn't
    // available.
    if crate::platform::linux::wlr_focus::try_focus(app_id) {
        return true;
    }
    for criteria in [
        format!("[app_id=\"(?i){app_id}\"] focus"),
        format!("[class=\"(?i){app_id}\"] focus"),
    ] {
        if let Ok(output) = std::process::Command::new("swaymsg")
            .arg(&criteria)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.contains("\"success\": true") {
                return true;
            }
        }
    }
    false
}

#[cfg(target_os = "linux")]
fn try_focus_hyprland(class: &str) -> bool {
    eprintln!("[focus] hyprland try class={class}");
    // Primary path: native wlr-foreign-toplevel-management. Works regardless
    // of the broken hyprctl dispatcher on v0.55+.
    if crate::platform::linux::wlr_focus::try_focus(class) {
        eprintln!("[focus] hyprland focus via wlr-foreign-toplevel succeeded");
        return true;
    }
    // Fallback for Hyprland < v0.55 where the legacy dispatcher still works
    // (and the wlr protocol may not be advertised).
    if !hyprland_has_client(class) {
        return false;
    }
    let _ = std::process::Command::new("hyprctl")
        .args(["dispatch", "focuswindow", &format!("class:{class}")])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output();
    if hyprland_active_class_matches(class) {
        eprintln!("[focus] hyprland legacy dispatcher worked");
        return true;
    }
    eprintln!("[focus] hyprland focus failed for class={class}, falling through to launch chain");
    false
}

#[cfg(target_os = "linux")]
fn hyprland_has_client(class: &str) -> bool {
    let Ok(output) = std::process::Command::new("hyprctl")
        .args(["clients", "-j"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
    else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    json_has_class(&String::from_utf8_lossy(&output.stdout), class)
}

#[cfg(target_os = "linux")]
fn hyprland_active_class_matches(class: &str) -> bool {
    let Ok(output) = std::process::Command::new("hyprctl")
        .args(["activewindow", "-j"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
    else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    json_has_class(&String::from_utf8_lossy(&output.stdout), class)
}

#[cfg(target_os = "linux")]
fn json_has_class(json: &str, class: &str) -> bool {
    let json = json.to_lowercase();
    let needle = class.to_lowercase();
    for key in ["\"class\":", "\"initialclass\":"] {
        let mut rest = json.as_str();
        while let Some(idx) = rest.find(key) {
            rest = &rest[idx + key.len()..];
            let trimmed = rest.trim_start();
            if let Some(after_quote) = trimmed.strip_prefix('"')
                && let Some(end) = after_quote.find('"')
                && after_quote[..end] == needle
            {
                return true;
            }
        }
    }
    false
}

fn parse_desktop_field(path: &str, field: &str) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let prefix = format!("{field}=");
    let mut in_desktop_entry = false;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_desktop_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_desktop_entry {
            continue;
        }
        if let Some(val) = line.strip_prefix(&prefix) {
            let val = val.trim();
            if !val.is_empty() {
                return Some(val.to_string());
            }
        }
    }
    None
}

fn find_desktop_file(id_path: &str) -> Option<String> {
    if std::path::Path::new(id_path).exists() {
        return Some(id_path.to_string());
    }
    let path = std::path::Path::new(id_path);
    let dir = path.parent()?;
    let filename_lower = path.file_name()?.to_str()?.to_lowercase();
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        if entry.file_name().to_str()?.to_lowercase() == filename_lower {
            return Some(entry.path().to_string_lossy().to_string());
        }
    }
    None
}
