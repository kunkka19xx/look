// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod autostart;
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
    #[cfg(target_os = "linux")]
    {
        platform::linux::transparency::has_compositor()
    }

    #[cfg(not(target_os = "linux"))]
    {
        true
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
        platform::linux::window_focus::notify_hidden();
        let _ = window.hide();
    } else if now_ms() - LAST_AUTO_HIDDEN_AT.load(Ordering::Relaxed) > 200 {
        // Only show if auto-hide didn't JUST fire.
        // On GNOME X11, Focused(false) races with this handler —
        // auto-hide hides the window before we run, so is_visible
        // is false.  The 200ms guard prevents re-showing.
        LAST_SHOWN_AT.store(now_ms(), Ordering::Relaxed);

        // Tiling WMs (i3, sway, Hyprland) ignore set_position on unmapped
        // windows — they apply their own placement on map. So we must
        // recenter AFTER show. Desktop environments (GNOME, KDE, …) work
        // best with recenter BEFORE show to avoid a visible jump.
        #[cfg(target_os = "linux")]
        let tiling = platform::linux::wm::is_tiling_wm();
        #[cfg(not(target_os = "linux"))]
        let tiling = false;

        if !tiling {
            recenter_window(&window);
        }
        let _ = window.set_always_on_top(true);
        let _ = window.show();
        if tiling {
            recenter_window(&window);
        }
        let _ = window.emit("window-shown", ());

        // On Linux/X11, bypass Mutter's focus-stealing prevention
        // by bumping _NET_WM_USER_TIME before activation.
        #[cfg(target_os = "linux")]
        if !platform::linux::transparency::is_wayland() {
            platform::linux::window_focus::activate_self();
            platform::linux::window_focus::notify_shown();
        }

        let _ = window.set_focus();
    }
}

/// Center and scale a window to fit the current monitor.
/// Called once at startup. Avoid calling on toggle — see toggle_window.
fn center_and_scale_window(window: &tauri::WebviewWindow) {
    if let Ok(Some(monitor)) = window.current_monitor() {
        let screen = monitor.size();
        let scale = monitor.scale_factor();
        let (win_w, win_h) = scaled_window_size(screen.width, screen.height, scale);
        let size = tauri::PhysicalSize::new(win_w, win_h);
        let _ = window.set_size(size);
        // Lock min/max to the scaled size: on Wayland, hide()/show() can
        // otherwise revert to tauri.conf's default (860×580) on remap,
        // producing a visible "big rectangle then snap" on toggle.
        let _ = window.set_min_size(Some(tauri::Size::Physical(size)));
        let _ = window.set_max_size(Some(tauri::Size::Physical(size)));
        let x = ((screen.width as f64 - win_w as f64) / 2.0) as i32;
        let y = ((screen.height as f64 - win_h as f64) / 2.0) as i32;
        let _ = window.set_position(PhysicalPosition::new(x, y));
    }
}

/// Re-center the window on the current monitor without changing its size.
/// Used on each toggle so the window follows the user across monitors but
/// doesn't trigger a Wayland configure-cycle resize.
///
/// Note: we recalculate the expected size via `scaled_window_size` instead of
/// querying `outer_size()` because the window is still hidden when this runs,
/// and on some X11 WMs (e.g. i3) a hidden window reports stale/zero sizes,
/// causing the position to drift downward on each toggle.
fn recenter_window(window: &tauri::WebviewWindow) {
    let Ok(Some(monitor)) = window.current_monitor() else {
        return;
    };
    let screen = monitor.size();
    let scale = monitor.scale_factor();
    let (win_w, win_h) = scaled_window_size(screen.width, screen.height, scale);
    let x = ((screen.width as f64 - win_w as f64) / 2.0) as i32;
    let y = ((screen.height as f64 - win_h as f64) / 2.0) as i32;
    let _ = window.set_position(PhysicalPosition::new(x, y));
}

#[cfg(target_os = "linux")]
fn is_wayland() -> bool {
    platform::linux::transparency::is_wayland()
}

