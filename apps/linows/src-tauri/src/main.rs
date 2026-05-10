// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod calc;
mod clipboard;
mod commands;
mod config;
mod files;
#[cfg(target_os = "linux")]
mod linux_gnome_ext;
#[cfg(target_os = "linux")]
mod linux_transparency;
#[cfg(target_os = "linux")]
mod linux_wayland_shortcut;
#[cfg(target_os = "linux")]
mod linux_window_focus;
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
/// Timestamp (ms) of last auto-hide.  When Alt+Space fires and the window is
/// already hidden, we check this to avoid re-showing a window that auto-hide
/// just closed (the GNOME X11 race: Focused(false) fires before the shortcut).
static LAST_AUTO_HIDDEN_AT: AtomicU64 = AtomicU64::new(0);

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
        linux_transparency::has_compositor()
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

/// Toggle the main window: hide if visible, show (centered) if hidden.
fn toggle_window(app_handle: &tauri::AppHandle) {
    let Some(window) = app_handle.get_webview_window("main") else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        #[cfg(target_os = "linux")]
        linux_window_focus::notify_hidden();
        let _ = window.hide();
    } else if now_ms() - LAST_AUTO_HIDDEN_AT.load(Ordering::Relaxed) > 200 {
        // Only show if auto-hide didn't JUST fire.
        // On GNOME X11, Focused(false) races with this handler —
        // auto-hide hides the window before we run, so is_visible
        // is false.  The 200ms guard prevents re-showing.
        LAST_SHOWN_AT.store(now_ms(), Ordering::Relaxed);
        let _ = window.set_always_on_top(true);
        let _ = window.show();
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

        // On Linux/X11, bypass Mutter's focus-stealing prevention
        // by bumping _NET_WM_USER_TIME before activation.
        #[cfg(target_os = "linux")]
        if !linux_transparency::is_wayland() {
            linux_window_focus::activate_self();
            linux_window_focus::notify_shown();
        }

        let _ = window.set_focus();
    }
}

#[cfg(target_os = "linux")]
fn is_wayland() -> bool {
    linux_transparency::is_wayland()
}

fn main() {
    if std::env::args().any(|a| a == "--version" || a == "-V") {
        println!("lookapp {}", env!("APP_VERSION"));
        return;
    }

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // Focus the main window when a second instance is launched
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::new())
        .manage(platform::IconCache::new());

    // On X11 (or non-Linux), register the global shortcut plugin.
    // On Wayland, we use the XDG Desktop Portal instead (set up in .setup()).
    #[cfg(target_os = "linux")]
    let use_wayland = is_wayland();
    #[cfg(not(target_os = "linux"))]
    let use_wayland = false;

    if !use_wayland {
        builder = builder.plugin(tauri_plugin_global_shortcut::Builder::new().build());
    }

    builder
        .setup(move |app| {
            AppState::init_app_handle(app);
            clipboard::start_monitor();
            let app_handle = app.handle().clone();

            if use_wayland {
                // Wayland: register Alt+Space via GNOME custom keybinding + D-Bus
                #[cfg(target_os = "linux")]
                {
                    // Install GNOME Shell extension for window focusing
                    linux_gnome_ext::ensure_installed();

                    let handle = app_handle.clone();
                    linux_wayland_shortcut::start(move || {
                        toggle_window(&handle);
                    });
                }
            } else {
                // X11 / macOS / Windows: use tauri-plugin-global-shortcut
                let handle = app_handle.clone();
                use tauri_plugin_global_shortcut::GlobalShortcutExt;
                app.global_shortcut()
                    .on_shortcut("Alt+Space", move |_app, _shortcut, event| {
                        if event.state
                            != tauri_plugin_global_shortcut::ShortcutState::Pressed
                        {
                            return;
                        }
                        toggle_window(&handle);
                    })?;
            }

            // Cache Look's X11 window ID for later focus activation,
            // and start monitoring _NET_ACTIVE_WINDOW for auto-hide.
            #[cfg(target_os = "linux")]
            if !use_wayland {
                linux_window_focus::cache_self_window();
                let w_monitor = app.get_webview_window("main").expect("main window missing");
                linux_window_focus::start_active_window_monitor(move || {
                    if now_ms() - LAST_SHOWN_AT.load(Ordering::Relaxed) > 300 {
                        LAST_AUTO_HIDDEN_AT.store(now_ms(), Ordering::Relaxed);
                        let _ = w_monitor.hide();
                    }
                });
            }

            // Scale window for current monitor on startup
            let window = app.get_webview_window("main").expect("main window missing");
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
            } else {
                let _ = window
                    .eval("document.documentElement.setAttribute('data-transparent', 'false')");
            }

            // Window event handler
            let w = window.clone();
            window.on_window_event(move |event| {
                match event {
                    tauri::WindowEvent::Focused(true) => {
                        let _ = w.eval("{ let q = document.getElementById('query'); if (q) { q.focus(); q.select(); } }");
                    }
                    // On Linux, Focused(false) fires on mouse-leave (GNOME/Mutter
                    // with undecorated always-on-top windows), so auto-hide is
                    // handled entirely by the X11 active-window monitor instead.
                    // TODO: add Wayland auto-hide when Wayland support is added.
                    #[cfg(not(target_os = "linux"))]
                    tauri::WindowEvent::Focused(false) => {
                        if now_ms() - LAST_SHOWN_AT.load(Ordering::Relaxed) > 300 {
                            LAST_AUTO_HIDDEN_AT.store(now_ms(), Ordering::Relaxed);
                            let _ = w.hide();
                        }
                    }
                    _ => {}
                }
            });

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
            commands::force_index_refresh,
            commands::toggle_window,
            commands::hide_window,
            // Config
            config::get_config,
            config::set_config,
            config::reset_config,
            // Files: meta, version, clipboard, music, folder
            files::get_file_meta,
            files::get_app_version,
            files::copy_files_to_clipboard,
            files::get_home_dir,
            files::list_fonts,
            files::scan_music_folder,
            files::pick_folder,
            files::pick_image,
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
        .build(tauri::generate_context!())
        .expect("error while building look desktop")
        .run(|_app, event| {
            #[cfg(target_os = "linux")]
            if let tauri::RunEvent::Exit = event
                && is_wayland()
            {
                linux_wayland_shortcut::cleanup_gnome_keybinding();
            }
        });
}
