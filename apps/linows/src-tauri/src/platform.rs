use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Mutex;
use tauri::State;

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

fn resolve_icon(kind: &str, path: &str, id: Option<&str>) -> Option<String> {
    match kind {
        "app" => resolve_app_icon(path, id),
        "folder" => resolve_themed_icon("folder"),
        _ => resolve_file_icon(path),
    }
}

// --- XDG data directories ---

fn xdg_data_dirs() -> Vec<String> {
    let mut dirs = Vec::new();

    // XDG_DATA_HOME (defaults to ~/.local/share)
    let home = std::env::var("HOME").unwrap_or_default();
    if let Ok(data_home) = std::env::var("XDG_DATA_HOME") {
        dirs.push(data_home);
    } else if !home.is_empty() {
        dirs.push(format!("{home}/.local/share"));
    }

    // XDG_DATA_DIRS
    if let Ok(data_dirs) = std::env::var("XDG_DATA_DIRS") {
        for dir in data_dirs.split(':') {
            let d = dir.trim();
            if !d.is_empty() {
                dirs.push(d.to_string());
            }
        }
    } else {
        dirs.push("/usr/local/share".to_string());
        dirs.push("/usr/share".to_string());
    }

    // NixOS-specific fallbacks
    if !home.is_empty() {
        let nix_profile = format!("{home}/.nix-profile/share");
        if Path::new(&nix_profile).is_dir() && !dirs.contains(&nix_profile) {
            dirs.push(nix_profile);
        }
    }
    let system_sw = "/run/current-system/sw/share".to_string();
    if Path::new(&system_sw).is_dir() && !dirs.contains(&system_sw) {
        dirs.push(system_sw);
    }

    dirs
}

// --- App icons ---

fn resolve_app_icon(exec_path: &str, id: Option<&str>) -> Option<String> {
    // Try direct .desktop lookup from id (most reliable)
    if let Some(id) = id {
        if let Some(desktop_path) = id.strip_prefix("app:") {
            if let Some(icon_name) = parse_desktop_icon(desktop_path) {
                if let Some(icon) = resolve_themed_icon(&icon_name) {
                    return Some(icon);
                }
            }
        }
    }

    let first_token = exec_path.split_whitespace().next()?;
    let bin_name = Path::new(first_token)
        .file_name()?
        .to_str()?
        .to_lowercase();

    // Try binary name as icon name
    if let Some(icon) = resolve_themed_icon(&bin_name) {
        return Some(icon);
    }

    // Scan .desktop files by Exec match
    let data_dirs = xdg_data_dirs();
    for data_dir in &data_dirs {
        let apps_dir = format!("{data_dir}/applications");
        if let Some(icon) = search_desktop_dir(&apps_dir, &bin_name) {
            return Some(icon);
        }
    }

    None
}

/// Parse Icon= from a .desktop file. Tries the path as-is first,
/// then case-insensitive search in the same directory (id is lowercased).
fn parse_desktop_icon(desktop_path: &str) -> Option<String> {
    // Try exact path first
    if let Some(icon) = parse_desktop_icon_field(desktop_path) {
        return Some(icon);
    }
    // The id is lowercased, so try case-insensitive match in the directory
    let path = Path::new(desktop_path);
    let dir = path.parent()?;
    let filename_lower = path.file_name()?.to_str()?.to_lowercase();
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        if name.to_str()?.to_lowercase() == filename_lower {
            return parse_desktop_icon_field(entry.path().to_str()?);
        }
    }
    None
}

fn parse_desktop_icon_field(path: &str) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
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
        if let Some(val) = line.strip_prefix("Icon=") {
            let val = val.trim();
            if !val.is_empty() {
                return Some(val.to_string());
            }
        }
    }
    None
}

fn search_desktop_dir(dir: &str, bin_name: &str) -> Option<String> {
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(icon) = search_desktop_dir(path.to_str()?, bin_name) {
                return Some(icon);
            }
            continue;
        }
        let path_str = path.to_str()?;
        if !path_str.ends_with(".desktop") {
            continue;
        }
        if let Some(icon_name) = parse_desktop_icon_if_match(path_str, bin_name) {
            if let Some(data_url) = resolve_themed_icon(&icon_name) {
                return Some(data_url);
            }
        }
    }
    None
}

fn parse_desktop_icon_if_match(desktop_path: &str, bin_name: &str) -> Option<String> {
    let content = fs::read_to_string(desktop_path).ok()?;
    let mut icon = None;
    let mut exec_matches = false;
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
        if let Some(val) = line.strip_prefix("Icon=") {
            icon = Some(val.trim().to_string());
        } else if let Some(val) = line.strip_prefix("Exec=") {
            let exec_lower = val.to_lowercase();
            for token in exec_lower.split_whitespace() {
                let token_bin = Path::new(token)
                    .file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or(token);
                if token_bin == bin_name {
                    exec_matches = true;
                    break;
                }
            }
        }
    }

    if exec_matches { icon } else { None }
}

