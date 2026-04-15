pub(crate) mod paths;

#[cfg(not(target_os = "windows"))]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(not(target_os = "windows"))]
use macos as platform_impl;
#[cfg(target_os = "windows")]
use windows as platform_impl;

pub(crate) struct SettingsCatalogEntry {
    pub(crate) title: &'static str,
    pub(crate) target: &'static str,
    pub(crate) candidate_id_suffix: &'static str,
    pub(crate) aliases: &'static str,
}

pub(crate) fn app_scan_roots() -> &'static [&'static str] {
    platform_impl::APP_SCAN_ROOTS
}

#[cfg(target_os = "windows")]
pub(crate) fn discover_windows_installed_apps(
    config: &crate::config::RuntimeConfig,
    tx: std::sync::mpsc::SyncSender<look_indexing::Candidate>,
) {
    windows::discover_installed_apps(config, tx)
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn discover_macos_installed_apps(
    config: &crate::config::RuntimeConfig,
    tx: std::sync::mpsc::SyncSender<look_indexing::Candidate>,
) {
    macos::discover_installed_apps(config, tx)
}

pub(crate) fn file_scan_root_suffixes() -> &'static [&'static str] {
    platform_impl::FILE_SCAN_ROOT_SUFFIXES
}

pub(crate) fn settings_url_scheme_prefix() -> &'static str {
    platform_impl::SETTINGS_URL_SCHEME_PREFIX
}

pub(crate) fn settings_subtitle_prefix() -> &'static str {
    platform_impl::SETTINGS_SUBTITLE_PREFIX
}

pub(crate) fn settings_catalog() -> &'static [SettingsCatalogEntry] {
    platform_impl::SETTINGS_CATALOG
}
