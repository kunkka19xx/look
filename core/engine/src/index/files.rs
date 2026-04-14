use crate::config::RuntimeConfig;
use crate::index::{FILE_CANDIDATE_ID_PREFIX, FOLDER_CANDIDATE_ID_PREFIX};
use crate::platform::paths::{candidate_id_path_component, path_is_same_or_child};
use ignore::WalkBuilder;
use look_indexing::{Candidate, CandidateKind};
use std::sync::mpsc;

pub fn discover_local_files_and_folders(config: &RuntimeConfig, tx: mpsc::SyncSender<Candidate>) {
    let roots = &config.file_scan_roots;

    let mut file_count = 0usize;
    for root in roots {
        walk_files(root, config, &tx, &mut file_count);
        if file_count >= config.file_scan_limit {
            break;
        }
    }
}

fn walk_files(
    path: &str,
    config: &RuntimeConfig,
    tx: &mpsc::SyncSender<Candidate>,
    file_count: &mut usize,
) {
    if should_exclude_path(path, &config.file_exclude_paths) {
        return;
    }

    let mut walker = WalkBuilder::new(path);
    let root_path = path.to_string();
    let exclude_paths = config.file_exclude_paths.clone();
    let skip_dir_names = config.skip_dir_names.clone();
    walker
        .hidden(false)
        .git_ignore(false)
        .git_global(false)
        .git_exclude(false)
        .parents(false)
        .max_depth(Some(config.file_scan_depth))
        .filter_entry(move |entry| {
            let path_buf = entry.path();
            let Some(path_str) = path_buf.to_str() else {
                return false;
            };

            if path_str == root_path {
                return true;
            }

            if should_exclude_path(path_str, &exclude_paths) {
                return false;
            }

            let Some(name) = path_buf.file_name().and_then(|value| value.to_str()) else {
                return false;
            };

            if name.starts_with('.') {
                return false;
            }

            if entry
                .file_type()
                .map(|file_type| file_type.is_dir())
                .unwrap_or(false)
                && (name.ends_with(".app") || should_skip_dir(name, &skip_dir_names))
            {
                return false;
            }

            true
        });

    for entry in walker.build().flatten() {
        if *file_count >= config.file_scan_limit {
            break;
        }

        let path_buf = entry.into_path();
        let Some(path_str) = path_buf.to_str() else {
            continue;
        };
        if path_str == path {
            continue;
        }

        let Some(name) = path_buf.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if path_buf.is_dir() {
            let key = format!(
                "{FOLDER_CANDIDATE_ID_PREFIX}{}",
                candidate_id_path_component(path_str)
            );
            let mut candidate = Candidate::new(&key, CandidateKind::Folder, name, path_str);
            candidate.subtitle = Some(CandidateKind::Folder.as_str().into());
            let _ = tx.send(candidate);
            continue;
        }

        if path_buf.is_file() {
            *file_count += 1;
            let key = format!(
                "{FILE_CANDIDATE_ID_PREFIX}{}",
                candidate_id_path_component(path_str)
            );
            let mut candidate = Candidate::new(&key, CandidateKind::File, name, path_str);
            candidate.subtitle = Some(CandidateKind::File.as_str().into());
            let _ = tx.send(candidate);
        }
    }
}

fn should_skip_dir(name: &str, skip_dir_names: &[String]) -> bool {
    let lower = name.to_lowercase();
    skip_dir_names.iter().any(|entry| entry == &lower)
}

fn should_exclude_path(path: &str, file_exclude_paths: &[String]) -> bool {
    file_exclude_paths.iter().any(|entry| {
        let normalized_exclude = entry.trim();
        if normalized_exclude.is_empty() {
            return false;
        }
        path_is_same_or_child(path, normalized_exclude)
    })
}

#[cfg(test)]
mod tests {
    use super::should_exclude_path;

    #[test]
    fn excludes_nested_paths_and_exact_matches() {
        let excludes = vec!["/Users/demo/Downloads/tmp".to_string()];
        assert!(should_exclude_path("/Users/demo/Downloads/tmp", &excludes));
        assert!(should_exclude_path(
            "/Users/demo/Downloads/tmp/cache/file.txt",
            &excludes
        ));
    }

    #[test]
    fn does_not_exclude_unrelated_paths() {
        let excludes = vec!["/Users/demo/Downloads/tmp".to_string()];
        assert!(!should_exclude_path(
            "/Users/demo/Downloads/template",
            &excludes
        ));
        assert!(!should_exclude_path(
            "/Users/demo/Documents/report.md",
            &excludes
        ));
    }

    #[test]
    fn handles_trailing_slashes_and_blank_entries() {
        let excludes = vec!["/Users/demo/Downloads/tmp/".to_string(), " ".to_string()];
        assert!(should_exclude_path("/Users/demo/Downloads/tmp", &excludes));
        assert!(should_exclude_path(
            "/Users/demo/Downloads/tmp/cache/a.txt",
            &excludes
        ));
    }

    #[test]
    fn path_prefix_is_boundary_aware() {
        let excludes = vec!["/Users/demo/Down".to_string()];
        assert!(!should_exclude_path("/Users/demo/Downloads", &excludes));
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn exclude_path_matching_supports_windows_style_separators() {
        let excludes = vec!["C:\\Users\\demo\\Downloads".to_string()];
        assert!(should_exclude_path(
            "C:/Users/demo/Downloads/cache/a.txt",
            &excludes
        ));
    }
}
