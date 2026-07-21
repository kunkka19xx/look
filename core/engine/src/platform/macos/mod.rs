mod apps;
mod settings_catalog;
#[cfg(test)]
mod test_support;

use std::collections::HashMap;
use std::env;
use std::path::Path;

use crate::platform::SettingsCatalogEntry;
use objc2::rc::Retained;
use objc2_foundation::{NSArray, NSBundle, NSLocale, NSString};

pub(crate) const APP_SCAN_ROOTS: &[&str] = &[
    "/Applications",
    "/System/Applications",
    "/System/Applications/Utilities",
    "/System/Library/CoreServices/Applications",
    "/System/Library/CoreServices/Finder.app/Contents/Applications",
];

pub(crate) const REQUIRED_APP_SCAN_ROOTS: &[&str] = &[
    "/System/Library/CoreServices/Applications",
    "/System/Library/CoreServices/Finder.app/Contents/Applications",
];

pub(crate) const FILE_SCAN_ROOT_SUFFIXES: &[&str] = &["Desktop", "Documents", "Downloads"];

pub(crate) const SETTINGS_URL_SCHEME_PREFIX: &str = "x-apple.systempreferences:";
pub(crate) const SETTINGS_SUBTITLE_PREFIX: &str = "System Settings ";
pub(crate) const APP_BUNDLE_EXTENSION: &str = ".app";
const SETTINGS_BUNDLE_EXTENSION: &str = ".appex";
const INFO_PLIST_STRINGS_TABLE: &str = "InfoPlist";
const DISPLAY_NAME_KEYS: [&str; 2] = ["CFBundleDisplayName", "CFBundleName"];

/// Reading a bundle's name costs one Info.plist read, which dominates both
/// scans and is independent per bundle, so the reads are spread over a small
/// pool. Measured cold on 92 apps: ~69ms on one thread, ~23ms on four. Capped
/// well under the core count because indexing is background work.
const MAX_BUNDLE_READ_THREADS: usize = 4;
/// Below this many bundles per thread, spawning costs more than it saves.
const MIN_BUNDLES_PER_READ_THREAD: usize = 8;

pub(crate) use apps::discover_installed_apps;
pub(crate) use settings_catalog::SETTINGS_CATALOG;

/// Settings panes ship as `.appex` bundles here on macOS 13+. On macOS 12 and
/// earlier they are `.prefPane` bundles in `/System/Library/PreferencePanes/`,
/// which this never reads: localized names need macOS 15.4+ anyway, so the
/// older layout can't reach this path.
const SETTINGS_EXTENSIONS_DIR: &str = "/System/Library/ExtensionKit/Extensions";

/// `localizedStringForKey:value:table:localizations:` is the only API that
/// resolves a bundle's name against an explicit language list rather than the
/// running process's language, and it landed in macOS 15.4.
fn localized_names_available() -> bool {
    objc2::available!(macos = 15.4)
}

pub(crate) fn additional_app_scan_roots() -> Vec<String> {
    env::var("HOME")
        .ok()
        .filter(|home| !home.trim().is_empty())
        .map(|home| vec![format!("{home}/Applications")])
        .unwrap_or_default()
}

/// Bundle names localized for the user's preferred languages, one entry per
/// input path and in the same order. `None` means the bundle has no usable
/// name and the caller should keep whatever it already had.
pub(crate) fn read_localized_display_names(
    paths: &[String],
    strip_suffix: &str,
) -> Vec<Option<String>> {
    if !localized_names_available() {
        return vec![None; paths.len()];
    }
    read_bundles(paths, |bundle, languages, _| {
        read_bundle_name_for_languages(bundle, strip_suffix, languages)
    })
}

