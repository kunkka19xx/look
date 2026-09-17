//! The launcher toggle on the global-shortcut plugin path (Windows and X11).
//! Only Windows reads `launcher_hotkey`: on Wayland the compositor owns the
//! binding, and X11 keeps the same key so both Linux sessions agree.

use crate::health;
use look_engine::hotkey::{HotkeyCheck, LauncherHotkey};
use serde::Serialize;
use std::sync::Mutex;
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

const CONFIGURABLE: bool = cfg!(target_os = "windows");

const CONFLICT_REMEDY: &str = if CONFIGURABLE {
    "free it or set another launcher_hotkey, then reload the config"
} else {
    "free it and restart Look"
};

static REGISTERED: Mutex<Option<Shortcut>> = Mutex::new(None);

fn configured() -> LauncherHotkey {
    if CONFIGURABLE {
        look_engine::config::RuntimeConfig::load_cached().launcher_hotkey
    } else {
        LauncherHotkey::default()
    }
}

/// Replaces the registered hotkey with the configured one. Failures become
/// health issues: a launcher with a dead hotkey is still reachable by relaunching.
pub fn register(app: &AppHandle) {
    unregister(app);
    health::clear(health::ISSUE_HOTKEY);
    let launcher = configured();

    if let Some(warning) = launcher.warning {
        health::report_as(health::ISSUE_HOTKEY, health::KIND_HOTKEY_CONFIG, warning);
    }

    let shortcut = match launcher.accelerator.parse::<Shortcut>() {
        Ok(shortcut) => shortcut,
        Err(e) => {
            health::report(
                health::ISSUE_HOTKEY,
                format!(
                    "{} is not a shortcut this system accepts ({e}).",
                    launcher.display
                ),
            );
            return;
        }
    };

    let handle = app.clone();
    let registered = app
        .global_shortcut()
        .on_shortcut(shortcut, move |_app, _shortcut, event| {
            if event.state != ShortcutState::Pressed {
                return;
            }
            crate::toggle_window(&handle);
        });
    match registered {
        Ok(()) => {
            if let Ok(mut slot) = REGISTERED.lock() {
                *slot = Some(shortcut);
            }
        }
        Err(e) => health::report(
            health::ISSUE_HOTKEY,
            format!(
                "{} could not be registered ({e}). Another app may hold the key - \
                 {CONFLICT_REMEDY}. Until then, open Look again from the app menu \
                 to show this window.",
                launcher.display
            ),
        ),
    }
}

fn unregister(app: &AppHandle) {
    let previous = REGISTERED.lock().ok().and_then(|mut slot| slot.take());
    if let Some(previous) = previous {
        let _ = app.global_shortcut().unregister(previous);
    }
}

#[derive(Serialize)]
pub struct LauncherHotkeyState {
    display: String,
    default_spec: String,
    default_display: String,
    configurable: bool,
}

#[tauri::command]
pub fn launcher_hotkey_state() -> LauncherHotkeyState {
    let launcher = configured();
    let default = HotkeyCheck::new(&launcher.default_spec);
    LauncherHotkeyState {
        display: launcher.display,
        default_display: default.display.unwrap_or_default(),
        default_spec: launcher.default_spec,
        configurable: CONFIGURABLE,
    }
}

#[tauri::command]
pub fn hotkey_check(spec: String) -> HotkeyCheck {
    HotkeyCheck::new(&spec)
}

/// Frees the key while the settings recorder listens for it.
#[tauri::command]
pub fn launcher_hotkey_suspend(app: AppHandle) {
    if CONFIGURABLE {
        unregister(&app);
    }
}

/// `set_config` leaves the engine's cached config stale, so drop it first.
#[tauri::command]
pub fn launcher_hotkey_apply(app: AppHandle) {
    if CONFIGURABLE {
        look_engine::config::RuntimeConfig::invalidate_cache();
        register(&app);
    }
}
