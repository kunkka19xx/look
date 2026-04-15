mod apps;
mod settings_catalog;

use super::SettingsCatalogEntry;

pub(crate) const APP_SCAN_ROOTS: &[&str] = &[
    "C:/ProgramData/Microsoft/Windows/Start Menu/Programs",
    "~/AppData/Roaming/Microsoft/Windows/Start Menu/Programs",
];

pub(crate) const REQUIRED_APP_SCAN_ROOTS: &[&str] = &[];
pub(crate) const APP_FALLBACK_SCAN_ROOTS: &[&str] = &[
    "C:/Program Files",
    "C:/Program Files (x86)",
    "~/AppData/Local/Programs",
];

pub(crate) const FILE_SCAN_ROOT_SUFFIXES: &[&str] = &["Desktop", "Documents", "Downloads"];

pub(crate) const SETTINGS_URL_SCHEME_PREFIX: &str = "ms-settings:";
pub(crate) const SETTINGS_SUBTITLE_PREFIX: &str = "Windows Settings ";

pub(crate) use apps::discover_installed_apps;
pub(crate) use settings_catalog::SETTINGS_CATALOG;

pub(crate) fn additional_app_scan_roots() -> Vec<String> {
    Vec::new()
}

pub(crate) fn user_home_dir() -> Option<String> {
    std::env::var("USERPROFILE")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
}

pub(crate) fn merged_app_scan_roots(
    config_roots: &[String],
    additional_roots: &[String],
    required_roots: &[&str],
) -> Vec<String> {
    use crate::platform::paths::candidate_id_path_component;
    use std::collections::HashSet;

    let mut out =
        Vec::with_capacity(config_roots.len() + additional_roots.len() + required_roots.len());
    let mut seen =
        HashSet::with_capacity(config_roots.len() + additional_roots.len() + required_roots.len());

    for root in config_roots.iter().chain(additional_roots.iter()) {
        let normalized = candidate_id_path_component(root);
        if seen.insert(normalized) {
            out.push(root.clone());
        }
    }

    for root in required_roots {
        let normalized = candidate_id_path_component(root);
        if seen.insert(normalized) {
            out.push((*root).to_string());
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::{merged_app_scan_roots, user_home_dir};
    use std::env;

    #[test]
    fn merged_roots_preserve_order_and_deduplicate() {
        let config = vec![
            "C:/ProgramData/Microsoft/Windows/Start Menu/Programs".to_string(),
            "~/AppData/Roaming/Microsoft/Windows/Start Menu/Programs".to_string(),
        ];
        let additional = vec![
            "C:/ProgramData/Microsoft/Windows/Start Menu/Programs/".to_string(),
            "C:/Users/demo/AppData/Roaming/Microsoft/Windows/Start Menu/Programs".to_string(),
        ];
        let required = vec!["C:/ProgramData/Microsoft/Windows/Start Menu/Programs"];

        let merged = merged_app_scan_roots(&config, &additional, &required);
        assert_eq!(
            merged,
            vec![
                "C:/ProgramData/Microsoft/Windows/Start Menu/Programs".to_string(),
                "~/AppData/Roaming/Microsoft/Windows/Start Menu/Programs".to_string(),
                "C:/Users/demo/AppData/Roaming/Microsoft/Windows/Start Menu/Programs".to_string(),
            ]
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn user_home_dir_prefers_userprofile_over_home() {
        unsafe {
            env::set_var("HOME", "/c/Users/posix-home");
            env::set_var("USERPROFILE", "C:/Users/win-home");
        }

        let resolved = user_home_dir();
        assert_eq!(resolved.as_deref(), Some("C:/Users/win-home"));
    }
}
