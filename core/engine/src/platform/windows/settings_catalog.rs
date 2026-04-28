use crate::platform::SettingsCatalogEntry;

// Curated catalog of Windows Settings pages reachable via `ms-settings:` URIs.
// Source: https://learn.microsoft.com/en-us/windows/apps/develop/launch/launch-settings
//
// Scope choices:
//   - included: every page commonly surfaced in Settings UI search
//   - skipped: deprecated pages, IME-specific dialects, HoloLens-only pages, kiosk/IT
//     provisioning targets, and pages whose URI requires runtime parameters (e.g.
//     sound-properties?endpointId=...)
//   - duplicate URIs are not allowed (see test below); when MS docs list synonyms we keep
//     the more specific page (e.g. personalization-colors over the colors alias)
pub(crate) const SETTINGS_CATALOG: &[SettingsCatalogEntry] = &[
    // ---------- Accounts ----------
    SettingsCatalogEntry {
        title: "Your info",
        target: "yourinfo",
        candidate_id_suffix: "windows.accounts.yourinfo",
        aliases: "settings account profile picture name your info",
    },
    SettingsCatalogEntry {
        title: "Email & accounts",
        target: "emailandaccounts",
        candidate_id_suffix: "windows.accounts.emailandaccounts",
        aliases: "settings email accounts add account microsoft outlook",
    },
    SettingsCatalogEntry {
        title: "Sign-in options",
        target: "signinoptions",
        candidate_id_suffix: "windows.accounts.signinoptions",
        aliases: "settings sign in options password pin windows hello biometric",
    },
    SettingsCatalogEntry {
        title: "Windows Hello (Face)",
        target: "signinoptions-launchfaceenrollment",
        candidate_id_suffix: "windows.accounts.hello.face",
        aliases: "settings windows hello face recognition camera enrollment",
    },
    SettingsCatalogEntry {
        title: "Windows Hello (Fingerprint)",
        target: "signinoptions-launchfingerprintenrollment",
        candidate_id_suffix: "windows.accounts.hello.fingerprint",
        aliases: "settings windows hello fingerprint enrollment touch id biometric",
    },
    SettingsCatalogEntry {
        title: "Dynamic lock",
        target: "signinoptions-dynamiclock",
        candidate_id_suffix: "windows.accounts.signinoptions.dynamiclock",
        aliases: "settings dynamic lock bluetooth phone presence",
    },
    SettingsCatalogEntry {
        title: "Family & other users",
        target: "otherusers",
        candidate_id_suffix: "windows.accounts.otherusers",
        aliases: "settings family other users add user account child",
    },
    SettingsCatalogEntry {
        title: "Access work or school",
        target: "workplace",
        candidate_id_suffix: "windows.accounts.workplace",
        aliases: "settings work school enterprise account azure ad join",
    },
    SettingsCatalogEntry {
        title: "Sync your settings",
        target: "sync",
        candidate_id_suffix: "windows.accounts.sync",
        aliases: "settings sync windows backup roaming microsoft account",
    },
    // ---------- Apps ----------
    SettingsCatalogEntry {
        title: "Installed apps",
        target: "appsfeatures",
        candidate_id_suffix: "windows.apps.appsfeatures",
        aliases: "settings apps features installed uninstall programs",
    },
    SettingsCatalogEntry {
        title: "Default apps",
        target: "defaultapps",
        candidate_id_suffix: "windows.apps.defaultapps",
        aliases: "settings default apps file associations browser email",
    },
    SettingsCatalogEntry {
        title: "Startup apps",
        target: "startupapps",
        candidate_id_suffix: "windows.apps.startupapps",
        aliases: "settings startup apps boot login auto launch",
    },
    SettingsCatalogEntry {
        title: "Optional features",
        target: "optionalfeatures",
        candidate_id_suffix: "windows.apps.optionalfeatures",
        aliases: "settings optional features windows components capabilities",
    },
    SettingsCatalogEntry {
        title: "Apps for websites",
        target: "appsforwebsites",
        candidate_id_suffix: "windows.apps.appsforwebsites",
        aliases: "settings apps websites uri handlers protocol",
    },
    SettingsCatalogEntry {
        title: "Offline maps",
        target: "maps",
        candidate_id_suffix: "windows.apps.maps",
        aliases: "settings offline maps download regions",
    },
    SettingsCatalogEntry {
        title: "Video playback",
        target: "videoplayback",
        candidate_id_suffix: "windows.apps.videoplayback",
        aliases: "settings video playback hdr battery streaming",
    },
    // ---------- Devices ----------
    SettingsCatalogEntry {
        title: "Bluetooth & devices",
        target: "bluetooth",
        candidate_id_suffix: "windows.devices.bluetooth",
        aliases: "settings bluetooth devices pair mouse keyboard headphones",
    },
    SettingsCatalogEntry {
        title: "Connected devices",
        target: "connecteddevices",
        candidate_id_suffix: "windows.devices.connecteddevices",
        aliases: "settings connected devices peripherals manage",
    },
    SettingsCatalogEntry {
        title: "Printers & scanners",
        target: "printers",
        candidate_id_suffix: "windows.devices.printers",
        aliases: "settings printers scanners print add printer",
    },
    SettingsCatalogEntry {
        title: "Mouse & touchpad",
        target: "mousetouchpad",
        candidate_id_suffix: "windows.devices.mousetouchpad",
        aliases: "settings mouse touchpad pointer scroll speed buttons",
    },
    SettingsCatalogEntry {
        title: "Touchpad",
        target: "devices-touchpad",
        candidate_id_suffix: "windows.devices.touchpad",
        aliases: "settings touchpad gestures sensitivity precision",
    },
    SettingsCatalogEntry {
        title: "Touch",
        target: "devices-touch",
        candidate_id_suffix: "windows.devices.touch",
        aliases: "settings touch screen pointer haptic",
    },
    SettingsCatalogEntry {
        title: "Pen & Windows Ink",
        target: "pen",
        candidate_id_suffix: "windows.devices.pen",
        aliases: "settings pen windows ink stylus surface",
    },
    SettingsCatalogEntry {
        title: "Typing",
        target: "typing",
        candidate_id_suffix: "windows.devices.typing",
        aliases: "settings typing keyboard autocorrect spelling text",
    },
    SettingsCatalogEntry {
        title: "AutoPlay",
        target: "autoplay",
        candidate_id_suffix: "windows.devices.autoplay",
        aliases: "settings autoplay removable drives default",
    },
    SettingsCatalogEntry {
        title: "USB",
        target: "usb",
        candidate_id_suffix: "windows.devices.usb",
        aliases: "settings usb notification connection",
    },
    SettingsCatalogEntry {
        title: "Camera",
        target: "camera",
        candidate_id_suffix: "windows.devices.camera",
        aliases: "settings camera webcam configure default",
    },
    SettingsCatalogEntry {
        title: "Phone Link",
        target: "mobile-devices",
        candidate_id_suffix: "windows.devices.mobile",
        aliases: "settings phone link mobile devices android iphone your",
    },
    // ---------- Network & Internet ----------
    SettingsCatalogEntry {
        title: "Network & Internet",
        target: "network-status",
        candidate_id_suffix: "windows.network.status",
        aliases: "settings network internet status connection wifi ethernet",
    },
    SettingsCatalogEntry {
        title: "Wi-Fi",
        target: "network-wifi",
        candidate_id_suffix: "windows.network.wifi",
        aliases: "settings wifi wireless network ssid",
    },
    SettingsCatalogEntry {
        title: "Manage known networks",
        target: "network-wifisettings",
        candidate_id_suffix: "windows.network.wifisettings",
        aliases: "settings manage known networks wifi saved forget",
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
        aliases: "settings proxy network http",
    },
    SettingsCatalogEntry {
        title: "Mobile hotspot",
        target: "network-mobilehotspot",
        candidate_id_suffix: "windows.network.mobilehotspot",
        aliases: "settings mobile hotspot tethering share internet",
    },
    SettingsCatalogEntry {
        title: "Airplane mode",
        target: "network-airplanemode",
        candidate_id_suffix: "windows.network.airplanemode",
        aliases: "settings airplane mode flight wireless off",
    },
    SettingsCatalogEntry {
        title: "Cellular & SIM",
        target: "network-cellular",
        candidate_id_suffix: "windows.network.cellular",
        aliases: "settings cellular sim mobile data lte 5g esim",
    },
    SettingsCatalogEntry {
        title: "Dial-up",
        target: "network-dialup",
        candidate_id_suffix: "windows.network.dialup",
        aliases: "settings dial up modem network",
    },
    SettingsCatalogEntry {
        title: "Advanced network settings",
        target: "network-advancedsettings",
        candidate_id_suffix: "windows.network.advancedsettings",
        aliases: "settings advanced network adapter reset properties",
    },
    // ---------- Personalization ----------
    SettingsCatalogEntry {
        title: "Personalization",
        target: "personalization",
        candidate_id_suffix: "windows.personalization",
        aliases: "settings personalization wallpaper background theme color",
    },
    SettingsCatalogEntry {
        title: "Background",
        target: "personalization-background",
        candidate_id_suffix: "windows.personalization.background",
        aliases: "settings personalization background wallpaper desktop picture slideshow",
    },
    SettingsCatalogEntry {
        title: "Colors",
        target: "personalization-colors",
        candidate_id_suffix: "windows.personalization.colors",
        aliases: "settings colors accent light dark mode transparency",
    },
    SettingsCatalogEntry {
        title: "Themes",
        target: "themes",
        candidate_id_suffix: "windows.personalization.themes",
        aliases: "settings themes save apply browse store",
    },
    SettingsCatalogEntry {
        title: "Lock screen",
        target: "lockscreen",
        candidate_id_suffix: "windows.personalization.lockscreen",
        aliases: "settings lock screen wallpaper picture spotlight",
    },
    SettingsCatalogEntry {
        title: "Start",
        target: "personalization-start",
        candidate_id_suffix: "windows.personalization.start",
        aliases: "settings start menu pins recent apps tile layout",
    },
    SettingsCatalogEntry {
        title: "Taskbar",
        target: "taskbar",
        candidate_id_suffix: "windows.personalization.taskbar",
        aliases: "settings taskbar align icons system tray search",
    },
    SettingsCatalogEntry {
        title: "Fonts",
        target: "fonts",
        candidate_id_suffix: "windows.personalization.fonts",
        aliases: "settings fonts install download family typeface",
    },
    SettingsCatalogEntry {
        title: "Text input",
        target: "personalization-textinput",
        candidate_id_suffix: "windows.personalization.textinput",
        aliases: "settings text input ime emoji clipboard panel",
    },
    SettingsCatalogEntry {
        title: "Touch keyboard",
        target: "personalization-touchkeyboard",
        candidate_id_suffix: "windows.personalization.touchkeyboard",
        aliases: "settings touch keyboard onscreen size theme",
    },
    SettingsCatalogEntry {
        title: "Dynamic Lighting",
        target: "personalization-lighting",
        candidate_id_suffix: "windows.personalization.lighting",
        aliases: "settings dynamic lighting rgb peripherals effects",
    },
    // ---------- Privacy ----------
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
        aliases: "settings privacy general advertising id tracking",
    },
    SettingsCatalogEntry {
        title: "Account info",
        target: "privacy-accountinfo",
        candidate_id_suffix: "windows.privacy.accountinfo",
        aliases: "settings privacy account info name picture access",
    },
    SettingsCatalogEntry {
        title: "Activity history",
        target: "privacy-activityhistory",
        candidate_id_suffix: "windows.privacy.activityhistory",
        aliases: "settings privacy activity history timeline",
    },
    SettingsCatalogEntry {
        title: "App diagnostics",
        target: "privacy-appdiagnostics",
        candidate_id_suffix: "windows.privacy.appdiagnostics",
        aliases: "settings privacy app diagnostics info access",
    },
    SettingsCatalogEntry {
        title: "Automatic file downloads",
        target: "privacy-automaticfiledownloads",
        candidate_id_suffix: "windows.privacy.automaticfiledownloads",
        aliases: "settings privacy automatic file downloads onedrive",
    },
    SettingsCatalogEntry {
        title: "Background apps",
        target: "privacy-backgroundapps",
        candidate_id_suffix: "windows.privacy.backgroundapps",
        aliases: "settings privacy background apps permissions battery",
    },
    SettingsCatalogEntry {
        title: "Calendar (privacy)",
        target: "privacy-calendar",
        candidate_id_suffix: "windows.privacy.calendar",
        aliases: "settings privacy calendar access permissions",
    },
    SettingsCatalogEntry {
        title: "Call history",
        target: "privacy-callhistory",
        candidate_id_suffix: "windows.privacy.callhistory",
        aliases: "settings privacy call history phone access",
    },
    SettingsCatalogEntry {
        title: "Camera (privacy)",
        target: "privacy-webcam",
        candidate_id_suffix: "windows.privacy.camera",
        aliases: "settings privacy camera webcam access permissions",
    },
    SettingsCatalogEntry {
        title: "Contacts (privacy)",
        target: "privacy-contacts",
        candidate_id_suffix: "windows.privacy.contacts",
        aliases: "settings privacy contacts access permissions",
    },
    SettingsCatalogEntry {
        title: "Documents (privacy)",
        target: "privacy-documents",
        candidate_id_suffix: "windows.privacy.documents",
        aliases: "settings privacy documents access library",
    },
    SettingsCatalogEntry {
        title: "Downloads folder (privacy)",
        target: "privacy-downloadsfolder",
        candidate_id_suffix: "windows.privacy.downloadsfolder",
        aliases: "settings privacy downloads folder access",
    },
    SettingsCatalogEntry {
        title: "Email (privacy)",
        target: "privacy-email",
        candidate_id_suffix: "windows.privacy.email",
        aliases: "settings privacy email access permissions",
    },
    SettingsCatalogEntry {
        title: "Eye tracker",
        target: "privacy-eyetracker",
        candidate_id_suffix: "windows.privacy.eyetracker",
        aliases: "settings privacy eye tracker accessibility",
    },
    SettingsCatalogEntry {
        title: "Feedback & diagnostics",
        target: "privacy-feedback",
        candidate_id_suffix: "windows.privacy.feedback",
        aliases: "settings privacy feedback diagnostics telemetry",
    },
    SettingsCatalogEntry {
        title: "File system access",
        target: "privacy-broadfilesystemaccess",
        candidate_id_suffix: "windows.privacy.broadfilesystemaccess",
        aliases: "settings privacy file system access broad permissions",
    },
    SettingsCatalogEntry {
        title: "Inking & typing personalization",
        target: "privacy-speechtyping",
        candidate_id_suffix: "windows.privacy.speechtyping",
        aliases: "settings privacy inking typing personalization dictionary",
    },
    SettingsCatalogEntry {
        title: "Location",
        target: "privacy-location",
        candidate_id_suffix: "windows.privacy.location",
        aliases: "settings privacy location gps geofence access",
    },
    SettingsCatalogEntry {
        title: "Messaging (privacy)",
        target: "privacy-messaging",
        candidate_id_suffix: "windows.privacy.messaging",
        aliases: "settings privacy messaging sms access permissions",
    },
    SettingsCatalogEntry {
        title: "Microphone",
        target: "privacy-microphone",
        candidate_id_suffix: "windows.privacy.microphone",
        aliases: "settings privacy microphone mic access permissions",
    },
    SettingsCatalogEntry {
        title: "Music library (privacy)",
        target: "privacy-musiclibrary",
        candidate_id_suffix: "windows.privacy.musiclibrary",
        aliases: "settings privacy music library access",
    },
    SettingsCatalogEntry {
        title: "Notifications (privacy)",
        target: "privacy-notifications",
        candidate_id_suffix: "windows.privacy.notifications",
        aliases: "settings privacy notifications access permissions",
    },
    SettingsCatalogEntry {
        title: "Other devices (privacy)",
        target: "privacy-customdevices",
        candidate_id_suffix: "windows.privacy.customdevices",
        aliases: "settings privacy other devices custom access",
    },
    SettingsCatalogEntry {
        title: "Phone calls (privacy)",
        target: "privacy-phonecalls",
        candidate_id_suffix: "windows.privacy.phonecalls",
        aliases: "settings privacy phone calls access",
    },
    SettingsCatalogEntry {
        title: "Pictures (privacy)",
        target: "privacy-pictures",
        candidate_id_suffix: "windows.privacy.pictures",
        aliases: "settings privacy pictures access library",
    },
    SettingsCatalogEntry {
        title: "Radios",
        target: "privacy-radios",
        candidate_id_suffix: "windows.privacy.radios",
        aliases: "settings privacy radios bluetooth wifi control access",
    },
    SettingsCatalogEntry {
        title: "Speech (privacy)",
        target: "privacy-speech",
        candidate_id_suffix: "windows.privacy.speech",
        aliases: "settings privacy speech online recognition cortana",
    },
    SettingsCatalogEntry {
        title: "Tasks (privacy)",
        target: "privacy-tasks",
        candidate_id_suffix: "windows.privacy.tasks",
        aliases: "settings privacy tasks access permissions",
    },
    SettingsCatalogEntry {
        title: "Videos (privacy)",
        target: "privacy-videos",
        candidate_id_suffix: "windows.privacy.videos",
        aliases: "settings privacy videos access library",
    },
    SettingsCatalogEntry {
        title: "Voice activation",
        target: "privacy-voiceactivation",
        candidate_id_suffix: "windows.privacy.voiceactivation",
        aliases: "settings privacy voice activation wake word assistant",
    },
    // ---------- Sound ----------
    SettingsCatalogEntry {
        title: "Sound",
        target: "sound",
        candidate_id_suffix: "windows.sound",
        aliases: "settings sound audio speakers microphone input output",
    },
    SettingsCatalogEntry {
        title: "Sound devices",
        target: "sound-devices",
        candidate_id_suffix: "windows.sound.devices",
        aliases: "settings sound devices manage all enable disable",
    },
    SettingsCatalogEntry {
        title: "Volume mixer",
        target: "apps-volume",
        candidate_id_suffix: "windows.sound.appsvolume",
        aliases: "settings volume mixer apps audio per app",
    },
    // ---------- System ----------
    SettingsCatalogEntry {
        title: "About",
        target: "about",
        candidate_id_suffix: "windows.system.about",
        aliases: "settings system about device specifications windows version edition",
    },
    SettingsCatalogEntry {
        title: "Display",
        target: "display",
        candidate_id_suffix: "windows.system.display",
        aliases: "settings display monitor scale resolution brightness night light hdr",
    },
    SettingsCatalogEntry {
        title: "Advanced display",
        target: "display-advanced",
        candidate_id_suffix: "windows.system.display.advanced",
        aliases: "settings advanced display refresh rate hdr color profile",
    },
    SettingsCatalogEntry {
        title: "Night light",
        target: "nightlight",
        candidate_id_suffix: "windows.system.nightlight",
        aliases: "settings night light blue light schedule color temperature",
    },
    SettingsCatalogEntry {
        title: "Notifications",
        target: "notifications",
        candidate_id_suffix: "windows.system.notifications",
        aliases: "settings notifications alerts focus action center banners",
    },
    SettingsCatalogEntry {
        title: "Focus",
        target: "quiethours",
        candidate_id_suffix: "windows.system.focus",
        aliases: "settings focus assist do not disturb quiet hours notifications",
    },
    SettingsCatalogEntry {
        title: "Power & battery",
        target: "powersleep",
        candidate_id_suffix: "windows.system.powersleep",
        aliases: "settings power battery sleep energy saver lid",
    },
    SettingsCatalogEntry {
        title: "Battery saver",
        target: "batterysaver",
        candidate_id_suffix: "windows.system.batterysaver",
        aliases: "settings battery saver energy power threshold",
    },
    SettingsCatalogEntry {
        title: "Energy recommendations",
        target: "energyrecommendations",
        candidate_id_suffix: "windows.system.energyrecommendations",
        aliases: "settings energy recommendations carbon eco sustainability",
    },
    SettingsCatalogEntry {
        title: "Storage",
        target: "storagesense",
        candidate_id_suffix: "windows.system.storagesense",
        aliases: "settings storage disk cleanup sense free space",
    },
    SettingsCatalogEntry {
        title: "Storage Sense",
        target: "storagepolicies",
        candidate_id_suffix: "windows.system.storagepolicies",
        aliases: "settings storage sense policies cleanup schedule",
    },
    SettingsCatalogEntry {
        title: "Storage recommendations",
        target: "storagerecommendations",
        candidate_id_suffix: "windows.system.storagerecommendations",
        aliases: "settings storage recommendations large files cleanup",
    },
    SettingsCatalogEntry {
        title: "Default save locations",
        target: "savelocations",
        candidate_id_suffix: "windows.system.savelocations",
        aliases: "settings default save locations documents pictures music videos",
    },
    SettingsCatalogEntry {
        title: "Disks & volumes",
        target: "disksandvolumes",
        candidate_id_suffix: "windows.system.disksandvolumes",
        aliases: "settings disks volumes drives partitions storage",
    },
    SettingsCatalogEntry {
        title: "Encryption",
        target: "deviceencryption",
        candidate_id_suffix: "windows.system.deviceencryption",
        aliases: "settings encryption bitlocker device security",
    },
    SettingsCatalogEntry {
        title: "Multitasking",
        target: "multitasking",
        candidate_id_suffix: "windows.system.multitasking",
        aliases: "settings multitasking snap windows virtual desktops alt tab",
    },
    SettingsCatalogEntry {
        title: "Clipboard",
        target: "clipboard",
        candidate_id_suffix: "windows.system.clipboard",
        aliases: "settings clipboard history sync paste",
    },
    SettingsCatalogEntry {
        title: "Remote Desktop",
        target: "remotedesktop",
        candidate_id_suffix: "windows.system.remotedesktop",
        aliases: "settings remote desktop rdp enable connect",
    },
    SettingsCatalogEntry {
        title: "Projecting to this PC",
        target: "project",
        candidate_id_suffix: "windows.system.project",
        aliases: "settings projecting to this pc miracast wireless display",
    },
    SettingsCatalogEntry {
        title: "Shared experiences",
        target: "crossdevice",
        candidate_id_suffix: "windows.system.crossdevice",
        aliases: "settings shared experiences cross device nearby sharing",
    },
    SettingsCatalogEntry {
        title: "Presence sensing",
        target: "presence",
        candidate_id_suffix: "windows.system.presence",
        aliases: "settings presence sensing wake lock human",
    },
    SettingsCatalogEntry {
        title: "Quick Settings (Control Center)",
        target: "controlcenter",
        candidate_id_suffix: "windows.system.controlcenter",
        aliases: "settings quick control center action panel toggles",
    },
    SettingsCatalogEntry {
        title: "Search",
        target: "search",
        candidate_id_suffix: "windows.system.search",
        aliases: "settings search indexing spotlight find",
    },
    SettingsCatalogEntry {
        title: "Search permissions",
        target: "search-permissions",
        candidate_id_suffix: "windows.system.search.permissions",
        aliases: "settings search permissions safe filter safesearch",
    },
    // ---------- Time and language ----------
    SettingsCatalogEntry {
        title: "Date & time",
        target: "dateandtime",
        candidate_id_suffix: "windows.timelanguage.dateandtime",
        aliases: "settings date time timezone clock automatic",
    },
    SettingsCatalogEntry {
        title: "Language & region",
        target: "regionlanguage",
        candidate_id_suffix: "windows.timelanguage.regionlanguage",
        aliases: "settings language region keyboard add display",
    },
    SettingsCatalogEntry {
        title: "Region (formatting)",
        target: "regionformatting",
        candidate_id_suffix: "windows.timelanguage.regionformatting",
        aliases: "settings region format number date currency calendar",
    },
    SettingsCatalogEntry {
        title: "Keyboard languages",
        target: "keyboard",
        candidate_id_suffix: "windows.timelanguage.keyboard",
        aliases: "settings keyboard input languages layout switch",
    },
    SettingsCatalogEntry {
        title: "Speech",
        target: "speech",
        candidate_id_suffix: "windows.timelanguage.speech",
        aliases: "settings speech voice recognition language",
    },
    // ---------- Accessibility (Ease of access) ----------
    SettingsCatalogEntry {
        title: "Accessibility - Display",
        target: "easeofaccess-display",
        candidate_id_suffix: "windows.accessibility.display",
        aliases: "accessibility settings display contrast text size",
    },
    SettingsCatalogEntry {
        title: "Accessibility - Visual effects",
        target: "easeofaccess-visualeffects",
        candidate_id_suffix: "windows.accessibility.visualeffects",
        aliases: "accessibility settings visual effects animations transparency scrollbars",
    },
    SettingsCatalogEntry {
        title: "Accessibility - Mouse pointer & touch",
        target: "easeofaccess-mousepointer",
        candidate_id_suffix: "windows.accessibility.mousepointer",
        aliases: "accessibility settings mouse pointer size color touch indicator",
    },
    SettingsCatalogEntry {
        title: "Accessibility - Text cursor",
        target: "easeofaccess-cursor",
        candidate_id_suffix: "windows.accessibility.cursor",
        aliases: "accessibility settings text cursor indicator thickness",
    },
    SettingsCatalogEntry {
        title: "Accessibility - Magnifier",
        target: "easeofaccess-magnifier",
        candidate_id_suffix: "windows.accessibility.magnifier",
        aliases: "accessibility settings magnifier zoom screen",
    },
    SettingsCatalogEntry {
        title: "Accessibility - Color filters",
        target: "easeofaccess-colorfilter",
        candidate_id_suffix: "windows.accessibility.colorfilter",
        aliases: "accessibility settings color filters colorblind grayscale",
    },
    SettingsCatalogEntry {
        title: "Accessibility - Contrast themes",
        target: "easeofaccess-highcontrast",
        candidate_id_suffix: "windows.accessibility.highcontrast",
        aliases: "accessibility settings high contrast themes",
    },
    SettingsCatalogEntry {
        title: "Accessibility - Narrator",
        target: "easeofaccess-narrator",
        candidate_id_suffix: "windows.accessibility.narrator",
        aliases: "accessibility settings narrator screen reader voice",
    },
    SettingsCatalogEntry {
        title: "Accessibility - Audio",
        target: "easeofaccess-audio",
        candidate_id_suffix: "windows.accessibility.audio",
        aliases: "accessibility settings audio mono flash",
    },
    SettingsCatalogEntry {
        title: "Accessibility - Captions",
        target: "easeofaccess-closedcaptioning",
        candidate_id_suffix: "windows.accessibility.closedcaptioning",
        aliases: "accessibility settings captions closed captioning subtitles live",
    },
    SettingsCatalogEntry {
        title: "Accessibility - Speech",
        target: "easeofaccess-speechrecognition",
        candidate_id_suffix: "windows.accessibility.speechrecognition",
        aliases: "accessibility settings speech recognition voice access",
    },
    SettingsCatalogEntry {
        title: "Accessibility - Keyboard",
        target: "easeofaccess-keyboard",
        candidate_id_suffix: "windows.accessibility.keyboard",
        aliases: "accessibility settings keyboard sticky filter toggle keys",
    },
    SettingsCatalogEntry {
        title: "Accessibility - Mouse",
        target: "easeofaccess-mouse",
        candidate_id_suffix: "windows.accessibility.mouse",
        aliases: "accessibility settings mouse pointer keys numpad",
    },
    SettingsCatalogEntry {
        title: "Accessibility - Eye control",
        target: "easeofaccess-eyecontrol",
        candidate_id_suffix: "windows.accessibility.eyecontrol",
        aliases: "accessibility settings eye control tracker hardware",
    },
    SettingsCatalogEntry {
        title: "Accessibility - Hearing devices",
        target: "easeofaccess-hearingaids",
        candidate_id_suffix: "windows.accessibility.hearingaids",
        aliases: "accessibility settings hearing aids devices bluetooth le",
    },
    // ---------- Family ----------
    SettingsCatalogEntry {
        title: "Family group",
        target: "family-group",
        candidate_id_suffix: "windows.family.group",
        aliases: "settings family group safety screen time microsoft account",
    },
    // ---------- Gaming ----------
    SettingsCatalogEntry {
        title: "Game Bar",
        target: "gaming-gamebar",
        candidate_id_suffix: "windows.gaming.gamebar",
        aliases: "settings gaming game bar overlay xbox shortcut",
    },
    SettingsCatalogEntry {
        title: "Captures",
        target: "gaming-gamedvr",
        candidate_id_suffix: "windows.gaming.gamedvr",
        aliases: "settings gaming captures game dvr clips screenshots record",
    },
    SettingsCatalogEntry {
        title: "Game Mode",
        target: "gaming-gamemode",
        candidate_id_suffix: "windows.gaming.gamemode",
        aliases: "settings gaming game mode performance",
    },
    // ---------- Update & security ----------
    SettingsCatalogEntry {
        title: "Windows Update",
        target: "windowsupdate",
        candidate_id_suffix: "windows.update",
        aliases: "settings windows update upgrades patches install",
    },
    SettingsCatalogEntry {
        title: "Update history",
        target: "windowsupdate-history",
        candidate_id_suffix: "windows.update.history",
        aliases: "settings windows update history view installed",
    },
    SettingsCatalogEntry {
        title: "Update - Active hours",
        target: "windowsupdate-activehours",
        candidate_id_suffix: "windows.update.activehours",
        aliases: "settings windows update active hours restart schedule",
    },
    SettingsCatalogEntry {
        title: "Update - Advanced options",
        target: "windowsupdate-options",
        candidate_id_suffix: "windows.update.options",
        aliases: "settings windows update advanced options metered delivery",
    },
    SettingsCatalogEntry {
        title: "Optional updates",
        target: "windowsupdate-optionalupdates",
        candidate_id_suffix: "windows.update.optionalupdates",
        aliases: "settings windows update optional drivers feature",
    },
    SettingsCatalogEntry {
        title: "Update - Restart options",
        target: "windowsupdate-restartoptions",
        candidate_id_suffix: "windows.update.restartoptions",
        aliases: "settings windows update restart options notify",
    },
    SettingsCatalogEntry {
        title: "Delivery Optimization",
        target: "delivery-optimization",
        candidate_id_suffix: "windows.update.deliveryoptimization",
        aliases: "settings windows update delivery optimization peer bandwidth",
    },
    SettingsCatalogEntry {
        title: "Activation",
        target: "activation",
        candidate_id_suffix: "windows.activation",
        aliases: "settings activation license product key digital",
    },
    SettingsCatalogEntry {
        title: "Recovery",
        target: "recovery",
        candidate_id_suffix: "windows.recovery",
        aliases: "settings recovery reset startup advanced this pc",
    },
    SettingsCatalogEntry {
        title: "Backup",
        target: "backup",
        candidate_id_suffix: "windows.backup",
        aliases: "settings backup windows onedrive sync",
    },
    SettingsCatalogEntry {
        title: "Troubleshoot",
        target: "troubleshoot",
        candidate_id_suffix: "windows.troubleshoot",
        aliases: "settings troubleshoot diagnostics fix problems",
    },
    SettingsCatalogEntry {
        title: "For developers",
        target: "developers",
        candidate_id_suffix: "windows.developers",
        aliases: "settings developers dev mode sideload terminal",
    },
    SettingsCatalogEntry {
        title: "Find My Device",
        target: "findmydevice",
        candidate_id_suffix: "windows.findmydevice",
        aliases: "settings find my device location track lost",
    },
    SettingsCatalogEntry {
        title: "Windows Security",
        target: "windowsdefender",
        candidate_id_suffix: "windows.security.defender",
        aliases: "settings windows security defender antivirus firewall threat",
    },
    SettingsCatalogEntry {
        title: "Windows Insider Program",
        target: "windowsinsider",
        candidate_id_suffix: "windows.insider",
        aliases: "settings windows insider program preview builds dev beta",
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
