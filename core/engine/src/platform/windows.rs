use super::SettingsCatalogEntry;

pub(crate) const APP_SCAN_ROOTS: &[&str] = &[
    "C:/ProgramData/Microsoft/Windows/Start Menu/Programs",
    "~/AppData/Roaming/Microsoft/Windows/Start Menu/Programs",
];

pub(crate) const REQUIRED_APP_SCAN_ROOTS: &[&str] = &[];

pub(crate) const FILE_SCAN_ROOT_SUFFIXES: &[&str] = &["Desktop", "Documents", "Downloads"];

pub(crate) const SETTINGS_URL_SCHEME_PREFIX: &str = "ms-settings:";
pub(crate) const SETTINGS_SUBTITLE_PREFIX: &str = "Windows Settings ";

pub(crate) const SETTINGS_CATALOG: &[SettingsCatalogEntry] = &[];

pub(crate) fn additional_app_scan_roots() -> Vec<String> {
    Vec::new()
}
