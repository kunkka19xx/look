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
        // Original name
        norm_procs
            .entry(proc_name.to_lowercase())
            .or_default()
            .extend(pids);
        // Strip NixOS wrapper: ".firefox-wrappe" → "firefox", ".DiscordPTB-wra" → "DiscordPTB"
        // The /proc/status Name field is truncated to 15 chars, so "-wrapped" may
        // appear as "-wrappe", "-wrapp", "-wrap", "-wra", "-wr" etc.
        let stripped = proc_name.strip_prefix('.').unwrap_or(proc_name);
        let base = if let Some(pos) = stripped.find("-wr") {
            // Verify the suffix looks like truncated "-wrapped"
            let suffix = &stripped[pos..];
            if "-wrapped".starts_with(suffix) {
                &stripped[..pos]
            } else {
                stripped
            }
        } else {
            stripped
        };
        if !base.is_empty() && base != proc_name {
            norm_procs
                .entry(base.to_lowercase())
                .or_default()
                .extend(pids);
        }
    }

    // 3. Scan .desktop files, match Exec against running process names
    let desktop_entries = scan_desktop_files();
    let mut apps: Vec<RunningApp> = Vec::new();
    let mut matched_keys: std::collections::HashSet<String> = std::collections::HashSet::new();

    for de in &desktop_entries {
        let bin_names = extract_bin_names(&de.exec);
        for bin in &bin_names {
            let key = bin.to_lowercase();
            if matched_keys.contains(&key) {
                continue;
            }
            // /proc/<pid>/comm is limited to TASK_COMM_LEN-1 == 15 chars, so
            // `gnome-text-editor` shows up as `gnome-text-edit`. Match the
            // truncated form too when the desktop Exec is longer.
            let pids = norm_procs.get(&key).or_else(|| {
                let trunc: String = key.chars().take(15).collect();
                if trunc.len() < key.len() {
                    norm_procs.get(&trunc)
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
        path: path.to_string(),
    })
}

fn extract_bin_names(exec: &str) -> Vec<String> {
    let mut names = Vec::new();
    // First token is the command (may have env vars, prefixes)
    for token in exec.split_whitespace() {
        if token.contains('=') || token.starts_with('%') {
            continue; // skip env vars and field codes
        }
        let bin = Path::new(token)
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or(token);
        names.push(bin.to_string());
        break;
    }
    names
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
