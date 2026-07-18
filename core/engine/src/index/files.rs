use crate::config::RuntimeConfig;
use crate::index::{FILE_CANDIDATE_ID_PREFIX, FOLDER_CANDIDATE_ID_PREFIX};
use crate::platform::paths::{
    PathPolicy, candidate_id_path_component, compile_ignore_glob, path_is_same_or_child,
};
use globset::{GlobSet, GlobSetBuilder};
use ignore::WalkBuilder;
use look_indexing::{Candidate, CandidateKind};
use std::sync::mpsc;
use std::time::UNIX_EPOCH;

// One `GlobSet` per path policy: a candidate is normalized once per distinct
// policy and matched against all patterns of that policy in a single pass.
type IgnoredFileMatchers = Vec<(PathPolicy, GlobSet)>;

fn modified_unix_s(metadata: Option<&std::fs::Metadata>) -> Option<i64> {
    metadata?
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs() as i64)
}

pub fn discover_local_files_and_folders(config: &RuntimeConfig, tx: mpsc::SyncSender<Candidate>) {
    let mut roots = config.file_scan_roots.clone();
    roots.extend(config.file_scan_extra_roots.iter().cloned());
    roots.sort();
    roots.dedup();

    // Compile the ignore matchers once, not once per root.
    let ignored_file_matchers = build_ignored_matchers(&config.ignored_file_patterns);

    let mut file_count = 0usize;
    for root in &roots {
        walk_files(root, config, &ignored_file_matchers, &tx, &mut file_count);
        if file_count >= config.file_scan_limit {
            break;
        }
    }
}

fn walk_files(
    path: &str,
    config: &RuntimeConfig,
    ignored_file_matchers: &IgnoredFileMatchers,
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

            // Name-based skip applies to anything (dir, file, junction).
            // walkdir's `file_type` doesn't follow Windows reparse points, so
            // gating on `is_dir()` here lets `Documents\My Music` (a junction
            // to `~\Music`) leak through and become a duplicate folder result.
            if name.ends_with(".app") || should_skip_dir(name, &skip_dir_names) {
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

        // One stat per entry, reused for both kind and mtime. `fs::metadata`
        // follows symlinks, matching the old `Path::is_dir`/`is_file` calls,
        // which each did their own separate stat on top of the mtime read.
        let Ok(metadata) = std::fs::metadata(&path_buf) else {
            continue;
        };
        let modified_at = modified_unix_s(Some(&metadata));

        if metadata.is_dir() {
            let key = format!(
                "{FOLDER_CANDIDATE_ID_PREFIX}{}",
                candidate_id_path_component(path_str)
            );
            let mut candidate = Candidate::new(&key, CandidateKind::Folder, name, path_str);
            candidate.subtitle = Some(CandidateKind::Folder.as_str().into());
            candidate.fs_modified_at_unix_s = modified_at;
            let _ = tx.send(candidate);
            continue;
        }

        if metadata.is_file() {
            // Normalize the candidate with the same policy used for the
            // pattern. Without this, a Windows pattern like `C:\tmp\*.log`
            // becomes lowercase `c:/...`, while walker output like
            // `C:/tmp/debug.log` can keep its original case and miss the glob.
            if ignored_file_matchers.iter().any(|(policy, glob_set)| {
                glob_set.is_match(&*policy.normalize_for_matching(path_str))
            }) {
                continue;
            }

            *file_count += 1;
            let key = format!(
                "{FILE_CANDIDATE_ID_PREFIX}{}",
                candidate_id_path_component(path_str)
            );
            let mut candidate = Candidate::new(&key, CandidateKind::File, name, path_str);
            candidate.subtitle = Some(CandidateKind::File.as_str().into());
            candidate.fs_modified_at_unix_s = modified_at;
            let _ = tx.send(candidate);
        }
    }
}

