//! The launcher toggle on the global-shortcut plugin path (Windows and X11).
//! What the key is comes from the backend; binding it is this file.

use linows_backend::health;
use linows_backend::hotkey::{CONFIGURABLE, CONFLICT_REMEDY, configured};
use linows_backend::look_engine::config::RuntimeConfig;
use std::sync::Mutex;
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

static REGISTERED: Mutex<Option<Shortcut>> = Mutex::new(None);

/// Failures become health issues: a launcher with a dead hotkey is still
/// reachable by relaunching it.
pub fn register(app: &AppHandle) {
    unregister(app);
    health::clear(health::ISSUE_HOTKEY);
    let launcher = configured();
    if !launcher.enabled {
        return;
    }
    let handle = app.clone();
    let registered = launcher
        .accelerator
        .parse::<Shortcut>()
        .map_err(|e| e.to_string())
        .and_then(|shortcut| {
            app.global_shortcut()
                .on_shortcut(shortcut, move |_app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        crate::toggle_window(&handle);
                    }
                })
                .map(|()| shortcut)
                .map_err(|e| e.to_string())
        });
    match registered {
        Ok(shortcut) => {
            if let Ok(mut slot) = REGISTERED.lock() {
                *slot = Some(shortcut);
            }
            if let Some(warning) = launcher.warning {
                health::report(health::ISSUE_HOTKEY, warning);
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
                    .map(|warning| format!("{warning}. "))
                    .unwrap_or_default(),
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

/// Inactive frees the key for the settings recorder. Active re-reads the
/// config, which `set_config` leaves stale in the engine's cache.
pub fn set_active(app: &AppHandle, active: bool) {
    if !CONFIGURABLE {
        return;
    }
    if active {
        RuntimeConfig::invalidate_cache();
        register(app);
    } else {
        unregister(app);
    }
}
