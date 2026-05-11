use crate::platform::SettingsCatalogEntry;

pub(crate) static SETTINGS_CATALOG: &[SettingsCatalogEntry] = &[
    // Connectivity
    SettingsCatalogEntry {
        title: "Wi-Fi",
        target: "wifi",
        candidate_id_suffix: "wifi",
        aliases: "network wireless internet connection settings",
    },
    SettingsCatalogEntry {
        title: "Bluetooth",
        target: "bluetooth",
        candidate_id_suffix: "bluetooth",
        aliases: "bluetooth devices pair settings",
    },
    SettingsCatalogEntry {
        title: "Network",
        target: "network",
        candidate_id_suffix: "network",
        aliases: "network ethernet vpn proxy wired connection settings",
    },
    // Devices
    SettingsCatalogEntry {
        title: "Display",
        target: "display",
        candidate_id_suffix: "display",
        aliases: "monitor screen resolution brightness night light scale settings",
    },
    SettingsCatalogEntry {
        title: "Sound",
        target: "sound",
        candidate_id_suffix: "sound",
        aliases: "audio volume speaker microphone output input settings",
    },
    SettingsCatalogEntry {
        title: "Power",
        target: "power",
        candidate_id_suffix: "power",
        aliases: "battery power suspend sleep screen blank automatic settings",
    },
    SettingsCatalogEntry {
        title: "Keyboard",
        target: "keyboard",
        candidate_id_suffix: "keyboard",
        aliases: "keyboard input layout shortcuts keybinding settings",
    },
    SettingsCatalogEntry {
        title: "Mouse & Touchpad",
        target: "mouse",
        candidate_id_suffix: "mouse",
        aliases: "mouse pointer touchpad trackpad scroll speed acceleration settings",
    },
    SettingsCatalogEntry {
        title: "Printers",
        target: "printers",
        candidate_id_suffix: "printers",
        aliases: "printer scanner print devices settings",
    },
    // Personalization
    SettingsCatalogEntry {
        title: "Appearance",
        target: "background",
        candidate_id_suffix: "appearance",
        aliases: "appearance theme dark light style color accent wallpaper background desktop picture settings",
    },
    SettingsCatalogEntry {
        title: "Notifications",
        target: "notifications",
        candidate_id_suffix: "notifications",
        aliases: "notifications alerts do not disturb settings",
    },
    SettingsCatalogEntry {
        title: "Multitasking",
        target: "multitasking",
        candidate_id_suffix: "multitasking",
        aliases: "multitasking workspaces hot corner edge tiling settings",
    },
    // Apps & Online
    SettingsCatalogEntry {
        title: "Applications",
        target: "applications",
        candidate_id_suffix: "applications",
        aliases: "applications default apps file handler settings",
    },
    SettingsCatalogEntry {
        title: "Online Accounts",
        target: "online-accounts",
        candidate_id_suffix: "online-accounts",
        aliases: "online accounts google microsoft email cloud settings",
    },
    SettingsCatalogEntry {
        title: "Search",
        target: "search",
        candidate_id_suffix: "search",
        aliases: "search providers results settings",
    },
    // Privacy & Security
    SettingsCatalogEntry {
        title: "Privacy & Security",
        target: "privacy",
        candidate_id_suffix: "privacy",
        aliases: "privacy security screen lock location camera microphone permissions settings",
    },
    SettingsCatalogEntry {
        title: "Sharing",
        target: "sharing",
        candidate_id_suffix: "sharing",
        aliases: "sharing remote desktop media screen file settings",
    },
    // Accessibility
    SettingsCatalogEntry {
        title: "Accessibility",
        target: "universal-access",
        candidate_id_suffix: "universal-access",
        aliases: "accessibility universal access zoom large text high contrast cursor size settings",
    },
    // System
    SettingsCatalogEntry {
        title: "Users",
        target: "users",
        candidate_id_suffix: "users",
        aliases: "user accounts login password fingerprint settings",
    },
    SettingsCatalogEntry {
        title: "Date & Time",
        target: "datetime",
        candidate_id_suffix: "datetime",
        aliases: "date time timezone clock automatic format settings",
    },
    SettingsCatalogEntry {
        title: "System",
        target: "system",
        candidate_id_suffix: "system",
        aliases: "system about region language software updates remote desktop settings",
    },
    SettingsCatalogEntry {
        title: "Color",
        target: "color",
        candidate_id_suffix: "color",
        aliases: "color profile calibration display monitor settings",
    },
    SettingsCatalogEntry {
        title: "Wellbeing",
        target: "wellbeing",
        candidate_id_suffix: "wellbeing",
        aliases: "wellbeing screen time break reminder digital health settings",
    },
    // Ubuntu-specific
    SettingsCatalogEntry {
        title: "Ubuntu Desktop",
        target: "ubuntu",
        candidate_id_suffix: "ubuntu",
        aliases: "ubuntu desktop dock appearance theme dark light style icons settings",
    },
    // Devices (optional)
    SettingsCatalogEntry {
        title: "Wacom",
        target: "wacom",
        candidate_id_suffix: "wacom",
        aliases: "wacom tablet pen stylus drawing pressure settings",
    },
    SettingsCatalogEntry {
        title: "Mobile Broadband",
        target: "wwan",
        candidate_id_suffix: "wwan",
        aliases: "mobile broadband cellular data sim card wwan settings",
    },
];
