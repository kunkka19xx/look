pub(crate) mod paths;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
use linux as platform_impl;
#[cfg(target_os = "macos")]
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

#[cfg(target_os = "windows")]
pub(crate) use windows::control_panel::ControlPanelEntry as WindowsControlPanelEntry;

#[cfg(target_os = "windows")]
pub(crate) fn windows_control_panel_catalog() -> &'static [WindowsControlPanelEntry] {
    windows::CONTROL_PANEL_CATALOG
}

#[cfg(target_os = "windows")]
pub(crate) fn windows_control_panel_target_path(entry: &WindowsControlPanelEntry) -> String {
    windows::control_panel_target_path(entry)
}

#[cfg(target_os = "macos")]
pub(crate) fn discover_macos_installed_apps(
    config: &crate::config::RuntimeConfig,
    tx: std::sync::mpsc::SyncSender<look_indexing::Candidate>,
) {
    macos::discover_installed_apps(config, tx)
}

#[cfg(target_os = "macos")]
pub(crate) use macos::localized_settings_titles;

#[cfg(target_os = "linux")]
pub(crate) fn discover_linux_installed_apps(
    config: &crate::config::RuntimeConfig,
    tx: std::sync::mpsc::SyncSender<look_indexing::Candidate>,
) {
    linux::discover_installed_apps(config, tx)
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
    #[cfg(target_os = "linux")]
    {
        linux::settings_catalog()
    }
    #[cfg(not(target_os = "linux"))]
    {
        platform_impl::SETTINGS_CATALOG
    }
}

#[cfg(test)]
pub(crate) fn all_settings_catalogs() -> Vec<&'static [SettingsCatalogEntry]> {
    #[cfg(target_os = "linux")]
    {
        linux::all_settings_catalogs().to_vec()
    }
    #[cfg(not(target_os = "linux"))]
    {
        vec![platform_impl::SETTINGS_CATALOG]
    }
}

pub(crate) fn settings_entry_available(_entry: &SettingsCatalogEntry) -> bool {
    #[cfg(target_os = "linux")]
    {
        linux::settings_entry_available(_entry)
    }
    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}

/// Check if the system has a settings app (e.g. gnome-control-center).
/// Returns false on i3, sway, or minimal distros without a DE settings app.
pub(crate) fn has_settings_app() -> bool {
    #[cfg(target_os = "macos")]
    {
        true // macOS always has System Settings
    }
    #[cfg(target_os = "windows")]
    {
        true // Windows always has Settings
    }
    #[cfg(target_os = "linux")]
    {
        linux::settings_app().is_some()
    }
}

/// Whether a Bluetooth controller is present. Cheap filesystem probe, no D-Bus:
/// BlueZ exposes each controller as `/sys/class/bluetooth/hciN`. Lets us surface
/// the Bluetooth control on desktops without a settings app (KDE, sway, i3, ...),
/// where its BlueZ-backed toggle and device list still work.
#[cfg(target_os = "linux")]
pub(crate) fn bluetooth_present() -> bool {
    std::fs::read_dir("/sys/class/bluetooth")
        .map(|mut entries| entries.any(|e| e.is_ok()))
        .unwrap_or(false)
}
