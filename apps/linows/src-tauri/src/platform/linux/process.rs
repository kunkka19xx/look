//! Linux process listing (via `/proc`) and kill (via `kill -9`).

use crate::process::RunningApp;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub(crate) fn list() -> Vec<RunningApp> {
    let my_uid = read_my_uid();

    // 1. Collect running user processes: name → Vec<(pid, rss_kb)>
    let mut procs: HashMap<String, Vec<(u32, u64)>> = HashMap::new();
    if let Ok(entries) = fs::read_dir("/proc") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if !name_str.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let pid: u32 = match name_str.parse() {
                Ok(p) => p,
                Err(_) => continue,
            };
            let status = match fs::read_to_string(format!("/proc/{pid}/status")) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let proc_uid = parse_status_field(&status, "Uid:")
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(0);
            if proc_uid != my_uid {
                continue;
            }
            let proc_name = parse_status_field(&status, "Name:").unwrap_or_default();
            let rss = parse_status_field(&status, "VmRSS:")
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0);
            if proc_name.is_empty() || rss == 0 {
                continue;
            }
            procs.entry(proc_name).or_default().push((pid, rss));
        }
    }

    // 2. Normalize process names for matching
    //    NixOS wraps binaries: "firefox" → ".firefox-wrappe" (truncated to 15 chars)
    //    Build a map: normalized_name → Vec<(pid, rss)>
    let mut norm_procs: HashMap<String, Vec<(u32, u64)>> = HashMap::new();
    for (proc_name, pids) in &procs {
        for candidate in normalize_proc_name(proc_name) {
            norm_procs.entry(candidate).or_default().extend(pids);
        }
    }

    // 3. Scan .desktop files, match Exec against running process names.
    //    Sort so "primary" entries (desktop stem matches a bin in its own Exec,
    //    e.g. `steam.desktop` with `Exec=steam`) are tried first. Otherwise
    //    user-installed game shortcuts like `Dota 2.desktop` (Exec=steam …)
    //    would claim the steam /proc match before steam.desktop gets a turn,
    //    and Steam would show up as "Dota 2" in the running-apps list.
    let mut desktop_entries = scan_desktop_files();
    desktop_entries.sort_by_key(|de| {
        let stem = Path::new(&de.path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        let bins = extract_bin_names(&de.exec);
        let primary = bins.iter().any(|b| b.to_lowercase() == stem);
        (if primary { 0 } else { 1 }, stem)
    });
    let mut apps: Vec<RunningApp> = Vec::new();
    let mut matched_keys: std::collections::HashSet<String> = std::collections::HashSet::new();

    for de in &desktop_entries {
        let mut bin_names = extract_bin_names(&de.exec);
        // DBusActivatable apps may use `Exec=gapplication launch X.Y.Z` (Weather)
        // or wrap the binary so /proc Name doesn't match Exec. Derive a kebab
        // candidate from the desktop file stem as a fallback (`org.gnome.Weather`
        // → `gnome-weather`).
        if let Some(kebab) = kebab_from_desktop_stem(&de.path) {
            let key = kebab.to_lowercase();
            if !bin_names.iter().any(|n| n.to_lowercase() == key) {
                bin_names.push(kebab);
            }
        }
        // GJS apps (Weather, etc.) launch via `gjs-console` but set their
        // /proc Name to the GApplication ID — `org.gnome.Weather` truncated
        // to `org.gnome.Weath` at the 15-char comm limit. Add the raw dotted
        // stem so the truncation fallback below picks it up.
        if let Some(stem) = Path::new(&de.path).file_stem().and_then(|s| s.to_str())
            && stem.contains('.')
        {
            let key = stem.to_lowercase();
            if !bin_names.iter().any(|n| n.to_lowercase() == key) {
                bin_names.push(stem.to_string());
            }
        }
        for bin in &bin_names {
            let key = bin.to_lowercase();
            if matched_keys.contains(&key) {
                continue;
            }
            // /proc/<pid>/comm is limited to TASK_COMM_LEN-1 == 15 chars, so
            // `gnome-text-editor` shows up as `gnome-text-edit`. On NixOS the
            // wrapper prefixes a `.` (`.gnome-text-edi-wrapped`), eating one
            // more slot so the stripped form is only 14 chars (`gnome-text-edi`).
            // Try both lengths.
            let pids = norm_procs.get(&key).or_else(|| {
                let trunc15: String = key.chars().take(15).collect();
                if trunc15.len() < key.len()
                    && let Some(v) = norm_procs.get(&trunc15)
                {
                    return Some(v);
                }
                let trunc14: String = key.chars().take(14).collect();
                if trunc14.len() < key.len() {
                    norm_procs.get(&trunc14)
                } else {
                    None
                }
            });
            if let Some(pids) = pids {
                matched_keys.insert(key);
                let &(pid, _) = pids.iter().max_by_key(|(_, rss)| *rss).unwrap();
                apps.push(RunningApp {
                    name: de.name.clone(),
                    pid,
                    desktop_id: Some(format!("app:{}", de.path)),
                    exec: Some(de.exec.clone()),
                });
                break;
            }
        }
    }

    // Sort alphabetically by name
    apps.sort_by_key(|a| a.name.to_lowercase());
    apps
}

