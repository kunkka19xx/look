use crate::config::RuntimeConfig;
use crate::index::APP_CANDIDATE_ID_PREFIX;
use crate::platform::paths::{
    candidate_id_path_component, expand_with_home, path_is_same_or_child,
};
use crate::platform::windows;
use look_indexing::{Candidate, CandidateKind};
use std::collections::HashSet;
use std::fs;
use std::sync::mpsc;

pub(crate) fn discover_installed_apps(config: &RuntimeConfig, tx: mpsc::SyncSender<Candidate>) {
    let roots = windows::merged_app_scan_roots(
        &config.app_scan_roots,
        &windows::additional_app_scan_roots(),
        windows::REQUIRED_APP_SCAN_ROOTS,
    );

    let mut seen_ids = HashSet::new();
    for root in roots {
        walk_windows_app_entries(
            &root,
            config.app_scan_depth,
            &tx,
            &config.app_exclude_paths,
            &config.app_exclude_names,
            &mut seen_ids,
        );
    }

    let home = windows::user_home_dir();
    for fallback_root in windows::APP_FALLBACK_SCAN_ROOTS {
        let expanded_root = expand_with_home(fallback_root, home.as_deref());
        walk_windows_fallback_roots(
            &expanded_root,
            config.app_scan_depth,
            &tx,
            &config.app_exclude_paths,
            &config.app_exclude_names,
            &mut seen_ids,
        );
    }
}

fn walk_windows_app_entries(
    path: &str,
    depth: usize,
    tx: &mpsc::SyncSender<Candidate>,
    app_exclude_paths: &[String],
    app_exclude_names: &[String],
    seen_ids: &mut HashSet<String>,
) {
    if should_exclude_path(path, app_exclude_paths) || depth == 0 {
        return;
    }

    let Ok(entries) = fs::read_dir(path) else {
        return;
    };

    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        let path_buf = entry.path();
        let Some(path_str) = path_buf.to_str() else {
            continue;
        };
        if should_exclude_path(path_str, app_exclude_paths) {
            continue;
        }

        if file_type.is_dir() {
            walk_windows_app_entries(
                path_str,
                depth - 1,
                tx,
                app_exclude_paths,
                app_exclude_names,
                seen_ids,
            );
            continue;
        }

        if !is_windows_start_menu_entry(path_str) {
            continue;
        }

        emit_windows_app_candidate(path_str, tx, app_exclude_names, seen_ids, true);
    }
}

fn walk_windows_fallback_roots(
    path: &str,
    depth: usize,
    tx: &mpsc::SyncSender<Candidate>,
    app_exclude_paths: &[String],
    app_exclude_names: &[String],
    seen_ids: &mut HashSet<String>,
) {
    if should_exclude_path(path, app_exclude_paths) || depth == 0 {
        return;
    }

    // Read the current directory
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };

    // Iterate through everything in this directory
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        let path_buf = entry.path();
        let Some(path_str) = path_buf.to_str() else {
            continue;
        };

        if should_exclude_path(path_str, app_exclude_paths) {
            continue;
        }

        // IF IT IS A DIRECTORY: Recursively scan it, subtracting 1 from depth
        if file_type.is_dir() {
            walk_windows_fallback_roots(
                path_str,
                depth - 1,
                tx,
                app_exclude_paths,
                app_exclude_names,
                seen_ids,
            );
            continue;
        }

        // IF IT IS A FILE: Check if it's an executable we want
        if file_type.is_file() && is_windows_fallback_executable(path_str) {
            emit_windows_app_candidate(path_str, tx, app_exclude_names, seen_ids, false);
        }
    }
}

fn emit_windows_app_candidate(
    path: &str,
    tx: &mpsc::SyncSender<Candidate>,
    app_exclude_names: &[String],
    seen_ids: &mut HashSet<String>,
    is_primary_source: bool,
) {
    let title = std::path::Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("App")
        .to_string();

    if should_exclude_app_name(&title, app_exclude_names) {
        return;
    }

    let normalized_identity = normalize_app_name(&title);
    let path_id = candidate_id_path_component(path);

    if is_primary_source {
        // Primary Source (Start Menu): Claim a "Title Lock" to block the fallback later.
        // This now works for both .lnk AND .exe files found in the Start Menu!
        let title_lock = format!("title:{}", normalized_identity);
        if !seen_ids.insert(title_lock) {
            return;
        }
    } else {
        // Fallback Source: Check if the Start Menu already provided an entry
        let title_lock = format!("title:{}", normalized_identity);
        if seen_ids.contains(&title_lock) {
            return;
        }

        // Claim a "Path Lock" allowing identical vendor app names to coexist safely
        let path_lock = format!("path:{}", path_id);
        if !seen_ids.insert(path_lock) {
            return;
        }
    }

    // Globally unique candidate id
    let key = format!("{APP_CANDIDATE_ID_PREFIX}{normalized_identity}_{path_id}");

    let _ = tx.send(Candidate::new(&key, CandidateKind::App, &title, path));
}

