use crate::config::RuntimeConfig;
use crate::index::APP_CANDIDATE_ID_PREFIX;
use crate::platform::macos;
use crate::platform::paths::{candidate_id_path_component, path_is_same_or_child};
use look_indexing::{Candidate, CandidateKind};
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::mpsc;

/// Used when a bundle path has no readable file stem, which should not happen
/// for a real `.app` but keeps the candidate from having an empty title.
const FALLBACK_APP_NAME: &str = "App";

pub(crate) fn discover_installed_apps(config: &RuntimeConfig, tx: mpsc::SyncSender<Candidate>) {
    let mut bundles = Vec::new();
    for root in merged_app_scan_roots(
        &config.app_scan_roots,
        &macos::additional_app_scan_roots(),
        macos::REQUIRED_APP_SCAN_ROOTS,
    ) {
        collect_app_bundles(
            &root,
            config.app_scan_depth,
            &config.app_exclude_paths,
            &mut bundles,
        );
    }

    // Resolved in one batch rather than inline in the walk: each name costs an
    // Info.plist read, and batching lets those run on a thread pool.
    let localized = if config.localized_app_names {
        macos::read_localized_display_names(&bundles, macos::APP_BUNDLE_EXTENSION)
    } else {
        vec![None; bundles.len()]
    };

    for (path, localized) in bundles.iter().zip(localized) {
        emit_app(&tx, path, localized, &config.app_exclude_names);
    }
}