/// Like `list()`, but filtered to only GUI apps with visible windows.
/// On X11: uses `_NET_CLIENT_LIST` + `_NET_WM_PID` to get windowed PIDs.
/// On Wayland: uses `wlr-foreign-toplevel-management` app_ids, then falls
/// back to GNOME Shell (which doesn't expose PIDs, so we match by desktop stem).
pub(crate) fn list_gui() -> Vec<RunningApp> {
    let my_pid = std::process::id();
    let all: Vec<RunningApp> = list()
        .into_iter()
        .filter(|app| {
            if app.pid == my_pid {
                return false;
            }
            // Filter out Look itself by binary name
            if let Some(ref exec) = app.exec {
                let bin = exec.split_whitespace().next().unwrap_or("");
                let stem = Path::new(bin)
                    .file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or("");
                if stem == "lookapp" {
                    return false;
                }
            }
            true
        })
        .collect();
    if all.is_empty() {
        return all;
    }

    if super::transparency::is_wayland() {
        let debug = std::env::var("LOOK_DEBUG_GUI").is_ok();
        if debug {
            eprintln!("[list_gui] wayland; candidates from list(): {}", all.len());
            for app in &all {
                eprintln!(
                    "[list_gui]   candidate: name={:?} pid={} desktop_id={:?}",
                    app.name, app.pid, app.desktop_id
                );
            }
        }
        // Wayland: get app_ids from wlr-foreign-toplevel
        let app_ids = super::wlr_focus::list_toplevel_app_ids();
        if debug {
            eprintln!("[list_gui] wlr app_ids: {:?}", app_ids);
        }
        if !app_ids.is_empty() {
            return all
                .into_iter()
                .filter(|app| {
                    // Match desktop file stem (e.g. "org.mozilla.firefox" or "firefox")
                    // against toplevel app_ids
                    if let Some(ref id) = app.desktop_id {
                        let stem = id
                            .strip_prefix("app:")
                            .and_then(|p| {
                                std::path::Path::new(p).file_stem().and_then(|f| f.to_str())
                            })
                            .unwrap_or("");
                        let stem_lower = stem.to_lowercase();
                        // Try full stem and last segment (for reverse-DNS like org.mozilla.firefox)
                        let short = stem_lower.rsplit('.').next().unwrap_or("");
                        app_ids.contains(&stem_lower)
                            || (!short.is_empty() && app_ids.contains(short))
                    } else {
                        false
                    }
                })
                .collect();
        }
        // wlr unavailable (GNOME Wayland) — ask the Look GNOME Shell extension
        // which apps Shell.AppSystem considers running (≥1 window). This is
        // the same signal GNOME's Activities/app-switcher uses.
        let ext_ids = super::gnome_ext::list_windowed_apps();
        if debug {
            eprintln!("[list_gui] gnome ext ListWindowedApps: {:?}", ext_ids);
        }
        if let Some(ids) = ext_ids {
            let windowed: std::collections::HashSet<String> =
                ids.into_iter().map(|s| s.to_lowercase()).collect();
            return all
                .into_iter()
                .filter(|app| {
                    let Some(ref id) = app.desktop_id else {
                        return false;
                    };
                    let Some(path) = id.strip_prefix("app:") else {
                        return false;
                    };
                    let fname = Path::new(path)
                        .file_name()
                        .and_then(|f| f.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    let keep = windowed.contains(&fname);
                    if debug {
                        eprintln!(
                            "[list_gui]   ext-filter name={:?} fname={:?} keep={}",
                            app.name, fname, keep
                        );
                    }
                    keep
                })
                .collect();
        }
        // Extension unreachable — last-resort heuristic.
        if debug {
            eprintln!("[list_gui] no wlr, no extension — using desktop-hint heuristic");
        }
        return filter_by_desktop_hints(all);
    }

    // X11: enumerate visible windows with WM_CLASS + (optional) _NET_WM_PID.
    // We use WM_CLASS as a second matching axis because:
    //   • LibreOffice's /proc Name is `soffice.bin` — no `libreoffice-*.desktop`
    //     Exec matches it — but every LibreOffice window's WM_CLASS instance
    //     (`libreoffice-writer`, `libreoffice-calc`, …) matches the
    //     corresponding desktop's `StartupWMClass`.
    //   • Many Electron/Java apps (Postman, Discord, Element, …) don't set
    //     `_NET_WM_PID`, so the PID-only filter drops them entirely. WM_CLASS
    //     matching recovers them without needing the PID.
    let windows = super::window_focus::list_visible_windows();
    if windows.is_empty() {
        // X11 query failed (no X server, no _NET_CLIENT_LIST, etc.)
        return filter_by_desktop_hints(all);
    }

    // Pass 1: existing /proc-name-matched apps, filtered to those whose PID
    // (or its parent) owns a visible window. Apps whose windows omit
    // _NET_WM_PID won't survive this pass; pass 2 catches them.
    let windowed_pids: std::collections::HashSet<u32> =
        windows.iter().filter_map(|w| w.pid).collect();
    let mut expanded_pids = windowed_pids.clone();
    for &pid in &windowed_pids {
        if let Ok(status) = fs::read_to_string(format!("/proc/{pid}/status"))
            && let Some(ppid) =
                parse_status_field(&status, "PPid:").and_then(|v| v.parse::<u32>().ok())
        {
            expanded_pids.insert(ppid);
        }
    }
    let mut result: Vec<RunningApp> = all
        .into_iter()
        .filter(|app| expanded_pids.is_empty() || expanded_pids.contains(&app.pid))
        .collect();
    let mut seen_desktop_ids: std::collections::HashSet<String> =
        result.iter().filter_map(|a| a.desktop_id.clone()).collect();

    // Pass 2: enrich with WM_CLASS-derived matches. Scan desktop files again
    // (cheap — filesystem cache is hot) and pair each visible window's
    // WM_CLASS with a desktop entry's StartupWMClass / file-stem fallback.
    let desktop_entries = scan_desktop_files();
    let mut class_map: std::collections::HashMap<String, &DesktopEntry> =
        std::collections::HashMap::new();
    for de in &desktop_entries {
        for key in desktop_wm_class_keys(de) {
            class_map.entry(key).or_insert(de);
        }
    }
    for win in &windows {
        for candidate in [win.instance.as_str(), win.class.as_str()] {
            if candidate.is_empty() {
                continue;
            }
            let Some(de) = class_map.get(candidate) else {
                continue;
            };
            let desktop_id = format!("app:{}", de.path);
            if !seen_desktop_ids.insert(desktop_id.clone()) {
                continue;
            }
            result.push(RunningApp {
                name: de.name.clone(),
                pid: win.pid.unwrap_or(0),
                desktop_id: Some(desktop_id),
                exec: Some(de.exec.clone()),
            });
            break;
        }
    }

    result.sort_by_key(|a| a.name.to_lowercase());
    result
}

/// All lowercased keys a desktop entry can be matched against by `WM_CLASS`.
/// Includes the declared `StartupWMClass`, the file stem (often the
/// canonical class — e.g. `discord.desktop` → `discord`), and a kebab form
/// of reverse-DNS stems (`org.gnome.TextEditor` → `gnome-texteditor`).
fn desktop_wm_class_keys(de: &DesktopEntry) -> Vec<String> {
    let mut keys = Vec::new();
    if let Some(ref wc) = de.wm_class {
        let k = wc.to_lowercase();
        if !k.is_empty() {
            keys.push(k);
        }
    }
    if let Some(stem) = Path::new(&de.path).file_stem().and_then(|s| s.to_str()) {
        let lower = stem.to_lowercase();
        if !lower.is_empty() && !keys.contains(&lower) {
            keys.push(lower);
        }
        if let Some(kebab) = kebab_from_appid(stem) {
            let k = kebab.to_lowercase();
            if !keys.contains(&k) {
                keys.push(k);
            }
        }
    }
    keys
}

/// Heuristic filter for GNOME Wayland (no wlr, no X11 window list).
/// Checks desktop file for Terminal=true and known non-GUI categories.
///
/// We intentionally do NOT exclude `--gapplication-service` daemons here:
/// most GNOME apps (Calendar, Weather, Maps, Files, …) run in daemon mode
/// even when the user has a visible window open. Without a compositor signal
/// we can't distinguish "daemon idling in background" from "daemon with an
/// active window," so we err on the side of showing the app — false positives
/// (a few invisible daemons) beat false negatives (missing real user apps).
fn filter_by_desktop_hints(apps: Vec<RunningApp>) -> Vec<RunningApp> {
    apps.into_iter()
        .filter(|app| {
            let Some(ref id) = app.desktop_id else {
                return false;
            };
            let Some(path) = id.strip_prefix("app:") else {
                return false;
            };
            !is_terminal_or_background(path)
        })
        .collect()
}

/// Check if a desktop file is a terminal app, background service, or input method.
fn is_terminal_or_background(path: &str) -> bool {
    let Ok(content) = fs::read_to_string(path) else {
        return false;
    };
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
        if let Some(val) = line.strip_prefix("Terminal=")
            && val.trim().eq_ignore_ascii_case("true")
        {
            return true;
        }
        if let Some(val) = line.strip_prefix("Categories=") {
            let lower = val.to_lowercase();
            if lower.contains("inputmethod") || lower.contains("monitor") {
                return true;
            }
        }
        // fcitx5/ibus desktop files have Categories=System;Utility (no
        // InputMethod), but their unlocalized GenericName is "Input Method".
        if let Some(val) = line.strip_prefix("GenericName=")
            && val.to_lowercase().contains("input method")
        {
            return true;
        }
    }
    false
}

