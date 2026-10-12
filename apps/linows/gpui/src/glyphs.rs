//! The glyphs the shell draws itself: Lucide outlines, the same set
//! `src/js/icons.js` inlines for the webview. Served to `svg()` by path.

use std::borrow::Cow;

use base64::Engine;
use gpui::{AssetSource, SharedString};

/// A user's own SVG arrives from the backend inlined; served from here it is
/// drawn like a built-in glyph, in the tile's colour.
pub const INLINE_SVG_PREFIX: &str = "data:image/svg+xml;base64,";

pub const SEARCH: &str = "icons/search.svg";
pub const FILE: &str = "icons/file.svg";
pub const FOLDER: &str = "icons/folder.svg";
pub const APP: &str = "icons/app.svg";
pub const GLOBE: &str = "icons/globe.svg";
pub const CALC: &str = "icons/calculator.svg";

// Launchpad
pub const LIST_CHECKS: &str = "icons/list-checks.svg";
pub const EYE: &str = "icons/eye.svg";
pub const EYE_OFF: &str = "icons/eye-off.svg";
pub const CLOCK: &str = "icons/clock.svg";
pub const BLUETOOTH: &str = "icons/bluetooth.svg";
pub const WIFI: &str = "icons/wifi.svg";
pub const MOON: &str = "icons/moon.svg";
pub const SUN: &str = "icons/sun.svg";
pub const COFFEE: &str = "icons/coffee.svg";
pub const BATTERY: &str = "icons/battery.svg";
pub const BATTERY_CHARGING: &str = "icons/battery-charging.svg";
pub const MONITOR: &str = "icons/monitor.svg";
pub const MIC: &str = "icons/mic.svg";
pub const MIC_OFF: &str = "icons/mic-off.svg";
pub const REFRESH: &str = "icons/refresh-cw.svg";
pub const POWER: &str = "icons/power.svg";
pub const MUSIC: &str = "icons/music.svg";
pub const SKIP_BACK: &str = "icons/skip-back.svg";
pub const SKIP_FORWARD: &str = "icons/skip-forward.svg";
pub const PLAY: &str = "icons/play.svg";
pub const PAUSE: &str = "icons/pause.svg";
pub const CLOUD_SUN: &str = "icons/cloud-sun.svg";
pub const CLOUD: &str = "icons/cloud.svg";
pub const CLOUD_FOG: &str = "icons/cloud-fog.svg";
pub const CLOUD_DRIZZLE: &str = "icons/cloud-drizzle.svg";
pub const CLOUD_RAIN: &str = "icons/cloud-rain.svg";
pub const CLOUD_SNOW: &str = "icons/cloud-snow.svg";
pub const CLOUD_LIGHTNING: &str = "icons/cloud-lightning.svg";
pub const DROPLET: &str = "icons/droplet.svg";

// Rows, previews and panels
pub const CLIPBOARD: &str = "icons/clipboard.svg";
pub const IMAGE: &str = "icons/image.svg";
pub const CPU: &str = "icons/cpu.svg";
pub const TRASH: &str = "icons/trash.svg";
pub const COPY: &str = "icons/copy.svg";
pub const CHECK: &str = "icons/check.svg";
pub const LINK: &str = "icons/link.svg";
pub const EXTERNAL_LINK: &str = "icons/external-link.svg";
pub const HISTORY: &str = "icons/history.svg";
pub const ZAP: &str = "icons/zap.svg";
pub const X_CIRCLE: &str = "icons/x-circle.svg";
pub const LOADER: &str = "icons/loader.svg";
pub const TERMINAL: &str = "icons/terminal.svg";
pub const INFO: &str = "icons/info.svg";
pub const GAUGE: &str = "icons/gauge.svg";
pub const SPARKLES: &str = "icons/sparkles.svg";
pub const ARROW_UP_RIGHT: &str = "icons/arrow-up-right.svg";
pub const SETTINGS: &str = "icons/settings.svg";
pub const TIMER: &str = "icons/timer.svg";
// Todo
pub const BAR_CHART: &str = "icons/bar-chart.svg";
pub const FLAME: &str = "icons/flame.svg";
pub const PLUS: &str = "icons/plus.svg";
pub const SAVE: &str = "icons/save.svg";
pub const CALENDAR_PLUS: &str = "icons/calendar-plus.svg";
pub const CALENDAR: &str = "icons/calendar.svg";
pub const ACTIVITY: &str = "icons/activity.svg";
pub const CHEVRON_RIGHT: &str = "icons/chevron-right.svg";
pub const CHEVRON_DOWN: &str = "icons/chevron-down.svg";

