//! The launcher toggle hotkey on the global-shortcut plugin path (Windows and
//! X11). Windows reads `launcher_hotkey` from the config. Linux keeps the
//! default, because on Wayland the compositor owns the binding and both
//! sessions should answer to the same key.

use crate::health;
use look_engine::hotkey::LauncherHotkey;
use std::sync::Mutex;
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

#[cfg(target_os = "windows")]
const CONFLICT_REMEDY: &str = "free it or set another launcher_hotkey, then reload the config";
#[cfg(not(target_os = "windows"))]
const CONFLICT_REMEDY: &str = "free it and restart Look";

static REGISTERED: Mutex<Option<Shortcut>> = Mutex::new(None);

pub fn configured() -> LauncherHotkey {
    #[cfg(target_os = "windows")]
    {
        look_engine::config::RuntimeConfig::load_cached().launcher_hotkey
    }
    #[cfg(not(target_os = "windows"))]
    {
        LauncherHotkey::default()
    }
}

/// Registers the configured hotkey, replacing whatever this module registered
/// before. Failures become health issues instead of aborting: a launcher with
/// a dead hotkey is still reachable by relaunching it.
pub fn register(app: &AppHandle) {
    let launcher = configured();
    let shortcuts = app.global_shortcut();

    let previous = REGISTERED.lock().ok().and_then(|mut slot| slot.take());
    if let Some(previous) = previous {
        let _ = shortcuts.unregister(previous);
    }
    health::clear(health::ISSUE_HOTKEY);

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
    let registered = shortcuts.on_shortcut(shortcut, move |_app, _shortcut, event| {
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

#[tauri::command]
pub fn launcher_hotkey_display() -> String {
    configured().display
}
