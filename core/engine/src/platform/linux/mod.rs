mod apps;
mod settings_catalog;

use std::env;

pub(crate) const APP_SCAN_ROOTS: &[&str] =
    &["/usr/share/applications", "/usr/local/share/applications"];

pub(crate) const FILE_SCAN_ROOT_SUFFIXES: &[&str] = &["Desktop", "Documents", "Downloads"];

pub(crate) const SETTINGS_URL_SCHEME_PREFIX: &str = "settings://";
pub(crate) const SETTINGS_SUBTITLE_PREFIX: &str = "Settings ";

pub(crate) use apps::discover_installed_apps;
pub(crate) use settings_catalog::SETTINGS_CATALOG;

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