fn build_ignored_matchers(patterns: &[String]) -> IgnoredFileMatchers {
    let mut groups: Vec<(PathPolicy, GlobSetBuilder)> = Vec::new();
    for (policy, glob) in patterns
        .iter()
        .filter_map(|pattern| compile_ignore_glob(pattern))
    {
        match groups.iter_mut().find(|(existing, _)| *existing == policy) {
            Some((_, builder)) => {
                builder.add(glob);
            }
            None => {
                let mut builder = GlobSetBuilder::new();
                builder.add(glob);
                groups.push((policy, builder));
            }
        }
    }
    groups
        .into_iter()
        .filter_map(|(policy, builder)| builder.build().ok().map(|set| (policy, set)))
        .collect()
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
    use super::{discover_local_files_and_folders, should_exclude_path};
    use crate::config::RuntimeConfig;
    use crate::platform::paths::compile_ignore_matcher;
    use look_indexing::CandidateKind;
    use std::sync::mpsc;

    fn temp_path(prefix: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "look-files-test-{prefix}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos()
        ))
    }

    fn windows_style_ignored_pattern_matches(pattern: &str, path: &str) -> bool {
        let Some((policy, matcher)) = compile_ignore_matcher(pattern) else {
            return false;
        };
        let normalized_path = policy.normalize_for_matching(path);
        matcher.is_match(&*normalized_path)
    }

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

    #[test]
    fn ignored_patterns_skip_matching_files_but_keep_other_entries() {
        let root = temp_path("ignored-patterns");
        let nested = root.join("nested");
        std::fs::create_dir_all(&nested).expect("should create test directories");
        std::fs::write(root.join("debug.log"), "log").expect("should write log file");
        std::fs::write(root.join("notes.md"), "notes").expect("should write notes file");
        std::fs::write(nested.join("state.db-wal"), "wal").expect("should write wal file");

        let config = RuntimeConfig {
            file_scan_roots: vec![root.to_string_lossy().into_owned()],
            file_scan_depth: 4,
            file_scan_limit: 100,
            ignored_file_patterns: vec![
                root.join("*.log").to_string_lossy().into_owned(),
                root.join("**")
                    .join("*.db-wal")
                    .to_string_lossy()
                    .into_owned(),
            ],
            ..Default::default()
        };

        let (tx, rx) = mpsc::sync_channel(128);
        discover_local_files_and_folders(&config, tx);
        let candidates = rx.try_iter().collect::<Vec<_>>();

        assert!(candidates.iter().any(|candidate| {
            candidate.kind == CandidateKind::File && candidate.title.as_ref() == "notes.md"
        }));
        assert!(candidates.iter().any(|candidate| {
            candidate.kind == CandidateKind::Folder && candidate.title.as_ref() == "nested"
        }));
        assert!(
            !candidates
                .iter()
                .any(|candidate| candidate.title.as_ref() == "debug.log")
        );
        assert!(
            !candidates
                .iter()
                .any(|candidate| candidate.title.as_ref() == "state.db-wal")
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn ignored_patterns_do_not_consume_file_scan_limit() {
        let root = temp_path("ignored-pattern-limit");
        std::fs::create_dir_all(&root).expect("should create test directory");
        std::fs::write(root.join("debug.log"), "log").expect("should write ignored file");
        std::fs::write(root.join("notes.md"), "notes").expect("should write kept file");

        let config = RuntimeConfig {
            file_scan_roots: vec![root.to_string_lossy().into_owned()],
            file_scan_depth: 2,
            file_scan_limit: 1,
            ignored_file_patterns: vec![root.join("*.log").to_string_lossy().into_owned()],
            ..Default::default()
        };

        let (tx, rx) = mpsc::sync_channel(32);
        discover_local_files_and_folders(&config, tx);
        let candidates = rx.try_iter().collect::<Vec<_>>();

        let file_titles = candidates
            .iter()
            .filter(|candidate| candidate.kind == CandidateKind::File)
            .map(|candidate| candidate.title.as_ref())
            .collect::<Vec<_>>();
        assert_eq!(file_titles, vec!["notes.md"]);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn ignored_patterns_folder_scoped_glob_only_matches_direct_children() {
        // Case:
        // - root/direct.tmp
        // - root/nested/child.tmp
        // - ignored_file_patterns = [root/*.tmp]
        let root = temp_path("ignored-pattern-direct-children");
        let nested = root.join("nested");
        std::fs::create_dir_all(&nested).expect("should create test directories");
        std::fs::write(root.join("direct.tmp"), "tmp").expect("should write direct file");
        std::fs::write(nested.join("child.tmp"), "tmp").expect("should write nested file");

        let config = RuntimeConfig {
            file_scan_roots: vec![root.to_string_lossy().into_owned()],
            file_scan_depth: 4,
            file_scan_limit: 10,
            ignored_file_patterns: vec![root.join("*.tmp").to_string_lossy().into_owned()],
            ..Default::default()
        };

        let (tx, rx) = mpsc::sync_channel(32);
        discover_local_files_and_folders(&config, tx);
        let candidates = rx.try_iter().collect::<Vec<_>>();

        assert!(
            !candidates
                .iter()
                .any(|candidate| candidate.title.as_ref() == "direct.tmp")
        );
        assert!(
            candidates
                .iter()
                .any(|candidate| candidate.title.as_ref() == "child.tmp")
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn ignored_patterns_windows_style_paths_match_consistently() {
        // Case:
        // - pattern = C:\Users\me\AppData\Local\Temp\**\*.etl
        // - path = C:/Users/me/AppData/Local/Temp/nested/trace.etl
        // - unrelated = C:/Users/me/AppData/Local/Temp/nested/trace.log
        assert!(windows_style_ignored_pattern_matches(
            r"C:\Users\me\AppData\Local\Temp\**\*.etl",
            "C:/Users/me/AppData/Local/Temp/nested/trace.etl"
        ));
        assert!(!windows_style_ignored_pattern_matches(
            r"C:\Users\me\AppData\Local\Temp\**\*.etl",
            "C:/Users/me/AppData/Local/Temp/nested/trace.log"
        ));
    }

    #[test]
    fn ignored_patterns_match_windows_backslash_pattern_against_slash_candidate() {
        let pattern = r"C:\Users\me\Temp\*.log";
        let (policy, matcher) = compile_ignore_matcher(pattern).expect("pattern should compile");

        assert!(matcher.is_match(&*policy.normalize_for_matching("C:/Users/me/Temp/debug.log")));
        assert!(!matcher.is_match(&*policy.normalize_for_matching("C:/Users/me/Temp/debug.txt")));
    }
}