/// Loads each path as a bundle and applies `read` to it, spreading the work
/// over a small thread pool. Returns one entry per input path, in input order.
fn read_bundles<T, F>(paths: &[String], read: F) -> Vec<Option<T>>
where
    T: Send,
    F: Fn(&NSBundle, &NSArray<NSString>, &str) -> Option<T> + Sync,
{
    let read_chunk = |chunk: &[String]| -> Vec<Option<T>> {
        // Hoisted per chunk rather than per bundle: the list is the same every
        // time and building it costs about as much as a bundle load.
        let languages = NSLocale::preferredLanguages();
        chunk
            .iter()
            .map(|path| {
                objc2::rc::autoreleasepool(|_| {
                    let bundle = load_bundle(path)?;
                    read(&bundle, &languages, path)
                })
            })
            .collect()
    };

    let threads = bundle_read_threads(paths.len());
    if threads == 1 {
        return read_chunk(paths);
    }

    let mut names = Vec::with_capacity(paths.len());
    std::thread::scope(|scope| {
        let workers: Vec<_> = paths
            .chunks(paths.len().div_ceil(threads))
            .map(|chunk| {
                let read_chunk = &read_chunk;
                scope.spawn(move || read_chunk(chunk))
            })
            .collect();
        for worker in workers {
            let Ok(chunk_names) = worker.join() else {
                eprintln!("look index: bundle metadata worker panicked");
                return;
            };
            names.extend(chunk_names);
        }
    });

    // Callers zip this against `paths`, so a short result would shift every
    // name onto the wrong bundle. A panicked worker costs one serial retry.
    if names.len() == paths.len() {
        names
    } else {
        read_chunk(paths)
    }
}

fn bundle_read_threads(bundle_count: usize) -> usize {
    let cores = std::thread::available_parallelism()
        .map(|cores| cores.get())
        .unwrap_or(1);
    bundle_count
        .div_ceil(MIN_BUNDLES_PER_READ_THREAD)
        .min(cores)
        .clamp(1, MAX_BUNDLE_READ_THREADS)
}

/// Maps every settings catalog target that ships a matching `.appex` bundle to
/// its localized pane name, keyed by `SettingsCatalogEntry::target`. Targets
/// whose localized name equals the catalog title (or the bundle's own file
/// name) are left out, so callers can treat a hit as "this differs from the
/// English title".
///
/// The whole scan lives here rather than in `index::settings` so Foundation
/// types stay behind the platform boundary, and so each bundle is loaded once
/// instead of once per property read.
pub(crate) fn localized_settings_titles(
    catalog: &'static [SettingsCatalogEntry],
) -> HashMap<&'static str, String> {
    localized_settings_titles_in(catalog, SETTINGS_EXTENSIONS_DIR)
}

fn localized_settings_titles_in(
    catalog: &'static [SettingsCatalogEntry],
    extensions_dir: &str,
) -> HashMap<&'static str, String> {
    if !localized_names_available() {
        return HashMap::new();
    }
    let Ok(entries) = std::fs::read_dir(extensions_dir) else {
        return HashMap::new();
    };

    let paths: Vec<String> = entries
        .flatten()
        .filter_map(|entry| entry.path().to_str().map(str::to_string))
        .filter(|path| path.ends_with(SETTINGS_BUNDLE_EXTENSION))
        .collect();

    let targets: HashMap<&str, &SettingsCatalogEntry> =
        catalog.iter().map(|entry| (entry.target, entry)).collect();

    read_bundles(&paths, |bundle, languages, path| {
        let identifier = bundle.bundleIdentifier()?.to_string();
        // Only catalog panes are worth the localized-name lookup: the directory
        // holds hundreds of extensions, the catalog around thirty.
        let entry = targets.get(identifier.as_str())?;
        let localized =
            read_bundle_name_for_languages(bundle, SETTINGS_BUNDLE_EXTENSION, languages)?;

        let stem = Path::new(path)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default();
        (localized != entry.title && localized != stem).then_some((entry.target, localized))
    })
    .into_iter()
    .flatten()
    .collect()
}

fn load_bundle(path: &str) -> Option<Retained<NSBundle>> {
    let path_string = NSString::from_str(path);
    NSBundle::bundleWithPath(&path_string)
}