pub(crate) fn list_on_port(port: u16) -> Vec<RunningApp> {
    // Parse /proc/net/tcp and /proc/net/tcp6 to find listening sockets on the given port
    let mut pids: Vec<u32> = Vec::new();
    let mut inodes: std::collections::HashSet<u64> = std::collections::HashSet::new();

    for tcp_path in &["/proc/net/tcp", "/proc/net/tcp6"] {
        if let Ok(content) = fs::read_to_string(tcp_path) {
            for line in content.lines().skip(1) {
                let fields: Vec<&str> = line.split_whitespace().collect();
                if fields.len() < 10 {
                    continue;
                }
                // State 0A = LISTEN
                if fields[3] != "0A" {
                    continue;
                }
                // local_address is hex IP:PORT
                if let Some(port_hex) = fields[1].split(':').nth(1)
                    && let Ok(p) = u16::from_str_radix(port_hex, 16)
                    && p == port
                    && let Ok(inode) = fields[9].parse::<u64>()
                {
                    inodes.insert(inode);
                }
            }
        }
    }

    if inodes.is_empty() {
        return Vec::new();
    }

    // Find PIDs owning these inodes by scanning /proc/[pid]/fd/
    let my_uid = read_my_uid();
    if let Ok(entries) = fs::read_dir("/proc") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if !name_str.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let pid: u32 = match name_str.parse() {
                Ok(p) => p,
                Err(_) => continue,
            };
            // Check ownership
            let status_path = format!("/proc/{pid}/status");
            if let Ok(status) = fs::read_to_string(&status_path) {
                let proc_uid = parse_status_field(&status, "Uid:")
                    .and_then(|v| v.parse::<u32>().ok())
                    .unwrap_or(0);
                if proc_uid != my_uid {
                    continue;
                }
            } else {
                continue;
            }

            let fd_dir = format!("/proc/{pid}/fd");
            if let Ok(fds) = fs::read_dir(&fd_dir) {
                for fd in fds.flatten() {
                    if let Ok(link) = fs::read_link(fd.path())
                        && let Some(inode_str) = link
                            .to_string_lossy()
                            .strip_prefix("socket:[")
                            .and_then(|s| s.strip_suffix(']'))
                        && let Ok(inode) = inode_str.parse::<u64>()
                        && inodes.contains(&inode)
                    {
                        pids.push(pid);
                        break;
                    }
                }
            }
        }
    }

    pids.sort();
    pids.dedup();

    // Build RunningApp entries
    pids.iter()
        .map(|&pid| {
            let name = fs::read_to_string(format!("/proc/{pid}/comm"))
                .unwrap_or_default()
                .trim()
                .to_string();
            RunningApp {
                name: if name.is_empty() {
                    format!("PID {pid}")
                } else {
                    name
                },
                pid,
                desktop_id: None,
                exec: None,
            }
        })
        .collect()
}

