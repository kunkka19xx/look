use crate::platform::SettingsCatalogEntry;

pub(crate) const SETTINGS_CATALOG: &[SettingsCatalogEntry] = &[
    SettingsCatalogEntry {
        title: "System",
        target: "about",
        candidate_id_suffix: "windows.about",
        aliases: "settings system about device specifications windows version",
    },
    SettingsCatalogEntry {
        title: "Display",
        target: "display",
        candidate_id_suffix: "windows.display",
        aliases: "settings display monitor scale resolution night light",
    },
    SettingsCatalogEntry {
        title: "Sound",
        target: "sound",
        candidate_id_suffix: "windows.sound",
        aliases: "settings sound audio speakers microphone input output",
    },
    SettingsCatalogEntry {
        title: "Network & Internet",
        target: "network-status",
        candidate_id_suffix: "windows.network",
        aliases: "settings network internet wifi ethernet vpn proxy",
    },
    SettingsCatalogEntry {
        title: "Bluetooth & devices",
        target: "bluetooth",
        candidate_id_suffix: "windows.bluetooth",
        aliases: "settings bluetooth devices pair mouse keyboard",
    },
    SettingsCatalogEntry {
        title: "Apps & features",
        target: "appsfeatures",
        candidate_id_suffix: "windows.appsfeatures",
        aliases: "settings apps features uninstall installed programs",
    },
    SettingsCatalogEntry {
        title: "Default apps",
        target: "defaultapps",
        candidate_id_suffix: "windows.defaultapps",
        aliases: "settings default apps file associations browser email",
    },
    SettingsCatalogEntry {
        title: "Power & battery",
        target: "powersleep",
        candidate_id_suffix: "windows.powersleep",
        aliases: "settings power battery sleep energy saver",
    },
    SettingsCatalogEntry {
        title: "Storage",
        target: "storagesense",
        candidate_id_suffix: "windows.storagesense",
        aliases: "settings storage disk cleanup sense",
    },
    SettingsCatalogEntry {
        title: "Privacy",
        target: "privacy",
        candidate_id_suffix: "windows.privacy",
        aliases: "settings privacy permissions diagnostics",
    },
    SettingsCatalogEntry {
        title: "Windows Update",
        target: "windowsupdate",
        candidate_id_suffix: "windows.windowsupdate",
        aliases: "settings windows update upgrades patches",
    },
    SettingsCatalogEntry {
        title: "Date & time",
        target: "dateandtime",
        candidate_id_suffix: "windows.dateandtime",
        aliases: "settings date time timezone clock",
    },
    SettingsCatalogEntry {
        title: "Sign-in options",
        target: "signinoptions",
        candidate_id_suffix: "windows.signinoptions",
        aliases: "settings sign in options password pin windows hello",
    },
    SettingsCatalogEntry {
        title: "Taskbar",
        target: "taskbar",
        candidate_id_suffix: "windows.taskbar",
        aliases: "settings taskbar start menu icons",
    },
];

#[cfg(test)]
mod tests {
    use super::SETTINGS_CATALOG;
    use std::collections::HashSet;

    #[test]
    fn windows_settings_catalog_is_non_empty_and_unique() {
        assert!(!SETTINGS_CATALOG.is_empty());

        let mut seen_suffixes = HashSet::new();
        let mut seen_targets = HashSet::new();
        for entry in SETTINGS_CATALOG {
            assert!(entry.candidate_id_suffix.starts_with("windows."));
            assert!(
                seen_suffixes.insert(entry.candidate_id_suffix),
                "duplicate suffix: {}",
                entry.candidate_id_suffix
            );
            assert!(
                seen_targets.insert(entry.target),
                "duplicate target: {}",
                entry.target
            );
        }
    }
}
