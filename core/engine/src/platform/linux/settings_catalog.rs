use crate::platform::SettingsCatalogEntry;

pub(crate) static SETTINGS_CATALOG: &[SettingsCatalogEntry] = &[
    SettingsCatalogEntry {
        title: "Wi-Fi",
        target: "wifi",
        candidate_id_suffix: "wifi",
        aliases: "network wireless internet settings",
    },
    SettingsCatalogEntry {
        title: "Bluetooth",
        target: "bluetooth",
        candidate_id_suffix: "bluetooth",
        aliases: "bluetooth devices settings",
    },
    SettingsCatalogEntry {
        title: "Display",
        target: "display",
        candidate_id_suffix: "display",
        aliases: "monitor screen resolution brightness settings",
    },
    SettingsCatalogEntry {
        title: "Sound",
        target: "sound",
        candidate_id_suffix: "sound",
        aliases: "audio volume speaker microphone settings",
    },
    SettingsCatalogEntry {
        title: "Power",
        target: "power",
        candidate_id_suffix: "power",
        aliases: "battery power suspend sleep settings",
    },
    SettingsCatalogEntry {
        title: "Keyboard",
        target: "keyboard",
        candidate_id_suffix: "keyboard",
        aliases: "keyboard input layout shortcuts settings",
    },
    SettingsCatalogEntry {
        title: "Mouse",
        target: "mouse",
        candidate_id_suffix: "mouse",
        aliases: "mouse pointer touchpad settings",
    },
    SettingsCatalogEntry {
        title: "Printers",
        target: "printers",
        candidate_id_suffix: "printers",
        aliases: "printer scanner devices settings",
    },
    SettingsCatalogEntry {
        title: "Users",
        target: "users",
        candidate_id_suffix: "users",
        aliases: "user accounts login password settings",
    },
    SettingsCatalogEntry {
        title: "Date & Time",
        target: "datetime",
        candidate_id_suffix: "datetime",
        aliases: "date time timezone clock settings",
    },
];