pub(crate) fn kill(pid: u32) -> Result<String, String> {
    let output = std::process::Command::new("kill")
        .arg("-9")
        .arg(pid.to_string())
        .output()
        .map_err(|e| format!("Failed to kill: {e}"))?;

    if output.status.success() {
        Ok(format!("Killed PID {pid}"))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("Failed to kill PID {pid}: {}", stderr.trim()))
    }
}

// --- Desktop file scanning ---

struct DesktopEntry {
    name: String,
    exec: String,
    /// `StartupWMClass=` field, used to match against X11 windows' `WM_CLASS`
    /// when /proc-name → Exec matching fails (LibreOffice, Postman, Discord,
    /// and friends). None if the desktop file doesn't declare it.
    wm_class: Option<String>,
    path: String,
}

fn scan_desktop_files() -> Vec<DesktopEntry> {
    let mut entries = Vec::new();
    let dirs = xdg_app_dirs();
    for dir in &dirs {
        scan_desktop_dir(dir, &mut entries);
    }
    entries
}

fn scan_desktop_dir(dir: &str, entries: &mut Vec<DesktopEntry>) {
    let Ok(read_dir) = fs::read_dir(dir) else {
        return;
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(s) = path.to_str() {
                scan_desktop_dir(s, entries);
            }
            continue;
        }
        let Some(path_str) = path.to_str() else {
            continue;
        };
        if !path_str.ends_with(".desktop") {
            continue;
        }
        if let Some(de) = parse_desktop_entry(path_str) {
            entries.push(de);
        }
    }
}

