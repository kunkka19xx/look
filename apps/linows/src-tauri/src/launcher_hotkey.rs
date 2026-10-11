//! The launcher toggle on the global-shortcut plugin path (Windows and X11).
//! What the key is comes from the backend; binding it is this file.

use linows_backend::health;
use linows_backend::hotkey::{CONFIGURABLE, configured, report_bind};
use linows_backend::look_engine::config::RuntimeConfig;
use std::sync::Mutex;
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

static REGISTERED: Mutex<Option<Shortcut>> = Mutex::new(None);

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
    let registered = registered.map(|shortcut| {
        if let Ok(mut slot) = REGISTERED.lock() {
            *slot = Some(shortcut);
        }
    });
    report_bind(&launcher, registered);
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
