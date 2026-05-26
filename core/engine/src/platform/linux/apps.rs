use crate::config::RuntimeConfig;
use crate::index::APP_CANDIDATE_ID_PREFIX;
use crate::platform::linux;
use crate::platform::paths::candidate_id_path_component;
use look_indexing::{Candidate, CandidateKind};
use std::collections::HashSet;
use std::fs;
use std::sync::mpsc;

pub(crate) fn discover_installed_apps(config: &RuntimeConfig, tx: mpsc::SyncSender<Candidate>) {
    let mut seen = HashSet::new();

    for root in merged_app_scan_roots(&config.app_scan_roots, &linux::additional_app_scan_roots()) {
        scan_desktop_files(&root, &tx, &config.app_exclude_names, &mut seen);
    }
}

fn merged_app_scan_roots(config_roots: &[String], additional_roots: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(config_roots.len() + additional_roots.len());
    let mut seen = HashSet::with_capacity(config_roots.len() + additional_roots.len());
    for root in config_roots.iter().chain(additional_roots.iter()) {
        let normalized = candidate_id_path_component(root);
        if seen.insert(normalized) {
            out.push(root.clone());
        }
    }
    out
}

fn scan_desktop_files(
    dir: &str,
    tx: &mpsc::SyncSender<Candidate>,
    exclude_names: &[String],
    seen: &mut HashSet<String>,
) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(path_str) = path.to_str() else {
            continue;
        };

        if path.is_dir() {
            scan_desktop_files(path_str, tx, exclude_names, seen);
            continue;
        }

        if !path_str.ends_with(".desktop") {
            continue;
        }

        let Some(app) = parse_desktop_file(path_str) else {
            continue;
        };

        if app.no_display || app.hidden {
            continue;
        }

        if should_exclude_name(&app.name, exclude_names) {
            continue;
        }

        if !seen.insert(app.name.to_lowercase()) {
            continue;
        }

        let key = format!(
            "{APP_CANDIDATE_ID_PREFIX}{}",
            candidate_id_path_component(path_str)
        );

        let exec_path = app.exec.as_deref().unwrap_or(path_str);
        let mut candidate = Candidate::new(&key, CandidateKind::App, &app.name, exec_path);
        candidate.subtitle = Some("App".into());
        let _ = tx.send(candidate);
    }
}

struct DesktopEntry {
    name: String,
    exec: Option<String>,
    no_display: bool,
    hidden: bool,
}

fn parse_desktop_file(path: &str) -> Option<DesktopEntry> {
    let content = fs::read_to_string(path).ok()?;
    let mut name = None;
    let mut exec = None;
    let mut no_display = false;
    let mut hidden = false;
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
                name = Some(val.to_string());
            }
        } else if let Some(val) = line.strip_prefix("Exec=") {
            // Strip field codes like %f %u %F %U
            let clean = val
                .split_whitespace()
                .filter(|s| !s.starts_with('%'))
                .collect::<Vec<_>>()
                .join(" ");
            exec = Some(clean);
        } else if let Some(val) = line.strip_prefix("NoDisplay=") {
            no_display = val.trim().eq_ignore_ascii_case("true");
        } else if let Some(val) = line.strip_prefix("Hidden=") {
            hidden = val.trim().eq_ignore_ascii_case("true");
        }
    }

    Some(DesktopEntry {
        name: name?,
        exec,
        no_display,
        hidden,
    })
}

fn should_exclude_name(name: &str, exclude_names: &[String]) -> bool {
    let normalized = name.trim().to_lowercase();
    exclude_names
        .iter()
        .any(|e| !e.trim().is_empty() && e.trim().to_lowercase() == normalized)
}