fn is_windows_noise_executable(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.contains("uninstall")
        || lower.contains("setup")
        || lower.contains("updater")
        || lower.contains("crashpad")
}

fn is_windows_start_menu_entry(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    if !(lower.ends_with(".lnk") || lower.ends_with(".url") || lower.ends_with(".exe")) {
        return false;
    }
    // Apply the noise filter here!
    !is_windows_noise_executable(path)
}

fn is_windows_fallback_executable(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    if !lower.ends_with(".exe") {
        return false;
    }

    !is_windows_noise_executable(path)
}

fn should_exclude_path(path: &str, app_exclude_paths: &[String]) -> bool {
    app_exclude_paths.iter().any(|entry| {
        let normalized_exclude = entry.trim();
        if normalized_exclude.is_empty() {
            return false;
        }
        path_is_same_or_child(path, normalized_exclude)
    })
}

fn should_exclude_app_name(name: &str, app_exclude_names: &[String]) -> bool {
    let normalized_name = normalize_app_name(name);
    app_exclude_names.iter().any(|entry| {
        let normalized_exclude = normalize_app_name(entry);
        !normalized_exclude.is_empty() && normalized_exclude == normalized_name
    })
}

fn normalize_app_name(value: &str) -> String {
    let normalized = value.trim().to_lowercase();
    let mut stripped = normalized.as_str();
    for suffix in [".app", ".exe", ".lnk", ".url"] {
        if let Some(prefix) = stripped.strip_suffix(suffix) {
            stripped = prefix;
            break;
        }
    }
    stripped.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_menu_entry_extensions_are_detected() {
        assert!(is_windows_start_menu_entry("C:/Programs/App.lnk"));
        assert!(is_windows_start_menu_entry("C:/Programs/App.url"));
        assert!(is_windows_start_menu_entry("C:/Programs/App.EXE"));
        assert!(!is_windows_start_menu_entry("C:/Programs/App.txt"));
    }

    #[test]
    fn fallback_executable_filter_skips_noise_binaries() {
        assert!(is_windows_fallback_executable(
            "C:/Program Files/App/app.exe"
        ));
        assert!(!is_windows_fallback_executable(
            "C:/Program Files/App/uninstall.exe"
        ));
        assert!(!is_windows_fallback_executable(
            "C:/Program Files/App/setup.exe"
        ));
        assert!(!is_windows_fallback_executable(
            "C:/Program Files/App/updater.exe"
        ));
        assert!(!is_windows_fallback_executable(
            "C:/Program Files/App/crashpad_handler.exe"
        ));
    }

    #[test]
    fn exclude_path_matching_handles_windows_separators() {
        let excludes = vec!["C:\\Users\\demo\\AppData\\Local\\Programs".to_string()];
        assert!(should_exclude_path(
            "C:/Users/demo/AppData/Local/Programs/MyApp/app.exe",
            &excludes
        ));
    }

    #[test]
    fn emit_candidate_deduplicates_by_candidate_id() {
        let (tx, rx) = mpsc::sync_channel(8);
        let mut seen = HashSet::new();
        let excludes = Vec::new();

        emit_windows_app_candidate(
            "C:/Programs/MyApp/MyApp.exe",
            &tx,
            &excludes,
            &mut seen,
            false,
        );
        emit_windows_app_candidate(
            "C:/Programs/MyApp/MyApp.exe",
            &tx,
            &excludes,
            &mut seen,
            false,
        );

        drop(tx);
        let emitted: Vec<Candidate> = rx.into_iter().collect();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].title.as_ref(), "MyApp");
    }

    #[test]
    fn emit_candidate_deduplicates_cross_source() {
        let (tx, rx) = mpsc::sync_channel(8);
        let mut seen = HashSet::new();
        let excludes = Vec::new();

        // Source 1: Start menu shortcut
        emit_windows_app_candidate(
            "C:/Users/demo/Start Menu/Programs/MyApp.lnk",
            &tx,
            &excludes,
            &mut seen,
            true,
        );

        // Source 2: Install root fallback
        emit_windows_app_candidate(
            "C:/Program Files/MyApp/MyApp.exe",
            &tx,
            &excludes,
            &mut seen,
            false,
        );

        drop(tx);
        let emitted: Vec<Candidate> = rx.into_iter().collect();

        // This will now successfully assert to 1 instead of failing!
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].title.as_ref(), "MyApp");
    }

    #[test]
    fn exclude_name_matching_accepts_windows_suffixes() {
        let excludes = vec!["MyApp.exe".to_string(), "Another.lnk".to_string()];
        assert!(should_exclude_app_name("MyApp", &excludes));
        assert!(should_exclude_app_name("Another.url", &excludes));
        assert!(!should_exclude_app_name("Different", &excludes));
    }
}