/// A Lucide body in the shared 24-unit frame.
macro_rules! lucide {
    ($path:expr, $body:literal) => {
        (
            $path,
            concat!(
                r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"##,
                $body,
                "</svg>"
            )
            .as_bytes(),
        )
    };
}

const GLYPHS: &[(&str, &[u8])] = &[
    (
        LOADER,
        br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round"><path d="M21 12a9 9 0 1 1-6.219-8.56"/></svg>"##,
    ),
    (
        SEARCH,
        br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="7"/><path d="m20 20-3.8-3.8"/></svg>"##,
    ),
    lucide!(
        FILE,
        r#"<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/><line x1="16" y1="13" x2="8" y2="13"/><line x1="16" y1="17" x2="8" y2="17"/>"#
    ),
    lucide!(
        FOLDER,
        r#"<path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>"#
    ),
    lucide!(
        APP,
        r#"<rect x="2" y="4" width="20" height="16" rx="2"/><path d="M10 4v4"/><path d="M2 8h20"/><path d="M6 4v4"/>"#
    ),
    lucide!(
        GLOBE,
        r#"<circle cx="12" cy="12" r="10"/><line x1="2" y1="12" x2="22" y2="12"/><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/>"#
    ),
    lucide!(
        CALC,
        r#"<rect x="4" y="2" width="16" height="20" rx="2"/><line x1="8" y1="6" x2="16" y2="6"/><line x1="16" y1="14" x2="16" y2="18"/><line x1="8" y1="14" x2="8" y2="14.01"/><line x1="12" y1="14" x2="12" y2="14.01"/><line x1="8" y1="18" x2="8" y2="18.01"/><line x1="12" y1="18" x2="12" y2="18.01"/>"#
    ),
    lucide!(
        BAR_CHART,
        r#"<line x1="12" y1="20" x2="12" y2="10"/><line x1="18" y1="20" x2="18" y2="4"/><line x1="6" y1="20" x2="6" y2="16"/>"#
    ),
    lucide!(
        FLAME,
        r#"<path d="M8.5 14.5A2.5 2.5 0 0 0 11 12c0-1.38-.5-2-1-3-1.072-2.143-.224-4.054 2-6 .5 2.5 2 4.9 4 6.5 2 1.6 3 3.5 3 5.5a7 7 0 1 1-14 0c0-1.153.433-2.294 1-3a2.5 2.5 0 0 0 2.5 2.5z"/>"#
    ),
    lucide!(
        PLUS,
        r#"<line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/>"#
    ),
    lucide!(
        SAVE,
        r#"<path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z"/><polyline points="17 21 17 13 7 13 7 21"/><polyline points="7 3 7 8 15 8"/>"#
    ),
    lucide!(
        CALENDAR_PLUS,
        r#"<rect x="3" y="4" width="18" height="18" rx="2" ry="2"/><line x1="16" y1="2" x2="16" y2="6"/><line x1="8" y1="2" x2="8" y2="6"/><line x1="3" y1="10" x2="21" y2="10"/><line x1="12" y1="14" x2="12" y2="18"/><line x1="10" y1="16" x2="14" y2="16"/>"#
    ),
    lucide!(
        CALENDAR,
        r#"<rect x="3" y="4" width="18" height="18" rx="2" ry="2"/><line x1="16" y1="2" x2="16" y2="6"/><line x1="8" y1="2" x2="8" y2="6"/><line x1="3" y1="10" x2="21" y2="10"/>"#
    ),
    lucide!(
        ACTIVITY,
        r#"<polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/>"#
    ),
    lucide!(
        EYE,
        r#"<path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/><circle cx="12" cy="12" r="3"/>"#
    ),
    lucide!(
        EYE_OFF,
        r#"<path d="M9.88 9.88a3 3 0 1 0 4.24 4.24"/><path d="M10.73 5.08A10.43 10.43 0 0 1 12 5c7 0 11 7 11 7a13.16 13.16 0 0 1-1.67 2.68"/><path d="M6.61 6.61A13.526 13.526 0 0 0 1 12s4 8 11 8a9.74 9.74 0 0 0 5.39-1.61"/><line x1="1" y1="1" x2="23" y2="23"/>"#
    ),
    lucide!(
        LIST_CHECKS,
        r#"<path d="m3 17 2 2 4-4"/><path d="m3 7 2 2 4-4"/><path d="M13 6h8"/><path d="M13 12h8"/><path d="M13 18h8"/>"#
    ),
    lucide!(
        CLOCK,
        r#"<circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/>"#
    ),
    lucide!(
        BLUETOOTH,
        r#"<polyline points="6.5 6.5 17.5 17.5 12 23 12 1 17.5 6.5 6.5 17.5"/>"#
    ),
    lucide!(
        WIFI,
        r#"<path d="M5 12.55a11 11 0 0 1 14.08 0"/><path d="M1.42 9a16 16 0 0 1 21.16 0"/><path d="M8.53 16.11a6 6 0 0 1 6.95 0"/><line x1="12" y1="20" x2="12.01" y2="20"/>"#
    ),
    lucide!(
        MOON,
        r#"<path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"/>"#
    ),
    lucide!(
        SUN,
        r#"<circle cx="12" cy="12" r="5"/><line x1="12" y1="1" x2="12" y2="3"/><line x1="12" y1="21" x2="12" y2="23"/><line x1="4.22" y1="4.22" x2="5.64" y2="5.64"/><line x1="18.36" y1="18.36" x2="19.78" y2="19.78"/><line x1="1" y1="12" x2="3" y2="12"/><line x1="21" y1="12" x2="23" y2="12"/><line x1="4.22" y1="19.78" x2="5.64" y2="18.36"/><line x1="18.36" y1="5.64" x2="19.78" y2="4.22"/>"#
    ),
    lucide!(
        COFFEE,
        r#"<path d="M17 8h1a4 4 0 1 1 0 8h-1"/><path d="M3 8h14v9a4 4 0 0 1-4 4H7a4 4 0 0 1-4-4Z"/><line x1="6" y1="2" x2="6" y2="4"/><line x1="10" y1="2" x2="10" y2="4"/><line x1="14" y1="2" x2="14" y2="4"/>"#
    ),
    lucide!(
        BATTERY,
        r#"<rect x="1" y="6" width="18" height="12" rx="2" ry="2"/><line x1="23" y1="13" x2="23" y2="11"/>"#
    ),
    lucide!(
        BATTERY_CHARGING,
        r#"<rect x="1" y="6" width="18" height="12" rx="2" ry="2"/><line x1="23" y1="13" x2="23" y2="11"/><path transform="translate(-0.5 0)" d="M11 7.5 7.5 13h3l-1 4.5 4-5.5h-3z"/>"#
    ),
    lucide!(
        MONITOR,
        r#"<rect x="2" y="3" width="20" height="14" rx="2" ry="2"/><line x1="8" y1="21" x2="16" y2="21"/><line x1="12" y1="17" x2="12" y2="21"/>"#
    ),
    lucide!(
        MIC,
        r#"<path d="M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z"/><path d="M19 10v2a7 7 0 0 1-14 0v-2"/><line x1="12" y1="19" x2="12" y2="23"/><line x1="8" y1="23" x2="16" y2="23"/>"#
    ),
    lucide!(
        MIC_OFF,
        r#"<line x1="2" y1="2" x2="22" y2="22"/><path d="M18.89 13.23A7.12 7.12 0 0 0 19 12v-2"/><path d="M5 10v2a7 7 0 0 0 12 5"/><path d="M15 9.34V5a3 3 0 0 0-5.68-1.33"/><path d="M9 9v3a3 3 0 0 0 5.12 2.12"/><line x1="12" y1="19" x2="12" y2="23"/><line x1="8" y1="23" x2="16" y2="23"/>"#
    ),
    lucide!(
        REFRESH,
        r#"<polyline points="23 4 23 10 17 10"/><polyline points="1 20 1 14 7 14"/><path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/>"#
    ),
    lucide!(
        POWER,
        r#"<path d="M18.36 6.64a9 9 0 1 1-12.73 0"/><line x1="12" y1="2" x2="12" y2="12"/>"#
    ),
    lucide!(
        MUSIC,
        r#"<path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/>"#
    ),
    lucide!(
        SKIP_BACK,
        r#"<polygon points="19 20 9 12 19 4 19 20"/><line x1="5" y1="19" x2="5" y2="5"/>"#
    ),
    lucide!(
        SKIP_FORWARD,
        r#"<polygon points="5 4 15 12 5 20 5 4"/><line x1="19" y1="5" x2="19" y2="19"/>"#
    ),
    lucide!(PLAY, r#"<polygon points="5 3 19 12 5 21 5 3"/>"#),
    lucide!(
        PAUSE,
        r#"<rect x="6" y="4" width="4" height="16"/><rect x="14" y="4" width="4" height="16"/>"#
    ),
    lucide!(
        CLOUD_SUN,
        r#"<path d="M12 2v2"/><path d="m4.93 4.93 1.41 1.41"/><path d="M20 12h2"/><path d="m19.07 4.93-1.41 1.41"/><path d="M15.947 12.65a4 4 0 0 0-5.925-4.128"/><path d="M13 22H7a5 5 0 1 1 4.9-6H13a3 3 0 0 1 0 6Z"/>"#
    ),
    lucide!(
        CLOUD,
        r#"<path d="M17.5 19H9a7 7 0 1 1 6.71-9h1.79a4.5 4.5 0 1 1 0 9Z"/>"#
    ),
    lucide!(
        CLOUD_FOG,
        r#"<path d="M4 14.899A7 7 0 1 1 15.71 8h1.79a4.5 4.5 0 0 1 2.5 8.242"/><path d="M16 17H7"/><path d="M17 21H9"/>"#
    ),
    lucide!(
        CLOUD_DRIZZLE,
        r#"<path d="M4 14.899A7 7 0 1 1 15.71 8h1.79a4.5 4.5 0 0 1 2.5 8.242"/><path d="M8 19v1"/><path d="M8 14v1"/><path d="M16 19v1"/><path d="M16 14v1"/><path d="M12 21v1"/><path d="M12 16v1"/>"#
    ),
    lucide!(
        CLOUD_RAIN,
        r#"<path d="M4 14.899A7 7 0 1 1 15.71 8h1.79a4.5 4.5 0 0 1 2.5 8.242"/><path d="M16 14v6"/><path d="M8 14v6"/><path d="M12 16v6"/>"#
    ),
    lucide!(
        CLOUD_SNOW,
        r#"<path d="M4 14.899A7 7 0 1 1 15.71 8h1.79a4.5 4.5 0 0 1 2.5 8.242"/><path d="M8 15h.01"/><path d="M8 19h.01"/><path d="M12 17h.01"/><path d="M12 21h.01"/><path d="M16 15h.01"/><path d="M16 19h.01"/>"#
    ),
    lucide!(
        CLOUD_LIGHTNING,
        r#"<path d="M6 16.326A7 7 0 1 1 15.71 8h1.79a4.5 4.5 0 0 1 .5 8.973"/><path d="m13 12-3 5h4l-3 5"/>"#
    ),
    lucide!(
        DROPLET,
        r#"<path d="M12 22a7 7 0 0 0 7-7c0-2-1-3.9-3-5.5s-3.5-4-4-6.5c-.5 2.5-2 4.9-4 6.5C6 11.1 5 13 5 15a7 7 0 0 0 7 7z"/>"#
    ),
    lucide!(
        CLIPBOARD,
        r#"<path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2"/><rect x="8" y="2" width="8" height="4" rx="1" ry="1"/>"#
    ),
    lucide!(
        IMAGE,
        r#"<rect x="3" y="3" width="18" height="18" rx="2" ry="2"/><circle cx="8.5" cy="8.5" r="1.5"/><polyline points="21 15 16 10 5 21"/>"#
    ),
    lucide!(
        CPU,
        r#"<rect x="4" y="4" width="16" height="16" rx="2" ry="2"/><rect x="9" y="9" width="6" height="6"/><line x1="9" y1="1" x2="9" y2="4"/><line x1="15" y1="1" x2="15" y2="4"/><line x1="9" y1="20" x2="9" y2="23"/><line x1="15" y1="20" x2="15" y2="23"/><line x1="20" y1="9" x2="23" y2="9"/><line x1="20" y1="14" x2="23" y2="14"/><line x1="1" y1="9" x2="4" y2="9"/><line x1="1" y1="14" x2="4" y2="14"/>"#
    ),
    lucide!(
        TRASH,
        r#"<polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/><line x1="10" y1="11" x2="10" y2="17"/><line x1="14" y1="11" x2="14" y2="17"/>"#
    ),
    lucide!(
        COPY,
        r#"<rect x="9" y="9" width="13" height="13" rx="2" ry="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>"#
    ),
    lucide!(CHECK, r#"<polyline points="20 6 9 17 4 12"/>"#),
    lucide!(
        LINK,
        r#"<path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/>"#
    ),
    lucide!(
        EXTERNAL_LINK,
        r#"<path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/>"#
    ),
    lucide!(
        HISTORY,
        r#"<path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/><path d="M12 7v5l4 2"/>"#
    ),
    lucide!(
        ZAP,
        r#"<polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/>"#
    ),
    lucide!(
        X_CIRCLE,
        r#"<circle cx="12" cy="12" r="10"/><line x1="15" y1="9" x2="9" y2="15"/><line x1="9" y1="9" x2="15" y2="15"/>"#
    ),
    lucide!(
        TERMINAL,
        r#"<polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/>"#
    ),
    lucide!(
        INFO,
        r#"<circle cx="12" cy="12" r="10"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12.01" y2="8"/>"#
    ),
    lucide!(
        GAUGE,
        r#"<path d="m12 14 4-4"/><path d="M3.34 19a10 10 0 1 1 17.32 0"/>"#
    ),
    lucide!(
        SPARKLES,
        r#"<path d="M12 3l1.5 4.5L18 9l-4.5 1.5L12 15l-1.5-4.5L6 9l4.5-1.5z"/><path d="M19 14l.8 2.2L22 17l-2.2.8L19 20l-.8-2.2L16 17l2.2-.8z"/><path d="M5 16l.6 1.6L7 18l-1.4.4L5 20l-.6-1.6L3 18l1.4-.4z"/>"#
    ),
    lucide!(
        ARROW_UP_RIGHT,
        r#"<line x1="7" y1="17" x2="17" y2="7"/><polyline points="7 7 17 7 17 17"/>"#
    ),
    lucide!(
        SETTINGS,
        r#"<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"/>"#
    ),
    lucide!(
        TIMER,
        r#"<circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/>"#
    ),
    lucide!(CHEVRON_RIGHT, r#"<polyline points="9 18 15 12 9 6"/>"#),
    lucide!(CHEVRON_DOWN, r#"<polyline points="6 9 12 15 18 9"/>"#),
];

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        if let Some(payload) = path.strip_prefix(INLINE_SVG_PREFIX) {
            let bytes = base64::engine::general_purpose::STANDARD.decode(payload)?;
            return Ok(Some(Cow::Owned(bytes)));
        }
        Ok(GLYPHS
            .iter()
            .find(|(name, _)| *name == path)
            .map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }

    fn list(&self, _path: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_inlined_svg_is_served_decoded() {
        let url = format!(
            "{INLINE_SVG_PREFIX}{}",
            base64::engine::general_purpose::STANDARD.encode(b"<svg/>")
        );
        let served = Assets.load(&url).expect("decodes").expect("served");
        assert_eq!(&*served, b"<svg/>");
        assert!(Assets.load("icons/none.svg").expect("ok").is_none());
    }

    #[test]
    fn every_path_is_served_once() {
        let mut seen = std::collections::HashSet::new();
        for (path, bytes) in GLYPHS {
            assert!(seen.insert(*path), "{path} listed twice");
            assert!(bytes.starts_with(b"<svg"), "{path} is not an svg");
        }
    }
}