// --- File icons ---

fn resolve_file_icon(path: &str) -> Option<String> {
    let ext = Path::new(path).extension()?.to_str()?.to_lowercase();
    let icon_name = mime_icon_name(&ext);
    resolve_themed_icon(icon_name)
}

fn mime_icon_name(ext: &str) -> &str {
    match ext {
        "txt" | "md" | "log" | "csv" | "json" | "xml" | "yaml" | "yml" | "toml" | "ini"
        | "cfg" | "conf" => "text-x-generic",
        "rs" | "py" | "js" | "ts" | "c" | "cpp" | "h" | "hpp" | "java" | "go" | "rb" | "sh"
        | "bash" | "zsh" | "fish" | "lua" | "php" | "cs" | "swift" | "kt" | "scala" | "zig"
        | "nix" | "html" | "css" | "scss" | "less" | "jsx" | "tsx" | "vue" | "svelte" => {
            "text-x-script"
        }
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "svg" | "webp" | "ico" | "tiff" | "tif" => {
            "image-x-generic"
        }
        "mp3" | "flac" | "wav" | "ogg" | "aac" | "wma" | "m4a" | "opus" => "audio-x-generic",
        "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" | "m4v" => "video-x-generic",
        "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" | "zst" => "package-x-generic",
        "pdf" => "application-pdf",
        "doc" | "docx" | "odt" | "rtf" => "x-office-document",
        "xls" | "xlsx" | "ods" => "x-office-spreadsheet",
        "ppt" | "pptx" | "odp" => "x-office-presentation",
        "exe" | "msi" | "appimage" | "deb" | "rpm" | "flatpakref" => "application-x-executable",
        "iso" | "img" => "media-optical",
        _ => "text-x-generic",
    }
}

// --- Freedesktop icon theme lookup ---

fn resolve_themed_icon(name: &str) -> Option<String> {
    if name.starts_with('/') {
        return read_icon_file(name);
    }

    let icon_dirs = build_icon_search_dirs();

    for dir in &icon_dirs {
        for ext in &["png", "svg"] {
            let candidate = format!("{dir}/{name}.{ext}");
            if let Some(data_url) = read_icon_file(&candidate) {
                return Some(data_url);
            }
        }
    }

    None
}

fn build_icon_search_dirs() -> Vec<String> {
    let mut dirs = Vec::new();
    let theme = detect_gtk_icon_theme().unwrap_or_else(|| "Adwaita".to_string());
    let data_dirs = xdg_data_dirs();

    let sizes = ["scalable", "256x256", "128x128", "96x96", "72x72", "64x64", "48x48"];
    let categories = ["apps", "mimetypes", "places", "devices", "actions"];

    // Search active theme in all data dirs, then hicolor fallback
    for theme_name in [theme.as_str(), "hicolor"] {
        for data_dir in &data_dirs {
            for size in &sizes {
                for cat in &categories {
                    dirs.push(format!("{data_dir}/icons/{theme_name}/{size}/{cat}"));
                }
            }
        }
    }

    // ~/.icons (legacy user icon dir)
    if let Ok(home) = std::env::var("HOME") {
        for size in &sizes {
            for cat in &categories {
                dirs.push(format!("{home}/.icons/{theme}/{size}/{cat}"));
                dirs.push(format!("{home}/.icons/hicolor/{size}/{cat}"));
            }
        }
    }

    // Pixmaps fallback (all data dirs)
    for data_dir in &data_dirs {
        dirs.push(format!("{data_dir}/pixmaps"));
    }

    dirs
}

fn detect_gtk_icon_theme() -> Option<String> {
    let home = std::env::var("HOME").ok()?;

    for settings_path in [
        format!("{home}/.config/gtk-3.0/settings.ini"),
        format!("{home}/.config/gtk-4.0/settings.ini"),
    ] {
        if let Ok(content) = fs::read_to_string(&settings_path) {
            for line in content.lines() {
                if let Some(val) = line.strip_prefix("gtk-icon-theme-name") {
                    let val = val.trim().strip_prefix('=')?.trim();
                    if !val.is_empty() {
                        return Some(val.to_string());
                    }
                }
            }
        }
    }

    None
}

// --- Read & encode icon files ---

fn read_icon_file(path: &str) -> Option<String> {
    let data = fs::read(path).ok()?;
    if data.is_empty() {
        return None;
    }

    let mime = if path.ends_with(".svg") {
        "image/svg+xml"
    } else if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".xpm") {
        return None;
    } else {
        if data.starts_with(b"\x89PNG") {
            "image/png"
        } else if data.starts_with(b"<") || data.starts_with(b"<?xml") {
            "image/svg+xml"
        } else {
            return None;
        }
    };

    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&data);
    Some(format!("data:{mime};base64,{b64}"))
}
