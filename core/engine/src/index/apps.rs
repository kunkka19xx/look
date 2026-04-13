use crate::config::RuntimeConfig;
use crate::index::APP_CANDIDATE_ID_PREFIX;
use look_indexing::{Candidate, CandidateKind};
use std::env;
use std::fs;
use std::sync::mpsc;

const FINDER_EMBEDDED_APPS_ROOT: &str =
    "/System/Library/CoreServices/Finder.app/Contents/Applications";
const CORE_SERVICES_APPS_ROOT: &str = "/System/Library/CoreServices/Applications";

fn ensure_required_roots(roots: &mut Vec<String>) {
    for required in [FINDER_EMBEDDED_APPS_ROOT, CORE_SERVICES_APPS_ROOT] {
        if !roots.iter().any(|root| root == required) {
            roots.push(required.to_string());
        }
    }
}

pub fn discover_installed_apps(config: &RuntimeConfig, tx: mpsc::SyncSender<Candidate>) {
    let mut roots = config.app_scan_roots.clone();
    if let Ok(home) = env::var("HOME") {
        let home_apps = format!("{home}/Applications");
        if !roots.iter().any(|root| root == &home_apps) {
            roots.push(home_apps);
        }
    }

    ensure_required_roots(&mut roots);

    for root in roots {
        walk_apps(
            &root,
            config.app_scan_depth,
            &tx,
            &config.app_exclude_paths,
            &config.app_exclude_names,
        );
    }
}

fn walk_apps(
    path: &str,
    depth: usize,
    tx: &mpsc::SyncSender<Candidate>,
    app_exclude_paths: &[String],
    app_exclude_names: &[String],
) {
    if should_exclude_path(path, app_exclude_paths) {
        return;
    }

    if depth == 0 {
        return;
    }

    let Ok(entries) = fs::read_dir(path) else {
        return;
    };

    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }

        let app_path = entry.path();
        let Some(app_path_str) = app_path.to_str() else {
            continue;
        };
        if should_exclude_path(app_path_str, app_exclude_paths) {
            continue;
        }

        if app_path_str.ends_with(".app") {
            let title = app_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("App")
                .to_string();
            if should_exclude_app_name(&title, app_exclude_names) {
                continue;
            }

            let key = format!("{APP_CANDIDATE_ID_PREFIX}{}", app_path_str.to_lowercase());
            let _ = tx.send(Candidate::new(
                &key,
                CandidateKind::App,
                &title,
                app_path_str,
            ));
        } else {
            walk_apps(
                app_path_str,
                depth - 1,
                tx,
                app_exclude_paths,
                app_exclude_names,
            );
        }
    }
}

fn should_exclude_path(path: &str, app_exclude_paths: &[String]) -> bool {
    let normalized_path = path.trim_end_matches('/');
    app_exclude_paths.iter().any(|entry| {
        let normalized_exclude = entry.trim().trim_end_matches('/');
        if normalized_exclude.is_empty() {
            return false;
        }
        normalized_path == normalized_exclude
            || normalized_path.starts_with(&format!("{normalized_exclude}/"))
    })
}

fn should_exclude_app_name(name: &str, app_exclude_names: &[String]) -> bool {
    let normalized_name = name.trim().trim_end_matches(".app").trim().to_lowercase();
    app_exclude_names.iter().any(|entry| {
        let normalized_exclude = entry.trim().trim_end_matches(".app").trim().to_lowercase();
        !normalized_exclude.is_empty() && normalized_exclude == normalized_name
    })
}

#[cfg(test)]
mod tests {
    use super::{ensure_required_roots, should_exclude_app_name, should_exclude_path};

    #[test]
    fn excludes_app_paths_by_prefix() {
        let excludes = vec!["/Applications/Utilities".to_string()];
        assert!(should_exclude_path("/Applications/Utilities", &excludes));
        assert!(should_exclude_path(
            "/Applications/Utilities/Terminal.app",
            &excludes
        ));
    }

    #[test]
    fn excludes_app_names_case_insensitively() {
        let names = vec!["safari".to_string(), "Visual Studio Code".to_string()];
        assert!(should_exclude_app_name("Safari", &names));
        assert!(should_exclude_app_name("Visual Studio Code.app", &names));
        assert!(!should_exclude_app_name("Calculator", &names));
    }

    #[test]
    fn ignores_blank_exclude_entries() {
        let excludes = vec!["  ".to_string(), "".to_string()];
        assert!(!should_exclude_path("/Applications/Utilities", &excludes));

        let names = vec![" ".to_string(), "".to_string()];
        assert!(!should_exclude_app_name("Safari", &names));
    }

    #[test]
    fn path_prefix_is_boundary_aware() {
        let excludes = vec!["/Applications/Util".to_string()];
        assert!(!should_exclude_path("/Applications/Utilities", &excludes));
    }

    #[test]
    fn required_roots_are_appended_once() {
        let mut roots = vec!["/Applications".to_string()];
        ensure_required_roots(&mut roots);
        ensure_required_roots(&mut roots);

        assert!(
            roots
                .iter()
                .any(|root| root == "/System/Library/CoreServices/Applications")
        );
        assert!(roots
            .iter()
            .any(|root| root == "/System/Library/CoreServices/Finder.app/Contents/Applications"));

        let core_services_count = roots
            .iter()
            .filter(|root| *root == "/System/Library/CoreServices/Applications")
            .count();
        assert_eq!(core_services_count, 1);
    }
}
