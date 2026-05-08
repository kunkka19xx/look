// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod calc;
mod commands;
mod music;
mod platform;
mod process;
mod state;
mod sysinfo;

use state::AppState;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{Emitter, Manager, PhysicalPosition};

/// Timestamp (ms) of last window show, used to debounce focus-loss auto-hide.
static LAST_SHOWN_AT: AtomicU64 = AtomicU64::new(0);

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn supports_transparency() -> bool {
    #[cfg(not(target_os = "linux"))]
    {
        return true;
    }

    #[cfg(target_os = "linux")]
    {
        // Wayland compositors generally support transparency
        if std::env::var("XDG_SESSION_TYPE")
            .map(|v| v == "wayland")
            .unwrap_or(false)
        {
            return true;
        }
        // X11: only if a compositor is running
        std::process::Command::new("sh")
            .args([
                "-c",
                "pgrep -x picom || pgrep -x compton || pgrep -x compiz",
            ])
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
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::new())
        .manage(platform::IconCache::new())
        .setup(|app| {
            let app_handle = app.handle().clone();

            // Register Alt+Space global hotkey
            use tauri_plugin_global_shortcut::GlobalShortcutExt;
            app.global_shortcut()
                .on_shortcut("Alt+Space", move |_app, _shortcut, event| {
                    if event.state != tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        return;
                    }
                    if let Some(window) = app_handle.get_webview_window("main") {
                        if window.is_visible().unwrap_or(false) {
                            let _ = window.hide();
                        } else {
                            LAST_SHOWN_AT.store(now_ms(), Ordering::Relaxed);
                            let _ = window.show();
                            let _ = window.set_focus();
                            // Ensure search input gets focus inside the webview
                            let w = window.clone();
                            std::thread::spawn(move || {
                                std::thread::sleep(std::time::Duration::from_millis(50));
                                let _ = w.set_focus();
                                let _ = w.eval("document.getElementById('query')?.focus()");
                            });
                            if let Ok(Some(monitor)) = window.current_monitor() {
                                let screen = monitor.size();
                                let scale = monitor.scale_factor();
                                let win_w = 860.0 * scale;
                                let win_h = 580.0 * scale;
                                let x = ((screen.width as f64 - win_w) / 2.0) as i32;
                                let y = ((screen.height as f64 - win_h) / 2.0) as i32;
                                let _ = window.set_position(PhysicalPosition::new(x, y));
                            }
                            let _ = window.emit("window-shown", ());
                        }
                    }
                })?;

            // Detect display capabilities and tell the frontend
            let supports_transparency = supports_transparency();
            let window = app.get_webview_window("main").unwrap();

            if supports_transparency {
                let _ = window
                    .eval("document.documentElement.setAttribute('data-transparent', 'true')");
                // Auto-hide on focus loss (works on macOS/Windows/Wayland)
                let w = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::Focused(false) = event {
                        if now_ms() - LAST_SHOWN_AT.load(Ordering::Relaxed) > 300 {
                            let _ = w.hide();
                        }
                    }
                });
            } else {
                let _ = window
                    .eval("document.documentElement.setAttribute('data-transparent', 'false')");
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
            commands::copy_files_to_clipboard,
            commands::get_home_dir,
            commands::run_shell_command,
            commands::hide_window,
            commands::get_file_meta,
            commands::get_app_version,
            calc::eval_calc,
            sysinfo::get_system_info,
            process::list_processes,
            process::list_processes_on_port,
            process::kill_process,
            platform::get_icon,
            commands::scan_music_folder,
            commands::pick_folder,
            music::music_play,
            music::music_pause,
            music::music_resume,
            music::music_stop,
            music::music_is_finished,
        ])
        .run(tauri::generate_context!())
        .expect("error while running look desktop");
}