fn emit_app(
    tx: &mpsc::SyncSender<Candidate>,
    path: &str,
    localized: Option<String>,
    app_exclude_names: &[String],
) {
    let bundle_name = Path::new(path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(FALLBACK_APP_NAME);
    let title = localized.unwrap_or_else(|| bundle_name.to_string());
    if should_exclude_app(&title, bundle_name, app_exclude_names) {
        return;
    }

    let key = format!(
        "{APP_CANDIDATE_ID_PREFIX}{}",
        candidate_id_path_component(path)
    );
    let _ = tx.send(Candidate::new(&key, CandidateKind::App, &title, path));
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

/// Appends every `.app` bundle under `path` to `out`. Naming is left to the
/// caller so the Info.plist reads can be batched across the whole scan instead
/// of happening one directory entry at a time.
fn collect_app_bundles(
    path: &str,
    depth: usize,
    app_exclude_paths: &[String],
    out: &mut Vec<String>,
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

        let app_path = entry.path();
        let is_dir = file_type.is_dir();
        let is_symlink_dir =
            file_type.is_symlink() && fs::metadata(&app_path).map(|m| m.is_dir()).unwrap_or(false);
        if !is_dir && !is_symlink_dir {
            continue;
        }

        let Some(app_path_str) = app_path.to_str() else {
            continue;
        };
        if should_exclude_path(app_path_str, app_exclude_paths) {
            continue;
        }

        if app_path_str.ends_with(macos::APP_BUNDLE_EXTENSION) {
            out.push(app_path_str.to_string());
        } else if is_dir {
            collect_app_bundles(app_path_str, depth - 1, app_exclude_paths, out);
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

fn normalize_app_name(name: &str) -> String {
    let name = name.trim();
    name.strip_suffix(macos::APP_BUNDLE_EXTENSION)
        .unwrap_or(name)
        .trim()
        .to_lowercase()
}

fn should_exclude_app_name(name: &str, app_exclude_names: &[String]) -> bool {
    let normalized_name = normalize_app_name(name);
    app_exclude_names.iter().any(|entry| {
        let normalized_exclude = normalize_app_name(entry);
        !normalized_exclude.is_empty() && normalized_exclude == normalized_name
    })
}

/// Excludes on either name: `app_exclude_names` is written by hand, so it
/// holds whichever of the two the user happened to see in the launcher.
fn should_exclude_app(title: &str, bundle_name: &str, app_exclude_names: &[String]) -> bool {
    should_exclude_app_name(title, app_exclude_names)
        || should_exclude_app_name(bundle_name, app_exclude_names)
}

#[cfg(test)]
mod tests {
    use super::{
        collect_app_bundles, emit_app, merged_app_scan_roots, should_exclude_app,
        should_exclude_app_name, should_exclude_path,
    };
    use crate::platform::macos::test_support::TempDir;
    use look_indexing::Candidate;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::mpsc;

    fn create_app(root: &Path, name: &str) -> PathBuf {
        let app = root.join(name);
        fs::create_dir_all(app.join("Contents")).expect("create app contents");
        app
    }

    #[cfg(unix)]
    fn symlink_dir(target: &Path, link: &Path) {
        std::os::unix::fs::symlink(target, link).expect("create symlink");
    }

    /// Mirrors `discover_installed_apps` without a `RuntimeConfig`: walk, then
    /// emit with no localized names.
    fn collect_apps_excluding(root: &Path, app_exclude_names: &[String]) -> Vec<Candidate> {
        let mut bundles = Vec::new();
        collect_app_bundles(
            root.to_str().expect("utf-8 temp path"),
            3,
            &[],
            &mut bundles,
        );

        let (tx, rx) = mpsc::sync_channel(16);
        for path in &bundles {
            emit_app(&tx, path, None, app_exclude_names);
        }
        drop(tx);
        rx.into_iter().collect()
    }

    fn collect_apps(root: &Path) -> Vec<Candidate> {
        collect_apps_excluding(root, &[])
    }

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

    #[test]
    #[cfg(unix)]
    fn indexes_symlinked_app_bundle() {
        let tmp = TempDir::new("symlink-app");
        let real_root = tmp.path().join("Real Apps");
        let scan_root = tmp.path().join("Applications");
        fs::create_dir_all(&real_root).expect("create real root");
        fs::create_dir_all(&scan_root).expect("create scan root");
        let real_app = create_app(&real_root, "Riot Client.app");
        let link_app = scan_root.join("Client Riot.app");
        symlink_dir(&real_app, &link_app);

        let apps = collect_apps(&scan_root);

        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].title.as_ref(), "Client Riot");
        assert_eq!(
            apps[0].path.as_ref(),
            link_app.to_str().expect("utf-8 symlink path")
        );
    }

    #[test]
    #[cfg(unix)]
    fn does_not_recurse_into_non_app_symlinked_directory() {
        let tmp = TempDir::new("symlink-dir");
        let real_root = tmp.path().join("Real Apps");
        let scan_root = tmp.path().join("Applications");
        fs::create_dir_all(&real_root).expect("create real root");
        fs::create_dir_all(&scan_root).expect("create scan root");
        create_app(&real_root, "Nested.app");
        symlink_dir(&real_root, &scan_root.join("Linked Apps"));

        let apps = collect_apps(&scan_root);

        assert!(apps.is_empty());
    }

    #[test]
    fn excludes_app_by_bundle_filename() {
        let tmp = TempDir::new("exclude-filename");
        create_app(tmp.path(), "Client Riot.app");

        let apps = collect_apps_excluding(tmp.path(), &["Client Riot".to_string()]);

        assert!(apps.is_empty());
    }

    /// The walk and the name resolution are separate passes zipped back
    /// together, so the collected order is the contract between them.
    #[test]
    fn collected_bundles_are_the_app_directories_only() {
        let tmp = TempDir::new("collect-bundles");
        create_app(tmp.path(), "Alpha.app");
        create_app(tmp.path(), "Beta.app");
        fs::create_dir_all(tmp.path().join("Not An App")).expect("create plain dir");

        let mut bundles = Vec::new();
        collect_app_bundles(
            tmp.path().to_str().expect("utf-8 temp path"),
            3,
            &[],
            &mut bundles,
        );
        bundles.sort();

        assert_eq!(bundles.len(), 2);
        assert!(bundles[0].ends_with("Alpha.app"));
        assert!(bundles[1].ends_with("Beta.app"));
    }

    /// A localized title is used verbatim; the bundle file name is the fallback.
    #[test]
    fn emitted_title_prefers_the_localized_name() {
        let tmp = TempDir::new("emit-title");
        let app = create_app(tmp.path(), "WeChat.app");
        let path = app.to_str().expect("utf-8 app path");

        let (tx, rx) = mpsc::sync_channel(2);
        emit_app(&tx, path, Some("微信".to_string()), &[]);
        emit_app(&tx, path, None, &[]);
        drop(tx);

        let titles: Vec<String> = rx.into_iter().map(|c| c.title.to_string()).collect();
        assert_eq!(titles, vec!["微信".to_string(), "WeChat".to_string()]);
    }

    #[test]
    fn excludes_app_when_bundle_name_differs_from_localized_title() {
        let excludes = vec!["WeChat".to_string()];
        assert!(should_exclude_app("微信", "WeChat", &excludes));
        assert!(should_exclude_app("WeChat", "微信", &excludes));
    }
}