/// Set dev-mode config and database paths so dev doesn't pollute production.
/// SAFETY: Must be called at startup before any threads are spawned.
#[cfg(debug_assertions)]
fn setup_dev_env() {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    if std::env::var("LOOK_CONFIG_PATH")
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        unsafe {
            std::env::set_var(
                "LOOK_CONFIG_PATH",
                std::path::PathBuf::from(&home).join(".look.dev.config"),
            );
        }
    }
    if std::env::var("LOOK_DB_PATH")
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        let db_dir = std::env::var("XDG_DATA_HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::path::PathBuf::from(&home).join(".local").join("share"))
            .join("look");
        let _ = std::fs::create_dir_all(&db_dir);
        unsafe {
            std::env::set_var("LOOK_DB_PATH", db_dir.join("look.dev.db"));
        }
    }
    eprintln!(
        "[dev] config={} db={}",
        std::env::var("LOOK_CONFIG_PATH").unwrap_or_default(),
        std::env::var("LOOK_DB_PATH").unwrap_or_default(),
    );
}

/// Detect if running inside a VM with a virtual GPU that doesn't support EGL.
/// Returns true if GPU acceleration should be disabled.
/// SAFETY: Sets env vars — must be called before any threads are spawned.
#[cfg(target_os = "linux")]
fn detect_and_disable_virtual_gpu() -> bool {
    let detected = if !std::path::Path::new("/dev/dri").exists() {
        true
    } else {
        // /dev/dri exists but the driver may not support EGL (common in VMs).
        // Check for known virtual GPU drivers via /dev/dri/card* sysfs.
        std::fs::read_dir("/sys/class/drm")
            .map(|entries| {
                entries.filter_map(Result::ok).any(|e| {
                    let driver = e.path().join("device/driver");
                    if let Ok(target) = std::fs::read_link(&driver) {
                        let name = target
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();
                        matches!(
                            name.as_str(),
                            "virtio-pci"
                                | "virtio_gpu"
                                | "qxl"
                                | "bochs-drm"
                                | "vmwgfx"
                                | "vboxvideo"
                                | "cirrus"
                        )
                    } else {
                        false
                    }
                })
            })
            .unwrap_or(false)
    };
    if detected {
        unsafe {
            std::env::set_var("WEBKIT_DISABLE_GPU", "1");
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
    }
    detected
}

/// Read the `arch_disable_gpu` config key. User-opt-in workaround for
/// the WebKitGTK ghost-rendering bug observed on Arch GNOME 50 + webkit
/// 2.52.3 (other stacks with same webkit version, e.g. Ubuntu 26.04, are
/// unaffected, so we don't auto-detect — toggle lives in Advanced settings).
#[cfg(target_os = "linux")]
fn arch_disable_gpu_from_config() -> bool {
    let path = config::config_file_path();
    let Ok(contents) = std::fs::read_to_string(&path) else {
        return false;
    };
    for line in contents.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=')
            && k.trim() == "arch_disable_gpu"
        {
            return v.trim().eq_ignore_ascii_case("true");
        }
    }
    false
}

/// Disable hardware acceleration via WebKitGTK API for VM GPUs.
/// Env vars (WEBKIT_DISABLE_GPU) are ignored by newer WebKitGTK versions,
/// so we set the policy at the API level before the first render.
#[cfg(target_os = "linux")]
fn disable_gpu_acceleration(app: &tauri::App) {
    if let Some(webview) = app.get_webview_window("main") {
        let _ = webview.with_webview(|wv| {
            use webkit2gtk::SettingsExt;
            let inner = wv.inner();
            if let Some(settings) = webkit2gtk::WebViewExt::settings(&inner) {
                settings.set_hardware_acceleration_policy(
                    webkit2gtk::HardwareAccelerationPolicy::Never,
                );
            }
        });
    }
}

/// Enable autostart on first launch (only if user hasn't explicitly configured it).
fn enable_autostart_on_first_launch() {
    let config_path = config::config_file_path();
    let config_content = std::fs::read_to_string(&config_path).unwrap_or_default();
    if !config_content.contains("launch_at_login") {
        let _ = autostart::set_autostart(true);
    }
}

