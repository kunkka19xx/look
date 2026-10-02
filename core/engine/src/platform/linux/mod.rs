mod apps;
mod gnome_settings_catalog;
mod kde_settings_catalog;

use crate::platform::SettingsCatalogEntry;
use gnome_settings_catalog::GNOME_SETTINGS_CATALOG;
use kde_settings_catalog::KDE_SETTINGS_CATALOG;
use std::collections::HashSet;
use std::env;
use std::process::{Command, Stdio};
use std::sync::OnceLock;

pub(crate) const APP_SCAN_ROOTS: &[&str] =
    &["/usr/share/applications", "/usr/local/share/applications"];

pub(crate) const FILE_SCAN_ROOT_SUFFIXES: &[&str] =
    &["Desktop", "Documents", "Downloads", "Pictures", "Videos"];

pub(crate) const SETTINGS_URL_SCHEME_PREFIX: &str = "settings://";
pub(crate) const SETTINGS_SUBTITLE_PREFIX: &str = "Settings ";

pub(crate) use apps::discover_installed_apps;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum SettingsApp {
    Gnome,
    Kde,
}

pub(crate) fn settings_app() -> Option<SettingsApp> {
    static APP: OnceLock<Option<SettingsApp>> = OnceLock::new();
    *APP.get_or_init(detect_settings_app)
}

fn detect_settings_app() -> Option<SettingsApp> {
    let desktop = env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let on = |names: &[&str]| {
        desktop
            .split(':')
            .any(|s| names.iter().any(|n| s.trim().eq_ignore_ascii_case(n)))
    };
    // Not on standalone WMs, where the app may be installed but doesn't integrate.
    if on(&["KDE"]) && on_path("systemsettings") {
        return Some(SettingsApp::Kde);
    }
    if on(&["GNOME", "Budgie", "Cinnamon", "Unity", "Pantheon"]) && on_path("gnome-control-center")
    {
        return Some(SettingsApp::Gnome);
    }
    None
}

// Not `which`: it isn't installed everywhere (e.g. Arch base).
fn on_path(program: &str) -> bool {
    use std::os::unix::fs::PermissionsExt;

    env::var_os("PATH").is_some_and(|path| {
        env::split_paths(&path).any(|dir| {
            dir.is_absolute()
                && std::fs::metadata(dir.join(program))
                    .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        })
    })
}

pub(crate) fn settings_catalog() -> &'static [SettingsCatalogEntry] {
    match settings_app() {
        Some(SettingsApp::Kde) => KDE_SETTINGS_CATALOG,
        _ => GNOME_SETTINGS_CATALOG,
    }
}

#[cfg(test)]
pub(crate) fn all_settings_catalogs() -> [&'static [SettingsCatalogEntry]; 2] {
    [GNOME_SETTINGS_CATALOG, KDE_SETTINGS_CATALOG]
}

/// KDE modules ship in optional packages, so filter to what's installed.
pub(crate) fn settings_entry_available(entry: &SettingsCatalogEntry) -> bool {
    match settings_app() {
        Some(SettingsApp::Kde) => {
            installed_kde_modules().is_none_or(|modules| modules.contains(entry.target))
        }
        _ => true,
    }
}

/// `None` if unreadable, which shows the whole catalog.
fn installed_kde_modules() -> Option<&'static HashSet<String>> {
    static MODULES: OnceLock<Option<HashSet<String>>> = OnceLock::new();
    MODULES
        .get_or_init(|| {
            let output = Command::new("systemsettings")
                .arg("--list")
                .env_remove("LD_LIBRARY_PATH")
                .stdin(Stdio::null())
                .stderr(Stdio::null())
                .output()
                .ok()
                .filter(|o| o.status.success())?;
            let modules: HashSet<String> = String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter_map(|line| line.split_whitespace().next())
                .filter(|id| id.starts_with("kcm"))
                .map(str::to_string)
                .collect();
            (!modules.is_empty()).then_some(modules)
        })
        .as_ref()
}

pub(crate) fn additional_app_scan_roots() -> Vec<String> {
    let mut roots = Vec::new();
    if let Ok(home) = env::var("HOME") {
        let home = home.trim();
        if !home.is_empty() {
            roots.push(format!("{home}/.local/share/applications"));
        }
    }
    if let Ok(data_dirs) = env::var("XDG_DATA_DIRS") {
        for dir in data_dirs.split(':') {
            let dir = dir.trim();
            if !dir.is_empty() {
                let apps_dir = format!("{dir}/applications");
                if !roots.contains(&apps_dir) {
                    roots.push(apps_dir);
                }
            }
        }
    }
    roots
}