fn read_bundle_name_for_languages(
    bundle: &NSBundle,
    strip_suffix: &str,
    localizations: &NSArray<NSString>,
) -> Option<String> {
    let table = NSString::from_str(INFO_PLIST_STRINGS_TABLE);

    for key in DISPLAY_NAME_KEYS {
        let key = NSString::from_str(key);
        // A blank Info.plist value has to collapse to `None`: passing an empty
        // string as the fallback makes `localizedStringForKey:` echo the key
        // back, which would otherwise be indexed as the app's title.
        let fallback = bundle
            .objectForInfoDictionaryKey(&key)
            .and_then(|value| value.downcast::<NSString>().ok())
            .filter(|value| !value.to_string().trim().is_empty());
        let name = bundle.localizedStringForKey_value_table_localizations(
            &key,
            fallback.as_deref(),
            Some(&table),
            localizations,
        );
        if fallback.is_none() && name == key {
            continue;
        }
        if let Some(name) = normalize_display_name(&name.to_string(), strip_suffix) {
            return Some(name);
        }
    }
    None
}

fn normalize_display_name(display_name: &str, strip_suffix: &str) -> Option<String> {
    let name = display_name.trim();
    let name = name.strip_suffix(strip_suffix).unwrap_or(name).trim();
    (!name.is_empty()).then(|| name.to_string())
}

#[cfg(test)]
mod tests {
    use super::test_support::TempDir;
    use super::{
        APP_BUNDLE_EXTENSION, MIN_BUNDLES_PER_READ_THREAD, SETTINGS_BUNDLE_EXTENSION,
        SettingsCatalogEntry, bundle_read_threads, load_bundle, localized_names_available,
        localized_settings_titles_in, read_bundle_name_for_languages, read_localized_display_names,
    };
    use objc2_foundation::{NSArray, NSString};
    use std::fs;
    use std::path::Path;

    /// "Localization test" in Simplified Chinese, to prove the name really came
    /// from the zh-Hans strings file rather than the Info.plist fallback.
    const LOCALIZED_FIXTURE_NAME: &str = "本地化测试";
    const FIXTURE_LANGUAGE: &str = "zh-Hans";

