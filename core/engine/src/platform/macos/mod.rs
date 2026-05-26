mod apps;
mod settings_catalog;

use std::env;

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

pub(crate) fn additional_app_scan_roots() -> Vec<String> {
    env::var("HOME")
        .ok()
        .filter(|home| !home.trim().is_empty())
        .map(|home| vec![format!("{home}/Applications")])
        .unwrap_or_default()
}
