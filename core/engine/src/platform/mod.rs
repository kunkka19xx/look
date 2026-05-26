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
    platform_impl::SETTINGS_CATALOG
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
        // Settings catalog targets gnome-control-center panels.
        // Only show them on DEs that actually use it (GNOME, Budgie, etc.),
        // not on standalone WMs (sway, Hyprland, i3) where it may be
        // installed as a dependency but doesn't integrate properly.
        let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
        let on_gnome_de = desktop.split(':').any(|s| {
            matches!(
                s.trim(),
                "GNOME" | "Budgie" | "Cinnamon" | "Unity" | "Pantheon"
            )
        });
        if !on_gnome_de {
            return false;
        }
        use std::process::Command;
        Command::new("which")
            .arg("gnome-control-center")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}