fn parse_desktop_entry(path: &str) -> Option<DesktopEntry> {
    let content = fs::read_to_string(path).ok()?;
    let mut name = None;
    let mut exec = None;
    let mut wm_class = None;
    let mut no_display = false;
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
        if let Some(val) = line.strip_prefix("Name=") {
            if name.is_none() {
                name = Some(val.trim().to_string());
            }
        } else if let Some(val) = line.strip_prefix("Exec=") {
            exec = Some(val.trim().to_string());
        } else if let Some(val) = line.strip_prefix("StartupWMClass=") {
            wm_class = Some(val.trim().to_string());
        } else if let Some(val) = line.strip_prefix("NoDisplay=") {
            no_display = val.trim().eq_ignore_ascii_case("true");
        }
    }

    if no_display {
        return None;
    }

    Some(DesktopEntry {
        name: name?,
        exec: exec?,
        wm_class,
        path: path.to_string(),
    })
}

fn extract_bin_names(exec: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut tokens = exec
        .split_whitespace()
        .filter(|t| !t.contains('=') && !t.starts_with('%'));

    let Some(first) = tokens.next() else {
        return names;
    };
    let bin = Path::new(first)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(first);
    names.push(bin.to_string());

    // `gapplication launch org.gnome.Weather` — the real binary won't be
    // `gapplication`. Derive a kebab candidate from the app ID's last two
    // reverse-DNS segments (`org.gnome.Weather` → `gnome-weather`).
    if bin == "gapplication" {
        for t in tokens {
            if t == "launch" || t.starts_with("--") {
                continue;
            }
            if let Some(kebab) = kebab_from_appid(t) {
                names.push(kebab);
            }
            break;
        }
    }

    names
}

