mod apps;
mod settings_catalog;

use std::env;
use std::process::Command;

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

pub(crate) use apps::discover_installed_apps;
pub(crate) use settings_catalog::SETTINGS_CATALOG;

/// macOS 13+ only. Settings panes use `.appex` extensions under
/// `/System/Library/ExtensionKit/Extensions/`. On macOS 12 and earlier,
/// they use `.prefPane` bundles in `/System/Library/PreferencePanes/`
/// and localization via Spotlight is unavailable.
pub(crate) const SETTINGS_EXTENSIONS_DIR: &str = "/System/Library/ExtensionKit/Extensions";

pub(crate) fn additional_app_scan_roots() -> Vec<String> {
    env::var("HOME")
        .ok()
        .filter(|home| !home.trim().is_empty())
        .map(|home| vec![format!("{home}/Applications")])
        .unwrap_or_default()
}

/// Returns the Spotlight `kMDItemDisplayName` for the given path, with
/// the file suffix (`.app` or `.appex`) stripped. Returns `None` on
/// any failure — the caller should fall back to plist or filename stem.
pub(crate) fn read_spotlight_display_name(path: &str, strip_suffix: &str) -> Option<String> {
    let output = Command::new("mdls")
        .args(["-name", "kMDItemDisplayName", "-raw", path])
        .output()
        .ok()?;
    if !output.status.success() {
        #[cfg(debug_assertions)]
        eprintln!(
            "look index: mdls failed for {path}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return None;
    }
    let raw = String::from_utf8(output.stdout).ok()?;
    let name = raw.trim().strip_suffix(strip_suffix).unwrap_or(raw.trim());
    if name.is_empty() {
        return None;
    }
    Some(name.to_string())
}

pub(crate) fn read_app_display_name(app_path: &str, use_spotlight: bool) -> Option<String> {
    if use_spotlight && let Some(name) = read_spotlight_display_name(app_path, ".app") {
        return Some(name);
    }

    let plist_path = format!("{app_path}/Contents/Info.plist");
    let value = plist::Value::from_file(&plist_path).ok()?;
    let dict = value.as_dictionary()?;
    dict.get("CFBundleDisplayName")
        .or_else(|| dict.get("CFBundleName"))
        .and_then(|v| v.as_string())
        .map(|s| s.to_string())
}