    /// Writes a bundle whose Info.plist carries `info_plist_body`, plus a
    /// `zh-Hans` InfoPlist.strings when `localized_name` is set.
    fn write_bundle(path: &Path, info_plist_body: &str, localized_name: Option<&str>) {
        let resources = path.join(format!("Contents/Resources/{FIXTURE_LANGUAGE}.lproj"));
        fs::create_dir_all(&resources).expect("create test bundle resources");
        fs::write(
            path.join("Contents/Info.plist"),
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
{info_plist_body}
</dict></plist>"#
            ),
        )
        .expect("write test bundle Info.plist");
        if let Some(localized_name) = localized_name {
            fs::write(
                resources.join("InfoPlist.strings"),
                format!("\"CFBundleDisplayName\" = \"{localized_name}\";\n"),
            )
            .expect("write localized display name");
        }
    }

    fn read_fixture_name(path: &Path, strip_suffix: &str) -> Option<String> {
        let path = path.to_str().expect("UTF-8 test bundle path");
        let bundle = load_bundle(path).expect("load test bundle");
        let language = NSString::from_str(FIXTURE_LANGUAGE);
        let languages = NSArray::from_slice(&[&*language]);
        read_bundle_name_for_languages(&bundle, strip_suffix, &languages)
    }

    #[test]
    fn bundle_name_uses_explicit_localization_for_infoplist_strings() {
        if !localized_names_available() {
            eprintln!("skipping: localized bundle names need macOS 15.4+");
            return;
        }

        let dir = TempDir::new("localized-name");
        let app = dir.path().join(format!("Fixture{APP_BUNDLE_EXTENSION}"));
        write_bundle(
            &app,
            "<key>CFBundleIdentifier</key><string>test.look.localized-name</string>
<key>CFBundleDisplayName</key><string>English Fixture</string>",
            Some(LOCALIZED_FIXTURE_NAME),
        );

        assert_eq!(
            read_fixture_name(&app, APP_BUNDLE_EXTENSION).as_deref(),
            Some(LOCALIZED_FIXTURE_NAME)
        );
    }

    /// A blank `CFBundleDisplayName` makes `localizedStringForKey:` echo the
    /// key back; the name must fall through to `CFBundleName` instead of being
    /// indexed as "CFBundleDisplayName".
    #[test]
    fn blank_display_name_falls_through_to_bundle_name() {
        if !localized_names_available() {
            eprintln!("skipping: localized bundle names need macOS 15.4+");
            return;
        }

        let dir = TempDir::new("blank-display-name");
        let app = dir.path().join(format!("Fixture{APP_BUNDLE_EXTENSION}"));
        write_bundle(
            &app,
            "<key>CFBundleIdentifier</key><string>test.look.blank-display-name</string>
<key>CFBundleDisplayName</key><string></string>
<key>CFBundleName</key><string>Real Name</string>",
            None,
        );

        assert_eq!(
            read_fixture_name(&app, APP_BUNDLE_EXTENSION).as_deref(),
            Some("Real Name")
        );
    }

    #[test]
    fn settings_titles_map_only_catalog_targets_with_a_differing_localized_name() {
        if !localized_names_available() {
            eprintln!("skipping: localized bundle names need macOS 15.4+");
            return;
        }

        static CATALOG: &[SettingsCatalogEntry] = &[SettingsCatalogEntry {
            title: "Fixture Pane",
            target: "test.look.settings-fixture",
            candidate_id_suffix: "test.look.settings-fixture",
            aliases: "settings fixture",
        }];

        let dir = TempDir::new("settings-extensions");
        write_bundle(
            &dir.path()
                .join(format!("Fixture{SETTINGS_BUNDLE_EXTENSION}")),
            "<key>CFBundleIdentifier</key><string>test.look.settings-fixture</string>
<key>CFBundleDisplayName</key><string>Fixture Pane</string>",
            Some(LOCALIZED_FIXTURE_NAME),
        );
        // Not in the catalog: must be skipped rather than indexed.
        write_bundle(
            &dir.path()
                .join(format!("Unlisted{SETTINGS_BUNDLE_EXTENSION}")),
            "<key>CFBundleIdentifier</key><string>test.look.unlisted</string>
<key>CFBundleDisplayName</key><string>Unlisted Pane</string>",
            Some("Unlisted Localized"),
        );

        let titles =
            localized_settings_titles_in(CATALOG, dir.path().to_str().expect("UTF-8 temp path"));

        assert_eq!(titles.len(), 1);
        assert_eq!(
            titles.get("test.look.settings-fixture").map(String::as_str),
            Some(LOCALIZED_FIXTURE_NAME)
        );
    }

    /// Callers zip the result against the input, so a batch spread over several
    /// threads has to come back one-per-input and in the original order. Sized
    /// past the per-thread threshold so the parallel path is the one under test.
    #[test]
    fn batched_names_stay_aligned_with_their_input_paths() {
        if !localized_names_available() {
            eprintln!("skipping: localized bundle names need macOS 15.4+");
            return;
        }

        let bundle_count = MIN_BUNDLES_PER_READ_THREAD * 3;
        assert!(
            bundle_read_threads(bundle_count) > 1,
            "fixture too small to exercise the threaded path"
        );

        let dir = TempDir::new("batched-names");
        let mut paths = Vec::new();
        for index in 0..bundle_count {
            let app = dir
                .path()
                .join(format!("Fixture{index}{APP_BUNDLE_EXTENSION}"));
            write_bundle(
                &app,
                &format!(
                    "<key>CFBundleIdentifier</key><string>test.look.batch{index}</string>
<key>CFBundleDisplayName</key><string>English {index}</string>"
                ),
                Some(&format!("{LOCALIZED_FIXTURE_NAME}{index}")),
            );
            paths.push(app.to_str().expect("UTF-8 test bundle path").to_string());
        }

        let names = read_localized_display_names(&paths, APP_BUNDLE_EXTENSION);

        assert_eq!(names.len(), paths.len());
        for (index, name) in names.iter().enumerate() {
            assert_eq!(
                name.as_deref(),
                Some(format!("{LOCALIZED_FIXTURE_NAME}{index}").as_str()),
                "bundle {index} came back out of order"
            );
        }
    }
}
