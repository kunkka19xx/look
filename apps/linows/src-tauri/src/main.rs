// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod state;

use state::AppState;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Emitter, Manager, PhysicalPosition};

static WINDOW_VISIBLE: AtomicBool = AtomicBool::new(true);

fn supports_transparency() -> bool {
    #[cfg(not(target_os = "linux"))]
    { return true; }

    #[cfg(target_os = "linux")]
    {
        // Wayland compositors generally support transparency
        if std::env::var("XDG_SESSION_TYPE").map(|v| v == "wayland").unwrap_or(false) {
            return true;
        }
        // X11: only if a compositor is running
        std::process::Command::new("sh")
            .args(["-c", "pgrep -x picom || pgrep -x compton || pgrep -x compiz"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // Focus the main window when a second instance is launched
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(AppState::new())
        .setup(|app| {
            let app_handle = app.handle().clone();

            // Register Alt+Space global hotkey
            use tauri_plugin_global_shortcut::GlobalShortcutExt;
            app.global_shortcut().on_shortcut("Alt+Space", move |_app, _shortcut, _event| {
                if let Some(window) = app_handle.get_webview_window("main") {
                    if WINDOW_VISIBLE.load(Ordering::Relaxed) {
                        let _ = window.hide();
                        WINDOW_VISIBLE.store(false, Ordering::Relaxed);
                    } else {
                        let _ = window.show();
                        let _ = window.set_focus();
                        if let Ok(Some(monitor)) = window.current_monitor() {
                            let screen = monitor.size();
                            let scale = monitor.scale_factor();
                            let win_w = 860.0 * scale;
                            let win_h = 580.0 * scale;
                            let x = ((screen.width as f64 - win_w) / 2.0) as i32;
                            let y = ((screen.height as f64 - win_h) / 2.0) as i32;
                            let _ = window.set_position(PhysicalPosition::new(x, y));
                        }
                        WINDOW_VISIBLE.store(true, Ordering::Relaxed);
                        let _ = window.emit("window-shown", ());
                    }
                }
            })?;

            // Detect display capabilities and tell the frontend
            let supports_transparency = supports_transparency();
            let window = app.get_webview_window("main").unwrap();

            if supports_transparency {
                let _ = window.eval(
                    "document.documentElement.setAttribute('data-transparent', 'true')"
                );
                // Auto-hide on focus loss (works on macOS/Windows/Wayland)
                let w = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::Focused(false) = event {
                        let _ = w.hide();
                    }
                });
            } else {
                let _ = window.eval(
                    "document.documentElement.setAttribute('data-transparent', 'false')"
                );
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::search,
            commands::record_usage,
            commands::open_path,
            commands::reveal_path,
            commands::reload_config,
            commands::request_index_refresh,
            commands::toggle_window,
            commands::hide_window,
        ])
        .run(tauri::generate_context!())
        .expect("error while running look desktop");
}