/// Convert a reverse-DNS app ID like `org.gnome.Weather` into a kebab-cased
/// binary candidate `gnome-weather`. Returns None for IDs with fewer than 2
/// dot-separated segments.
fn kebab_from_appid(id: &str) -> Option<String> {
    let segs: Vec<&str> = id.split('.').filter(|s| !s.is_empty()).collect();
    if segs.len() < 2 {
        return None;
    }
    let n = segs.len();
    Some(format!("{}-{}", segs[n - 2], segs[n - 1]).to_lowercase())
}

/// Derive a kebab binary candidate from a desktop file path: the file stem
/// is treated as a reverse-DNS app ID. Returns None if the stem isn't dotted.
fn kebab_from_desktop_stem(path: &str) -> Option<String> {
    let stem = Path::new(path).file_stem().and_then(|s| s.to_str())?;
    kebab_from_appid(stem)
}

/// Build all matching candidates for a /proc/<pid>/status `Name:` field.
///
/// Always yields the lowercased original. For NixOS-style wrappers (binaries
/// renamed `.<bin>-wrapped`), `Name:` is truncated to 15 chars and the
/// `-wrapped` suffix may appear as anything from `-wrapped` down to just `-`
/// (or vanish entirely when `<bin>` is long enough to fill the 14 chars after
/// the dot). Yield the unwrapped base in that case too.
fn normalize_proc_name(proc_name: &str) -> Vec<String> {
    let mut out = vec![proc_name.to_lowercase()];
    let Some(stripped) = proc_name.strip_prefix('.') else {
        return out;
    };
    // Try each truncation of "-wrapped" as a suffix. Longest first so we don't
    // accidentally strip a single `-` from a name that actually ended in `-w*`.
    for sfx in [
        "-wrapped", "-wrappe", "-wrapp", "-wrap", "-wra", "-wr", "-w",
    ] {
        if let Some(base) = stripped.strip_suffix(sfx)
            && !base.is_empty()
        {
            out.push(base.to_lowercase());
            return out;
        }
    }
    // Trailing `-` is the most ambiguous case (could be a `-wrapped` truncation
    // OR a legitimate dash-ending name). Only treat it as a wrapper suffix
    // when the leading dot is present — which is already guaranteed here.
    if let Some(base) = stripped.strip_suffix('-')
        && !base.is_empty()
    {
        out.push(base.to_lowercase());
        return out;
    }
    // No wrapper suffix; emit the dot-stripped form as a candidate.
    if !stripped.is_empty() {
        out.push(stripped.to_lowercase());
    }
    out
}

