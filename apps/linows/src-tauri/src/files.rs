use serde::Serialize;
use std::time::UNIX_EPOCH;

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
        Some(time_from_unix(secs))
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

    let resolved = if bin.starts_with('/') {
        std::fs::canonicalize(bin).ok()
    } else {
        resolve_in_path(bin).and_then(|p| std::fs::canonicalize(p).ok())
    };

    if let Some(real) = resolved {
        let real_str = real.to_string_lossy();
        if let Some(v) = extract_nix_version(&real_str) {
            return Some(v);
        }
    }

    None
}

#[tauri::command]
pub fn is_dev_build() -> bool {
    cfg!(debug_assertions)
}

#[tauri::command]
pub fn copy_files_to_clipboard(paths: Vec<String>) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    crate::clipboard::mark_self_write();
    let uris: Vec<String> = paths
        .iter()
        .map(|p| {
            let encoded: String = p
                .bytes()
                .map(|b| match b {
                    b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                        (b as char).to_string()
                    }
                    _ => format!("%{b:02X}"),
                })
                .collect();
            format!("file://{encoded}")
        })
        .collect();
    let payload = format!("copy\n{}", uris.join("\n"));
    let mime = "x-special/gnome-copied-files";

    // Try wl-copy (Wayland) first, then xclip (X11) — no hard dependency on either
    let result = std::process::Command::new("wl-copy")
        .args(["-t", mime])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(ref mut stdin) = child.stdin {
                stdin.write_all(payload.as_bytes())?;
            }
            child.wait()
        });

    if result.is_ok() {
        return Ok(());
    }

    let result = std::process::Command::new("xclip")
        .args(["-selection", "clipboard", "-t", mime])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(ref mut stdin) = child.stdin {
                stdin.write_all(payload.as_bytes())?;
            }
            child.wait()
        });

    result
        .map(|_| ())
        .map_err(|e| format!("Failed to copy files: {e}. Install xclip or wl-clipboard."))
}

#[tauri::command]
pub fn get_home_dir() -> Option<String> {
    std::env::var("HOME").ok()
}

const AUDIO_EXTENSIONS: &[&str] = &["mp3", "m4a", "wav", "aac", "flac", "ogg", "aiff", "alac"];

#[tauri::command]
pub fn scan_music_folder(folder: String) -> Vec<String> {
    let dir = std::path::Path::new(&folder);
    let Ok(entries) = std::fs::read_dir(dir) else {
        return vec![];
    };
    let mut files: Vec<String> = entries
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            if !path.is_file() {
                return None;
            }
            let ext = path.extension()?.to_str()?.to_lowercase();
            if AUDIO_EXTENSIONS.contains(&ext.as_str()) {
                Some(path.to_string_lossy().to_string())
            } else {
                None
            }
        })
        .collect();
    files.sort();
    files
}

#[tauri::command]
pub async fn pick_folder(app: tauri::AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = std::sync::mpsc::channel();
    app.dialog()
        .file()
        .set_title("Choose Music Folder")
        .pick_folder(move |folder| {
            let result = folder.map(|f| f.to_string());
            let _ = tx.send(result);
        });
    rx.recv().ok().flatten()
}

#[tauri::command]
pub fn list_fonts() -> Vec<String> {
    let output = std::process::Command::new("fc-list")
        .args(["--format", "%{family}\n"])
        .output();
    let Ok(output) = output else { return vec![] };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut fonts: Vec<String> = stdout
        .lines()
        .flat_map(|line| line.split(',').map(|s| s.trim().to_string()))
        .filter(|s| !s.is_empty())
        .collect();
    fonts.sort();
    fonts.dedup();
    fonts
}

#[tauri::command]
pub async fn pick_image(app: tauri::AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = std::sync::mpsc::channel();
    app.dialog()
        .file()
        .set_title("Choose Background Image")
        .add_filter(
            "Images",
            &["png", "jpg", "jpeg", "webp", "bmp", "gif", "svg"],
        )
        .pick_file(move |file| {
            let result = file.map(|f| f.to_string());
            let _ = tx.send(result);
        });
    rx.recv().ok().flatten()
}

fn resolve_in_path(bin: &str) -> Option<std::path::PathBuf> {
    let path_var = std::env::var("PATH").ok()?;
    for dir in path_var.split(':') {
        let candidate = std::path::Path::new(dir).join(bin);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

fn extract_nix_version(path: &str) -> Option<String> {
    let store_prefix = "/nix/store/";
    let rest = path.strip_prefix(store_prefix)?;
    let dir_part = rest.split('/').next()?;
    let after_hash = dir_part.get(33..)?;
    let mut version_start = None;
    for (i, _) in after_hash.match_indices('-') {
        if after_hash
            .get(i + 1..i + 2)
            .map(|c| c.chars().next().unwrap_or(' ').is_ascii_digit())
            .unwrap_or(false)
        {
            version_start = Some(i + 1);
        }
    }
    let start = version_start?;
    Some(after_hash[start..].to_string())
}

fn time_from_unix(secs: u64) -> String {
    let days = secs / 86400;
    let time_secs = secs % 86400;
    let hours = time_secs / 3600;
    let minutes = (time_secs % 3600) / 60;

    let (year, month, day) = civil_from_days(days as i64);
    format!("{year:04}-{month:02}-{day:02} {hours:02}:{minutes:02}")
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
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
