// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod calc;
mod clipboard;
mod commands;
mod config;
mod files;
mod music;
mod platform;
mod process;
mod shell;
mod state;
mod sysinfo;
mod translate;

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

const BASE_W: f64 = 860.0;
const BASE_H: f64 = 580.0;

/// Scale window size for larger monitors. Base at 1080p (1.0x), up to 1.3x max.
/// 1440p → 1.2x, 4K → 1.3x (capped).
fn scaled_window_size(screen_w: u32, screen_h: u32, scale: f64) -> (u32, u32) {
    let logical_h = screen_h as f64 / scale;
    let ratio = if logical_h <= 1080.0 {
        1.0
    } else {
        // Linear from 1.0 at 1080 to 1.2 at 1440, capped at 1.3
        let r = 1.0 + (logical_h - 1080.0) / (1440.0 - 1080.0) * 0.2;
        r.min(1.3)
    };
    let _ = screen_w; // used only for centering
    let w = (BASE_W * ratio * scale).round() as u32;
    let h = (BASE_H * ratio * scale).round() as u32;
    (w, h)
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
            clipboard::start_monitor();
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
                                let (win_w, win_h) = scaled_window_size(screen.width, screen.height, scale);
                                let _ = window.set_size(tauri::PhysicalSize::new(win_w, win_h));
                                let x = ((screen.width as f64 - win_w as f64) / 2.0) as i32;
                                let y = ((screen.height as f64 - win_h as f64) / 2.0) as i32;
                                let _ = window.set_position(PhysicalPosition::new(x, y));
                            }
                            let _ = window.emit("window-shown", ());
                        }
                    }
                })?;

            // Scale window for current monitor on startup
            let window = app.get_webview_window("main").unwrap();
            if let Ok(Some(monitor)) = window.current_monitor() {
                let screen = monitor.size();
                let scale = monitor.scale_factor();
                let (win_w, win_h) = scaled_window_size(screen.width, screen.height, scale);
                let _ = window.set_size(tauri::PhysicalSize::new(win_w, win_h));
                let x = ((screen.width as f64 - win_w as f64) / 2.0) as i32;
                let y = ((screen.height as f64 - win_h as f64) / 2.0) as i32;
                let _ = window.set_position(PhysicalPosition::new(x, y));
            }

            // Detect display capabilities and tell the frontend
            let supports_transparency = supports_transparency();

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
            // Core: search, usage, open, reveal, window
            commands::search,
            commands::record_usage,
            commands::open_path,
            commands::reveal_path,
            commands::reload_config,
            commands::request_index_refresh,
            commands::toggle_window,
            commands::hide_window,
            // Config
            config::get_config,
            config::set_config,
            // Files: meta, version, clipboard, music, folder
            files::get_file_meta,
            files::get_app_version,
            files::copy_files_to_clipboard,
            files::get_home_dir,
            files::list_fonts,
            files::scan_music_folder,
            files::pick_folder,
            // Shell
            shell::run_shell_command,
            // Platform: icons, detection, window effects
            platform::get_icon,
            platform::get_platform,
            platform::set_window_effect,
            // Commands
            calc::eval_calc,
            sysinfo::get_system_info,
            process::list_processes,
            process::list_processes_on_port,
            process::kill_process,
            // Translation
            translate::translate,
            // Clipboard
            clipboard::get_clipboard_history,
            clipboard::delete_clipboard_entry,
            clipboard::copy_to_clipboard,
            // Music
            music::music_play,
            music::music_pause,
            music::music_resume,
            music::music_stop,
            music::music_is_finished,
        ])
        .run(tauri::generate_context!())
        .expect("error while running look desktop");
}
