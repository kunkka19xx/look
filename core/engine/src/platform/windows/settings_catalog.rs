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
        title: "Wi-Fi",
        target: "network-wifi",
        candidate_id_suffix: "windows.network.wifi",
        aliases: "settings wifi wireless network ssid",
    },
    SettingsCatalogEntry {
        title: "Ethernet",
        target: "network-ethernet",
        candidate_id_suffix: "windows.network.ethernet",
        aliases: "settings ethernet wired lan network",
    },
    SettingsCatalogEntry {
        title: "VPN",
        target: "network-vpn",
        candidate_id_suffix: "windows.network.vpn",
        aliases: "settings vpn virtual private network",
    },
    SettingsCatalogEntry {
        title: "Proxy",
        target: "network-proxy",
        candidate_id_suffix: "windows.network.proxy",
        aliases: "settings proxy network",
    },
    SettingsCatalogEntry {
        title: "Bluetooth & devices",
        target: "bluetooth",
        candidate_id_suffix: "windows.bluetooth",
        aliases: "settings bluetooth devices pair mouse keyboard",
    },
    SettingsCatalogEntry {
        title: "Printers & scanners",
        target: "printers",
        candidate_id_suffix: "windows.printers",
        aliases: "settings printers scanners print",
    },
    SettingsCatalogEntry {
        title: "Mouse",
        target: "mousetouchpad",
        candidate_id_suffix: "windows.mouse",
        aliases: "settings mouse pointer speed scroll",
    },
    SettingsCatalogEntry {
        title: "Touchpad",
        target: "devices-touchpad",
        candidate_id_suffix: "windows.touchpad",
        aliases: "settings touchpad gestures sensitivity",
    },
    SettingsCatalogEntry {
        title: "Typing",
        target: "typing",
        candidate_id_suffix: "windows.typing",
        aliases: "settings typing keyboard autocorrect",
    },
    SettingsCatalogEntry {
        title: "Apps & features",
        target: "appsfeatures",
        candidate_id_suffix: "windows.appsfeatures",
        aliases: "settings apps features uninstall installed programs",
    },
    SettingsCatalogEntry {
        title: "Startup apps",
        target: "startupapps",
        candidate_id_suffix: "windows.startupapps",
        aliases: "settings startup apps boot login",
    },
    SettingsCatalogEntry {
        title: "Optional features",
        target: "optionalfeatures",
        candidate_id_suffix: "windows.optionalfeatures",
        aliases: "settings optional features windows components",
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
        title: "Multitasking",
        target: "multitasking",
        candidate_id_suffix: "windows.multitasking",
        aliases: "settings multitasking snap windows virtual desktops",
    },
    SettingsCatalogEntry {
        title: "Clipboard",
        target: "clipboard",
        candidate_id_suffix: "windows.clipboard",
        aliases: "settings clipboard history sync",
    },
    SettingsCatalogEntry {
        title: "Notifications",
        target: "notifications",
        candidate_id_suffix: "windows.notifications",
        aliases: "settings notifications alerts focus",
    },
    SettingsCatalogEntry {
        title: "Personalization",
        target: "personalization",
        candidate_id_suffix: "windows.personalization",
        aliases: "settings personalization wallpaper background theme",
    },
    SettingsCatalogEntry {
        title: "Colors",
        target: "colors",
        candidate_id_suffix: "windows.colors",
        aliases: "settings colors accent light dark mode",
    },
    SettingsCatalogEntry {
        title: "Themes",
        target: "themes",
        candidate_id_suffix: "windows.themes",
        aliases: "settings themes personalization",
    },
    SettingsCatalogEntry {
        title: "Lock screen",
        target: "lockscreen",
        candidate_id_suffix: "windows.lockscreen",
        aliases: "settings lock screen personalization",
    },
    SettingsCatalogEntry {
        title: "Start",
        target: "personalization-start",
        candidate_id_suffix: "windows.start",
        aliases: "settings start menu pins recent apps",
    },
    SettingsCatalogEntry {
        title: "Privacy",
        target: "privacy",
        candidate_id_suffix: "windows.privacy",
        aliases: "settings privacy permissions diagnostics",
    },
    SettingsCatalogEntry {
        title: "Privacy - General",
        target: "privacy-general",
        candidate_id_suffix: "windows.privacy.general",
        aliases: "settings privacy general advertising id",
    },
    SettingsCatalogEntry {
        title: "Privacy - Location",
        target: "privacy-location",
        candidate_id_suffix: "windows.privacy.location",
        aliases: "settings privacy location gps",
    },
    SettingsCatalogEntry {
        title: "Privacy - Camera",
        target: "privacy-webcam",
        candidate_id_suffix: "windows.privacy.camera",
        aliases: "settings privacy camera webcam",
    },
    SettingsCatalogEntry {
        title: "Privacy - Microphone",
        target: "privacy-microphone",
        candidate_id_suffix: "windows.privacy.microphone",
        aliases: "settings privacy microphone mic",
    },
    SettingsCatalogEntry {
        title: "Privacy - Background apps",
        target: "privacy-backgroundapps",
        candidate_id_suffix: "windows.privacy.backgroundapps",
        aliases: "settings privacy background apps permissions",
    },
    SettingsCatalogEntry {
        title: "Windows Update",
        target: "windowsupdate",
        candidate_id_suffix: "windows.windowsupdate",
        aliases: "settings windows update upgrades patches",
    },
    SettingsCatalogEntry {
        title: "Activation",
        target: "activation",
        candidate_id_suffix: "windows.activation",
        aliases: "settings activation license product key",
    },
    SettingsCatalogEntry {
        title: "Recovery",
        target: "recovery",
        candidate_id_suffix: "windows.recovery",
        aliases: "settings recovery reset startup",
    },
    SettingsCatalogEntry {
        title: "Backup",
        target: "backup",
        candidate_id_suffix: "windows.backup",
        aliases: "settings backup windows backup",
    },
    SettingsCatalogEntry {
        title: "Troubleshoot",
        target: "troubleshoot",
        candidate_id_suffix: "windows.troubleshoot",
        aliases: "settings troubleshoot diagnostics",
    },
    SettingsCatalogEntry {
        title: "For developers",
        target: "developers",
        candidate_id_suffix: "windows.developers",
        aliases: "settings developers dev mode sideload",
    },
    SettingsCatalogEntry {
        title: "Date & time",
        target: "dateandtime",
        candidate_id_suffix: "windows.dateandtime",
        aliases: "settings date time timezone clock",
    },
    SettingsCatalogEntry {
        title: "Language & region",
        target: "regionlanguage",
        candidate_id_suffix: "windows.regionlanguage",
        aliases: "settings language region keyboard",
    },
    SettingsCatalogEntry {
        title: "Speech",
        target: "speech",
        candidate_id_suffix: "windows.speech",
        aliases: "settings speech voice recognition",
    },
    SettingsCatalogEntry {
        title: "Sign-in options",
        target: "signinoptions",
        candidate_id_suffix: "windows.signinoptions",
        aliases: "settings sign in options password pin windows hello",
    },
    SettingsCatalogEntry {
        title: "Accessibility - Display",
        target: "easeofaccess-display",
        candidate_id_suffix: "windows.accessibility.display",
        aliases: "settings accessibility display contrast magnifier",
    },
    SettingsCatalogEntry {
        title: "Accessibility - Keyboard",
        target: "easeofaccess-keyboard",
        candidate_id_suffix: "windows.accessibility.keyboard",
        aliases: "settings accessibility keyboard sticky keys",
    },
    SettingsCatalogEntry {
        title: "Accessibility - Mouse",
        target: "easeofaccess-mouse",
        candidate_id_suffix: "windows.accessibility.mouse",
        aliases: "settings accessibility mouse pointer",
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
