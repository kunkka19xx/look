mod apps;
mod settings_catalog;

use std::env;

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
pub(crate) const SETTINGS_BUNDLE_EXTENSION: &str = ".appex";
pub(crate) const SETTINGS_EXTENSION_NAME: &str = "appex";
const INFO_PLIST_STRINGS_TABLE: &str = "InfoPlist";
const DISPLAY_NAME_KEYS: [&str; 2] = ["CFBundleDisplayName", "CFBundleName"];

pub(crate) use apps::discover_installed_apps;
pub(crate) use settings_catalog::SETTINGS_CATALOG;

/// macOS 13+ only. Settings panes use `.appex` extensions under
/// `/System/Library/ExtensionKit/Extensions/`. On macOS 12 and earlier,
/// they use `.prefPane` bundles in `/System/Library/PreferencePanes/`.
pub(crate) const SETTINGS_EXTENSIONS_DIR: &str = "/System/Library/ExtensionKit/Extensions";

pub(crate) fn localized_names_available() -> bool {
    objc2::available!(macos = 15.4)
}

pub(crate) fn additional_app_scan_roots() -> Vec<String> {
    env::var("HOME")
        .ok()
        .filter(|home| !home.trim().is_empty())
        .map(|home| vec![format!("{home}/Applications")])
        .unwrap_or_default()
}

/// Returns the bundle name localized for the user's preferred languages.
pub(crate) fn read_localized_display_name(path: &str, strip_suffix: &str) -> Option<String> {
    if !localized_names_available() {
        return None;
    }
    let path_string = NSString::from_str(path);
    let bundle = NSBundle::bundleWithPath(&path_string)?;
    read_bundle_name_for_user_languages(&bundle, path, strip_suffix)
}

pub(crate) fn read_bundle_identifier(path: &str) -> Option<String> {
    let path_string = NSString::from_str(path);
    let bundle = NSBundle::bundleWithPath(&path_string)?;
    bundle
        .bundleIdentifier()
        .map(|identifier| identifier.to_string())
}

fn read_bundle_name_for_user_languages(
    bundle: &NSBundle,
    path: &str,
    strip_suffix: &str,
) -> Option<String> {
    let localizations = NSLocale::preferredLanguages();
    read_bundle_name_for_languages(bundle, path, strip_suffix, &localizations)
}

fn read_bundle_name_for_languages(
    bundle: &NSBundle,
    path: &str,
    strip_suffix: &str,
    localizations: &NSArray<NSString>,
) -> Option<String> {
    let table = NSString::from_str(INFO_PLIST_STRINGS_TABLE);

    for key in DISPLAY_NAME_KEYS {
        let key = NSString::from_str(key);
        let fallback = bundle
            .objectForInfoDictionaryKey(&key)
            .and_then(|value| value.downcast::<NSString>().ok());
        let name = bundle.localizedStringForKey_value_table_localizations(
            &key,
            fallback.as_deref(),
            Some(&table),
            localizations,
        );
        if fallback.is_none() && name == key {
            continue;
        }
        if let Some(name) = normalize_display_name(&name.to_string(), path, strip_suffix) {
            return Some(name);
        }
    }
    None
}

fn normalize_display_name(display_name: &str, path: &str, strip_suffix: &str) -> Option<String> {
    let name = display_name.trim();
    if name.is_empty() || name == path {
        return None;
    }

    let name = name.strip_suffix(strip_suffix).unwrap_or(name).trim();
    (!name.is_empty()).then(|| name.to_string())
}

#[cfg(test)]
mod tests {
    use super::{APP_BUNDLE_EXTENSION, localized_names_available, read_bundle_name_for_languages};
    use objc2_foundation::{NSArray, NSBundle, NSString};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempBundle(PathBuf);

    impl Drop for TempBundle {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn bundle_name_uses_explicit_localization_for_infoplist_strings() {
        if !localized_names_available() {
            eprintln!("skipping: localized bundle names need macOS 15.4+");
            return;
        }

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos();
        let app = TempBundle(std::env::temp_dir().join(format!(
            "look-localized-name-{unique}{APP_BUNDLE_EXTENSION}"
        )));
        let resources = app.0.join("Contents/Resources/zh-Hans.lproj");
        fs::create_dir_all(&resources).expect("create test bundle resources");
        fs::write(
            app.0.join("Contents/Info.plist"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>test.look.localized-name</string>
<key>CFBundleDisplayName</key><string>English Fixture</string>
</dict></plist>"#,
        )
        .expect("write test bundle Info.plist");
        fs::write(
            resources.join("InfoPlist.strings"),
            "\"CFBundleDisplayName\" = \"\u{672c}\u{5730}\u{5316}\u{6d4b}\u{8bd5}\";\n",
        )
        .expect("write localized display name");

        let app_path = app.0.to_str().expect("UTF-8 test bundle path");
        let app_path_string = NSString::from_str(app_path);
        let bundle = NSBundle::bundleWithPath(&app_path_string).expect("load test bundle");
        let language = NSString::from_str("zh-Hans");
        let languages = NSArray::from_slice(&[&*language]);
        let name =
            read_bundle_name_for_languages(&bundle, app_path, APP_BUNDLE_EXTENSION, &languages);

        assert_eq!(
            name.as_deref(),
            Some("\u{672c}\u{5730}\u{5316}\u{6d4b}\u{8bd5}")
        );
    }
}
