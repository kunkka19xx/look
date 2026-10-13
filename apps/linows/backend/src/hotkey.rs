//! The launcher hotkey as configured, and what the settings recorder asks
//! about it. Binding the key is the shell's job: each one has its own route
//! (a Tauri plugin, a compositor keybinding, a gpui key handler).

use crate::health;
use look_engine::config::RuntimeConfig;
use look_engine::hotkey::{HotkeyCheck, LauncherHotkey};
use serde::Serialize;

/// Only Windows rebinds the key; Linux honours `launcher_hotkey=none` alone.
pub const CONFIGURABLE: bool = cfg!(target_os = "windows");
pub const CONFLICT_REMEDY: &str = if CONFIGURABLE {
    "free it or set another launcher_hotkey, then reload the config"
} else {
    "free it and restart Look"
};

pub fn configured() -> LauncherHotkey {
    RuntimeConfig::load_cached().launcher_hotkey
}

#[derive(Serialize)]
pub struct LauncherHotkeyState {
    pub display: String,
    pub default_spec: String,
    pub default_display: Option<String>,
    pub configurable: bool,
}

pub fn launcher_hotkey_state() -> LauncherHotkeyState {
    let launcher = configured();
    LauncherHotkeyState {
        display: launcher.display,
        default_display: HotkeyCheck::new(&launcher.default_spec).display,
        default_spec: launcher.default_spec,
        configurable: CONFIGURABLE,
    }
}

pub fn hotkey_check(spec: &str) -> HotkeyCheck {
    HotkeyCheck::new(spec)
}

/// A shell's bind attempt, surfaced as a health issue: a launcher with a dead
/// hotkey is still reachable by relaunching it.
pub fn report_bind(launcher: &LauncherHotkey, result: Result<(), String>) {
    match result {
        Ok(()) => {
            if let Some(warning) = &launcher.warning {
                health::report(health::ISSUE_HOTKEY, warning.clone());
            }
        }
        // Carries the config warning too: only the first report per id is kept.
        Err(e) => health::report(
            health::ISSUE_HOTKEY,
            format!(
                "{}{} could not be registered ({e}). Another app may hold the key - \
                 {CONFLICT_REMEDY}. Until then, open Look again from the app menu \
                 to show this window.",
                launcher
                    .warning
                    .as_ref()
                    .map(|warning| format!("{warning}. "))
                    .unwrap_or_default(),
                launcher.display
            ),
        ),
    }
}