/// Register global shortcuts (Alt+Space to toggle, Alt+Shift+Q to quit).
/// Uses compositor-specific keybinding on Wayland, tauri-plugin on X11/macOS/Windows.
fn register_shortcuts(
    app: &tauri::App,
    use_wayland: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let app_handle = app.handle().clone();

    if use_wayland {
        #[cfg(target_os = "linux")]
        {
            // Install GNOME Shell extension for window focusing (GNOME only)
            if std::env::var("XDG_CURRENT_DESKTOP")
                .unwrap_or_default()
                .split(':')
                .any(|s| s.trim().eq_ignore_ascii_case("GNOME"))
            {
                platform::linux::gnome_ext::ensure_installed();
            }

            let handle = app_handle.clone();
            platform::linux::wayland_shortcut::start(move || {
                toggle_window(&handle);
            });
        }
    } else {
        use tauri_plugin_global_shortcut::GlobalShortcutExt;
        let handle = app_handle.clone();
        app.global_shortcut()
            .on_shortcut("Alt+Space", move |_app, _shortcut, event| {
                if event.state != tauri_plugin_global_shortcut::ShortcutState::Pressed {
                    return;
                }
                toggle_window(&handle);
            })?;
        app.global_shortcut()
            .on_shortcut("Alt+Shift+Q", |app, _shortcut, event| {
                if event.state != tauri_plugin_global_shortcut::ShortcutState::Pressed {
                    return;
                }
                eprintln!("look: quit via Alt+Shift+Q");
                app.exit(0);
            })?;
    }

    Ok(())
}

/// Cache Look's X11 window ID and start monitoring _NET_ACTIVE_WINDOW for auto-hide.
#[cfg(target_os = "linux")]
fn setup_x11_focus_monitor(app: &tauri::App) {
    platform::linux::window_focus::cache_self_window();
    let window = app.get_webview_window("main").expect("main window missing");
    platform::linux::window_focus::start_active_window_monitor(move || {
        if now_ms() - LAST_SHOWN_AT.load(Ordering::Relaxed) > 300 {
            LAST_AUTO_HIDDEN_AT.store(now_ms(), Ordering::Relaxed);
            let _ = window.hide();
        }
    });
}

/// Set the data-transparent attribute so CSS can adapt to compositor capabilities.
fn apply_transparency(window: &tauri::WebviewWindow) {
    let value = if supports_transparency() {
        "true"
    } else {
        "false"
    };
    let _ = window.eval(format!(
        "document.documentElement.setAttribute('data-transparent', '{value}')"
    ));
}

/// Set up window event handlers (focus input on focus, auto-hide on blur).
fn setup_window_events(window: &tauri::WebviewWindow) {
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
            tauri::WindowEvent::Focused(false)
                if now_ms() - LAST_SHOWN_AT.load(Ordering::Relaxed) > 300 =>
            {
                LAST_AUTO_HIDDEN_AT.store(now_ms(), Ordering::Relaxed);
                let _ = w.hide();
            }
            _ => {}
        }
    });
}

fn main() {
    if std::env::args().any(|a| a == "--version" || a == "-V") {
        println!("lookapp {}", env!("APP_VERSION"));
        return;
    }

    #[cfg(debug_assertions)]
    setup_dev_env();

    #[cfg(target_os = "linux")]
    let disable_gpu = detect_and_disable_virtual_gpu() || arch_disable_gpu_from_config();

    enable_autostart_on_first_launch();

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
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
            #[cfg(target_os = "linux")]
            if disable_gpu {
                disable_gpu_acceleration(app);
            }

            AppState::init_app_handle(app);
            app.state::<AppState>().start_bootstrap();
            clipboard::start_monitor();

            register_shortcuts(app, use_wayland)?;

            #[cfg(target_os = "linux")]
            if !use_wayland {
                setup_x11_focus_monitor(app);
            }

            let window = app.get_webview_window("main").expect("main window missing");
            // On transparency-capable Linux compositors, force the GTK window
            // background to transparent. Without this, GTK paints its theme
            // background (opaque, square corners) on the surface before WebKit
            // commits the HTML — visible as a brief "big rectangle without
            // corners" flash before the rounded launcher appears.
            // On X11 bare (no compositor), keep GTK's solid bg as a fallback.
            #[cfg(target_os = "linux")]
            if supports_transparency() {
                let _ = window.set_background_color(Some(tauri::window::Color(0, 0, 0, 0)));
            }
            center_and_scale_window(&window);
            apply_transparency(&window);
            setup_window_events(&window);

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
            commands::quit_app,
            // Config
            config::get_config,
            config::set_config,
            config::reset_config,
            // Files: meta, version, clipboard, music, folder
            files::get_file_meta,
            files::get_app_version,
            files::is_dev_build,
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
            // Autostart
            autostart::set_autostart,
            autostart::get_autostart,
        ])
        .build(tauri::generate_context!())
        .expect("error while building look desktop")
        .run(|_app, _event| {
            #[cfg(target_os = "linux")]
            if let tauri::RunEvent::Exit = _event
                && is_wayland()
            {
                platform::linux::wayland_shortcut::cleanup_keybinding();
            }
        });
}