fn xdg_app_dirs() -> Vec<String> {
    let mut dirs = Vec::new();
    let home = std::env::var("HOME").unwrap_or_default();

    if !home.is_empty() {
        dirs.push(format!("{home}/.local/share/applications"));
    }

    if let Ok(data_dirs) = std::env::var("XDG_DATA_DIRS") {
        for dir in data_dirs.split(':') {
            let d = dir.trim();
            if !d.is_empty() {
                dirs.push(format!("{d}/applications"));
            }
        }
    } else {
        dirs.push("/usr/share/applications".to_string());
        dirs.push("/usr/local/share/applications".to_string());
    }

    // NixOS
    if !home.is_empty() {
        let nix = format!("{home}/.nix-profile/share/applications");
        if Path::new(&nix).is_dir() && !dirs.contains(&nix) {
            dirs.push(nix);
        }
    }
    let sys = "/run/current-system/sw/share/applications".to_string();
    if Path::new(&sys).is_dir() && !dirs.contains(&sys) {
        dirs.push(sys);
    }

    dirs
}

fn read_my_uid() -> u32 {
    fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| parse_status_field(&s, "Uid:"))
        .and_then(|v| v.parse().ok())
        .unwrap_or(u32::MAX)
}

fn parse_status_field(status: &str, prefix: &str) -> Option<String> {
    status
        .lines()
        .find(|l| l.starts_with(prefix))
        .and_then(|l| l.split_whitespace().nth(1))
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nixos_wrapper_truncations() {
        // 15-char /proc/comm field, NixOS .<bin>-wrapped pattern.
        assert!(normalize_proc_name(".firefox-wrappe").contains(&"firefox".to_string()));
        assert!(normalize_proc_name(".nautilus-wrapp").contains(&"nautilus".to_string()));
        assert!(normalize_proc_name(".ghostty-wrappe").contains(&"ghostty".to_string()));
        // Trailing dash from `.gnome-weather-wrapped` truncated to 15 chars.
        assert!(normalize_proc_name(".gnome-weather-").contains(&"gnome-weather".to_string()));
        // No `-wrapped` suffix visible — base fills the 14 chars.
        assert!(normalize_proc_name(".gnome-calendar").contains(&"gnome-calendar".to_string()));
        // Non-wrapper name passes through.
        assert_eq!(normalize_proc_name("firefox"), vec!["firefox"]);
    }

    #[test]
    fn gapplication_launch_yields_kebab() {
        let names = extract_bin_names("gapplication launch org.gnome.Weather");
        assert!(names.contains(&"gapplication".to_string()));
        assert!(names.contains(&"gnome-weather".to_string()));
    }

    #[test]
    fn direct_exec_unchanged() {
        let names = extract_bin_names("gnome-calendar %U");
        assert_eq!(names, vec!["gnome-calendar"]);
    }

    #[test]
    fn gjs_appid_truncated_proc_name_matches_via_15char_fallback() {
        // GNOME Weather (GJS) shows up in /proc as `org.gnome.Weath` (15 chars
        // truncated from `org.gnome.Weather`). The desktop file is
        // `org.gnome.Weather.desktop`; lowercased stem `org.gnome.weather`
        // truncated to 15 chars is `org.gnome.weath`. Verify the truncation
        // produces the proc-name form a real Weather process exposes.
        let key = "org.gnome.weather";
        let trunc: String = key.chars().take(15).collect();
        assert_eq!(trunc, "org.gnome.weath");
        assert!(trunc.len() < key.len());
    }

    #[test]
    fn nixos_wrapper_14char_truncation_matches_long_binary() {
        // For a binary like `gnome-text-editor` (17 chars), the NixOS wrapper
        // form `.gnome-text-editor-wrapped` is truncated by /proc/<pid>/comm to
        // `.gnome-text-edi` — the leading dot eats one slot, leaving only 14
        // chars of the base. `normalize_proc_name` strips the dot and yields a
        // 14-char key. The 15-char truncation of "gnome-text-editor" produces
        // "gnome-text-edit" which DOESN'T match, so the lookup must also try
        // 14 chars to catch this case.
        let proc_name = ".gnome-text-edi"; // exactly what /proc shows on NixOS
        let candidates = normalize_proc_name(proc_name);
        assert!(
            candidates.contains(&"gnome-text-edi".to_string()),
            "stripped form must be in candidates"
        );
        let key = "gnome-text-editor".to_lowercase();
        let trunc15: String = key.chars().take(15).collect();
        let trunc14: String = key.chars().take(14).collect();
        assert_ne!(trunc15, "gnome-text-edi", "15-char trunc doesn't help here");
        assert_eq!(trunc14, "gnome-text-edi", "14-char trunc is the match");
    }

    #[test]
    fn primary_desktop_entries_sort_before_game_shortcuts() {
        // Steam game shortcuts (`~/.local/share/applications/Dota 2.desktop`
        // with `Exec=steam steam://run/570`) shouldn't shadow `steam.desktop`
        // when matching against the `steam` /proc entry. The sort key in
        // `list()` prioritizes desktop entries whose file stem matches a bin
        // name in their own Exec. Mirror that logic here as a regression check.
        let entries = vec![
            DesktopEntry {
                name: "Dota 2".into(),
                exec: "steam steam://run/570".into(),
                wm_class: None,
                path: "/home/u/.local/share/applications/Dota 2.desktop".into(),
            },
            DesktopEntry {
                name: "Steam".into(),
                exec: "steam %U".into(),
                wm_class: None,
                path: "/usr/share/applications/steam.desktop".into(),
            },
        ];
        let mut sorted = entries;
        sorted.sort_by_key(|de| {
            let stem = std::path::Path::new(&de.path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();
            let bins = extract_bin_names(&de.exec);
            let primary = bins.iter().any(|b| b.to_lowercase() == stem);
            (if primary { 0 } else { 1 }, stem)
        });
        assert_eq!(sorted[0].name, "Steam", "primary entry must come first");
        assert_eq!(sorted[1].name, "Dota 2");
    }

    #[test]
    fn wm_class_keys_cover_libreoffice_and_discord() {
        // LibreOffice Writer: declared StartupWMClass is the match anchor.
        let writer = DesktopEntry {
            name: "LibreOffice Writer".into(),
            exec: "libreoffice --writer %U".into(),
            wm_class: Some("libreoffice-writer".into()),
            path: "/usr/share/applications/libreoffice-writer.desktop".into(),
        };
        let keys = desktop_wm_class_keys(&writer);
        assert!(
            keys.contains(&"libreoffice-writer".to_string()),
            "StartupWMClass must be a matching key"
        );

        // Discord: no StartupWMClass declared; the file stem alone must
        // match the window's WM_CLASS instance (`discord`).
        let discord = DesktopEntry {
            name: "Discord".into(),
            exec: "/usr/bin/discord %U".into(),
            wm_class: None,
            path: "/usr/share/applications/discord.desktop".into(),
        };
        let keys = desktop_wm_class_keys(&discord);
        assert!(
            keys.contains(&"discord".to_string()),
            "file stem must be a matching key when StartupWMClass is absent"
        );

        // Reverse-DNS desktop stems should also produce a kebab fallback
        // (org.gnome.TextEditor → gnome-texteditor) for GTK apps whose
        // WM_CLASS is the short form.
        let te = DesktopEntry {
            name: "Text Editor".into(),
            exec: "gnome-text-editor %U".into(),
            wm_class: None,
            path: "/usr/share/applications/org.gnome.TextEditor.desktop".into(),
        };
        let keys = desktop_wm_class_keys(&te);
        assert!(
            keys.contains(&"gnome-texteditor".to_string()),
            "kebab from reverse-DNS stem must be a matching key"
        );
    }

    #[test]
    fn kebab_from_stem() {
        assert_eq!(
            kebab_from_desktop_stem("/usr/share/applications/org.gnome.Weather.desktop"),
            Some("gnome-weather".to_string())
        );
        assert_eq!(
            kebab_from_desktop_stem("/x/org.gnome.Calendar.desktop"),
            Some("gnome-calendar".to_string())
        );
        // Single-segment stems (e.g., `firefox.desktop`) yield None.
        assert_eq!(kebab_from_desktop_stem("/x/firefox.desktop"), None);
    }
}
