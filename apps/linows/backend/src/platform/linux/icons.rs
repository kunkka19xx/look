//! Linux icon resolution via XDG / freedesktop conventions.

use crate::platform::shared::read_icon_file;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

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

pub(crate) fn resolve_app_icon(exec_path: &str, id: Option<&str>) -> Option<String> {
    // Try direct .desktop lookup from id (most reliable)
    if let Some(id) = id
        && let Some(desktop_path) = id.strip_prefix("app:")
        && let Some(icon_name) = parse_desktop_icon(desktop_path)
        && let Some(icon) = resolve_themed_icon(&icon_name)
    {
        return Some(icon);
    }

    let first_token = exec_path.split_whitespace().next()?;
    let bin_name = Path::new(first_token).file_name()?.to_str()?.to_lowercase();

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
        if let Some(icon_name) = parse_desktop_icon_if_match(path_str, bin_name)
            && let Some(data_url) = resolve_themed_icon(&icon_name)
        {
            return Some(data_url);
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

pub(crate) fn resolve_file_icon(path: &str) -> Option<String> {
    let ext = Path::new(path).extension()?.to_str()?.to_lowercase();
    let icon_name = mime_icon_name(&ext);
    resolve_themed_icon(icon_name)
}

fn mime_icon_name(ext: &str) -> &str {
    match ext {
        "txt" | "md" | "log" | "csv" | "json" | "xml" | "yaml" | "yml" | "toml" | "ini" | "cfg"
        | "conf" => "text-x-generic",
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

/// The bitmap side a lookup aims for: the largest slot, the preview's 48 pt,
/// at 3x. A theme directory that holds this size, or a vector one that can
/// draw it, comes first; past that, the nearest larger one, as the UI only
/// ever shrinks a bitmap to its slot.
const WANTED_SIZE: u32 = 144;

/// Where a theme without an index.theme keeps its icons; only hicolor is
/// looked through this way, since every app falls back to it.
const LEGACY_SIZES: [&str; 7] = [
    "scalable", "256x256", "128x128", "96x96", "72x72", "64x64", "48x48",
];
const LEGACY_CATEGORIES: [&str; 5] = ["apps", "mimetypes", "places", "devices", "actions"];

pub(crate) fn resolve_themed_icon(name: &str) -> Option<String> {
    if name.starts_with('/') {
        return read_icon_file(name);
    }

    for dir in search_dirs() {
        for ext in &["png", "svg"] {
            let candidate = format!("{dir}/{name}.{ext}");
            if let Some(data_url) = read_icon_file(&candidate) {
                return Some(data_url);
            }
        }
    }

    None
}

/// Every directory an icon may sit in, best first, kept for the process the
/// way resolved icons are. Building it reads each theme's index.theme and
/// checks which of its directories exist, so a miss costs a pass over
/// those alone.
fn search_dirs() -> &'static [String] {
    static DIRS: OnceLock<Vec<String>> = OnceLock::new();
    DIRS.get_or_init(|| {
        let data_dirs = xdg_data_dirs();
        let mut bases = Vec::new();
        if let Ok(home) = std::env::var("HOME") {
            bases.push(format!("{home}/.icons"));
        }
        bases.extend(data_dirs.iter().map(|dir| format!("{dir}/icons")));
        let mut dirs = theme_dirs(&detect_icon_theme(), &bases);
        dirs.extend(
            data_dirs
                .iter()
                .map(|dir| format!("{dir}/pixmaps"))
                .filter(|dir| Path::new(dir).is_dir()),
        );
        dirs
    })
}

/// The icon theme spec's lookup order: `theme`, then what it inherits,
/// depth first, then hicolor. Within a theme, its directories best size
/// first, each across every base directory.
fn theme_dirs(theme: &str, bases: &[String]) -> Vec<String> {
    let mut dirs = Vec::new();
    for (name, index) in theme_chain(theme, bases) {
        let mut subdirs = index.dirs;
        subdirs.sort_by_key(|(_, range)| size_rank(*range));
        for (subdir, _) in &subdirs {
            for base in bases {
                let dir = format!("{base}/{name}/{subdir}");
                if Path::new(&dir).is_dir() {
                    dirs.push(dir);
                }
            }
        }
    }
    dirs
}

fn theme_chain(theme: &str, bases: &[String]) -> Vec<(String, ThemeIndex)> {
    fn visit(
        name: &str,
        bases: &[String],
        seen: &mut HashSet<String>,
        chain: &mut Vec<(String, ThemeIndex)>,
    ) {
        if !seen.insert(name.to_string()) {
            return;
        }
        let Some(index) = read_theme_index(name, bases) else {
            return;
        };
        let parents = index.inherits.clone();
        chain.push((name.to_string(), index));
        for parent in parents.iter().filter(|p| *p != HICOLOR) {
            visit(parent, bases, seen, chain);
        }
    }

    let mut seen = HashSet::new();
    let mut chain = Vec::new();
    visit(theme, bases, &mut seen, &mut chain);
    visit(HICOLOR, bases, &mut seen, &mut chain);
    chain
}

const HICOLOR: &str = "hicolor";

/// A theme's index.theme: the themes it falls back to and its icon
/// directories, each with the pixel sizes it serves.
struct ThemeIndex {
    inherits: Vec<String>,
    dirs: Vec<(String, (u32, u32))>,
}

fn read_theme_index(name: &str, bases: &[String]) -> Option<ThemeIndex> {
    match bases
        .iter()
        .find_map(|base| fs::read_to_string(format!("{base}/{name}/index.theme")).ok())
    {
        Some(text) => Some(parse_theme_index(&text)),
        None if name == HICOLOR => Some(ThemeIndex {
            inherits: Vec::new(),
            dirs: LEGACY_SIZES
                .iter()
                .flat_map(|size| {
                    let side = size
                        .split('x')
                        .next()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(WANTED_SIZE);
                    LEGACY_CATEGORIES
                        .iter()
                        .map(move |cat| (format!("{size}/{cat}"), (side, side)))
                })
                .collect(),
        }),
        None => None,
    }
}

fn parse_theme_index(text: &str) -> ThemeIndex {
    let mut sections: HashMap<&str, HashMap<&str, &str>> = HashMap::new();
    let mut section = "";
    for line in text.lines() {
        let line = line.trim();
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = name;
        } else if let Some((key, value)) = line.split_once('=') {
            sections
                .entry(section)
                .or_default()
                .insert(key.trim(), value.trim());
        }
    }
    let list = |key: &str| -> Vec<&str> {
        sections
            .get("Icon Theme")
            .and_then(|theme| theme.get(key))
            .map(|value| {
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    };
    // Breeze's scaled directories are links to the plain ones and carry no
    // section of their own; such entries are dropped here.
    let dirs = list("Directories")
        .into_iter()
        .chain(list("ScaledDirectories"))
        .filter_map(|dir| Some((dir.to_string(), size_range(sections.get(dir)?))))
        .collect();
    ThemeIndex {
        inherits: list("Inherits").into_iter().map(String::from).collect(),
        dirs,
    }
}

/// The pixel sizes a theme directory serves, per its Type: Threshold, the
/// spec's default, takes Size give or take Threshold.
fn size_range(keys: &HashMap<&str, &str>) -> (u32, u32) {
    let num = |key: &str, default: u32| {
        keys.get(key)
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(default)
    };
    let size = num("Size", 0);
    let scale = num("Scale", 1).max(1);
    let (min, max) = match keys.get("Type").copied() {
        Some("Fixed") => (size, size),
        Some("Scalable") => (num("MinSize", size), num("MaxSize", size)),
        _ => {
            let threshold = num("Threshold", 2);
            (size.saturating_sub(threshold), size + threshold)
        }
    };
    (min * scale, max * scale)
}

/// Lower sorts first: a directory serving `WANTED_SIZE`, then the nearest
/// larger, then the nearest smaller.
fn size_rank((min, max): (u32, u32)) -> (u8, u32) {
    if WANTED_SIZE < min {
        (1, min - WANTED_SIZE)
    } else if WANTED_SIZE > max {
        (2, WANTED_SIZE - max)
    } else {
        (0, 0)
    }
}

/// The theme the desktop shows: Plasma's own setting under Plasma, which
/// it also copies into GTK's; GTK's elsewhere. Each desktop's default if
/// unset.
fn detect_icon_theme() -> String {
    let plasma = std::env::var("XDG_CURRENT_DESKTOP")
        .is_ok_and(|desktops| desktops.split(':').any(|d| d.eq_ignore_ascii_case("KDE")));
    if plasma {
        detect_kde_icon_theme()
            .or_else(detect_gtk_icon_theme)
            .unwrap_or_else(|| "breeze".to_string())
    } else {
        detect_gtk_icon_theme().unwrap_or_else(|| "Adwaita".to_string())
    }
}

fn config_home() -> Option<String> {
    std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|dir| !dir.is_empty())
        .or_else(|| Some(format!("{}/.config", std::env::var("HOME").ok()?)))
}

fn detect_kde_icon_theme() -> Option<String> {
    let content = fs::read_to_string(format!("{}/kdeglobals", config_home()?)).ok()?;
    let mut in_icons = false;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_icons = line == "[Icons]";
        } else if in_icons
            && let Some((key, value)) = line.split_once('=')
            && key.trim() == "Theme"
            && !value.trim().is_empty()
        {
            return Some(value.trim().to_string());
        }
    }
    None
}

fn detect_gtk_icon_theme() -> Option<String> {
    let config = config_home()?;

    for settings_path in [
        format!("{config}/gtk-3.0/settings.ini"),
        format!("{config}/gtk-4.0/settings.ini"),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).expect("fixture dir");
        fs::write(path, text).expect("fixture file");
    }

    /// Breeze lays out `<context>/<size>`, not `<size>/<context>`, and
    /// inherits; its icons are found through its index, ahead of hicolor's.
    #[test]
    fn a_theme_is_searched_through_its_index_then_its_parents() {
        let root = std::env::temp_dir().join(format!("look-theme-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        write(
            &root.join("dark/index.theme"),
            "[Icon Theme]\nInherits=light,hicolor\nDirectories=status/22\n\n\
             [status/22]\nSize=22\nType=Fixed\n",
        );
        write(&root.join("dark/status/22/x.svg"), "<svg/>");
        write(
            &root.join("light/index.theme"),
            "[Icon Theme]\nInherits=hicolor\nDirectories=apps/16,apps/48\n\
             ScaledDirectories=apps/48@2x\n\n\
             [apps/16]\nSize=16\nType=Fixed\n\n\
             [apps/48]\nSize=48\nType=Scalable\nMinSize=48\nMaxSize=256\n",
        );
        write(&root.join("light/apps/16/x.svg"), "<svg/>");
        write(&root.join("light/apps/48/x.svg"), "<svg/>");
        write(
            &root.join("hicolor/index.theme"),
            "[Icon Theme]\nDirectories=48x48/apps,256x256/apps\n\n\
             [48x48/apps]\nSize=48\nType=Threshold\n\n\
             [256x256/apps]\nSize=256\nType=Threshold\n",
        );
        write(&root.join("hicolor/48x48/apps/x.png"), "png");
        write(&root.join("hicolor/256x256/apps/x.png"), "png");

        let base = root.to_str().unwrap().to_string();
        let dirs: Vec<String> = theme_dirs("dark", std::slice::from_ref(&base))
            .into_iter()
            .map(|dir| dir.strip_prefix(&format!("{base}/")).unwrap().to_string())
            .collect();
        assert_eq!(
            dirs,
            [
                "dark/status/22",
                "light/apps/48",
                "light/apps/16",
                "hicolor/256x256/apps",
                "hicolor/48x48/apps",
            ]
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_directory_serving_the_wanted_size_ranks_first() {
        let mut ranks = [(16, 16), (512, 512), (48, 256), (192, 192), (96, 96)];
        ranks.sort_by_key(|range| size_rank(*range));
        assert_eq!(
            ranks,
            [(48, 256), (192, 192), (512, 512), (96, 96), (16, 16)]
        );
    }
}
