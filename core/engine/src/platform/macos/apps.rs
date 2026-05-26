use crate::config::RuntimeConfig;
use crate::index::APP_CANDIDATE_ID_PREFIX;
use crate::platform::macos;
use crate::platform::paths::{candidate_id_path_component, path_is_same_or_child};
use look_indexing::{Candidate, CandidateKind};
use std::collections::HashSet;
use std::fs;
use std::sync::mpsc;

pub(crate) fn discover_installed_apps(config: &RuntimeConfig, tx: mpsc::SyncSender<Candidate>) {
    for root in merged_app_scan_roots(
        &config.app_scan_roots,
        &macos::additional_app_scan_roots(),
        macos::REQUIRED_APP_SCAN_ROOTS,
    ) {
        walk_apps(
            &root,
            config.app_scan_depth,
            &tx,
            &config.app_exclude_paths,
            &config.app_exclude_names,
        );
    }
}

fn merged_app_scan_roots(
    config_roots: &[String],
    additional_roots: &[String],
    required_roots: &[&str],
) -> Vec<String> {
    let mut out =
        Vec::with_capacity(config_roots.len() + additional_roots.len() + required_roots.len());
    let mut seen =
        HashSet::with_capacity(config_roots.len() + additional_roots.len() + required_roots.len());
    for root in config_roots.iter().chain(additional_roots.iter()) {
        let normalized = candidate_id_path_component(root);
        if seen.insert(normalized) {
            out.push(root.clone());
        }
    }

    for root in required_roots {
        let normalized = candidate_id_path_component(root);
        if seen.insert(normalized) {
            out.push((*root).to_string());
        }
    }

    out
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

            let key = format!(
                "{APP_CANDIDATE_ID_PREFIX}{}",
                candidate_id_path_component(app_path_str)
            );
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
    app_exclude_paths.iter().any(|entry| {
        let normalized_exclude = entry.trim();
        if normalized_exclude.is_empty() {
            return false;
        }
        path_is_same_or_child(path, normalized_exclude)
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
    use super::{merged_app_scan_roots, should_exclude_app_name, should_exclude_path};

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
    fn merged_roots_preserve_order_and_deduplicate() {
        let roots = vec!["/Applications".to_string()];
        let additional = vec![
            "/Users/demo/Applications".to_string(),
            "/Applications/".to_string(),
        ];

        let required = vec!["/System/Library/CoreServices/Applications", "/Applications"];

        let merged = merged_app_scan_roots(&roots, &additional, &required);
        assert_eq!(
            merged,
            vec![
                "/Applications".to_string(),
                "/Users/demo/Applications".to_string(),
                "/System/Library/CoreServices/Applications".to_string()
            ]
        );
    }
}
