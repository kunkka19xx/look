mod settings_catalog;

pub(crate) const APP_SCAN_ROOTS: &[&str] = &[
    "/Applications",
    "/System/Applications",
    "/System/Applications/Utilities",
    "/System/Library/CoreServices/Applications",
    "/System/Library/CoreServices/Finder.app/Contents/Applications",
];

pub(crate) const FILE_SCAN_ROOT_SUFFIXES: &[&str] = &["Desktop", "Documents", "Downloads"];

pub(crate) const SETTINGS_URL_SCHEME_PREFIX: &str = "x-apple.systempreferences:";
pub(crate) const SETTINGS_SUBTITLE_PREFIX: &str = "System Settings ";

pub(crate) use settings_catalog::SETTINGS_CATALOG;
