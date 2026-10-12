//! How big the launcher is on a given screen. Shared so every shell opens the
//! same panel at the same height.

use std::sync::Mutex;

use crate::config::{self, LauncherLayout};

/// The split layout's size (3:2), and the 1.0x rung of `scaled_window_size`.
pub const BASE_W: f64 = 840.0;
pub const BASE_H: f64 = 560.0;
/// Compact layout base size, tall enough for 6-7 rows. Mirrors macOS
/// `WindowAutoScale.compactBaseWidth/Height`.
pub const COMPACT_W: f64 = 680.0;
pub const COMPACT_H: f64 = 440.0;

/// Ctrl+Shift+C's layout for this run only; `None` follows the config file.
static SESSION_LAYOUT: Mutex<Option<LauncherLayout>> = Mutex::new(None);

pub fn set_session_layout(layout: Option<LauncherLayout>) {
    if let Ok(mut session) = SESSION_LAYOUT.lock() {
        *session = layout;
    }
}

pub fn session_layout() -> Option<LauncherLayout> {
    SESSION_LAYOUT.lock().ok().and_then(|session| *session)
}

pub fn effective_layout() -> LauncherLayout {
    SESSION_LAYOUT
        .lock()
        .ok()
        .and_then(|session| *session)
        .unwrap_or_else(config::launcher_layout)
}

/// Scale window size (logical pixels) to fit the current monitor.
/// Base size targets 1080p (1.0×). Scales up for larger logical screens
/// (1440p → 1.2×, 4K → 1.3× cap). The base follows the effective layout.
pub fn scaled_window_size(screen_h: u32, scale: f64) -> (u32, u32) {
    let ratio = screen_ratio(screen_h, scale);
    let (base_w, base_h) = match effective_layout() {
        LauncherLayout::Split => (BASE_W, BASE_H),
        LauncherLayout::Compact => (COMPACT_W, COMPACT_H),
    };
    let w = (base_w * ratio).round() as u32;
    let h = (base_h * ratio).round() as u32;
    (w, h)
}

pub fn screen_ratio(screen_h: u32, scale: f64) -> f64 {
    let logical_h = screen_h as f64 / scale;
    if logical_h <= 1080.0 {
        1.0
    } else {
        // Linear from 1.0 at 1080 to 1.2 at 1440, capped at 1.3
        let r = 1.0 + (logical_h - 1080.0) / (1440.0 - 1080.0) * 0.2;
        r.min(1.3)
    }
}

/// Logical distance from the monitor's top to where a centred split panel's top
/// sits. Every layout uses it, so the search bar never moves between layouts.
pub fn top_offset(screen_h: u32, scale: f64) -> f64 {
    let split_h = (BASE_H * screen_ratio(screen_h, scale)).round();
    (screen_h as f64 / scale - split_h) / 2.0
}
