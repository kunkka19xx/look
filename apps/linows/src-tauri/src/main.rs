// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod answers;
mod autostart;
mod calc;
mod clipboard;
mod commands;
mod config;
mod consts;
mod files;
mod highlight;
mod music;
mod platform;
mod process;
mod shell;
mod state;
mod sysinfo;
mod todo;
mod translate;
mod trash;

#[cfg(target_os = "linux")]
use platform::linux::gpu;
use state::AppState;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{Emitter, Manager};

/// Timestamp (ms) of last window show, used to debounce focus-loss auto-hide.
static LAST_SHOWN_AT: AtomicU64 = AtomicU64::new(0);
/// Timestamp (ms) of last auto-hide.  When Alt+Space fires and the window is
/// already hidden, we check this to avoid re-showing a window that auto-hide
/// just closed (the GNOME X11 race: Focused(false) fires before the shortcut).
static LAST_AUTO_HIDDEN_AT: AtomicU64 = AtomicU64::new(0);
/// True while a native file/folder picker dialog is open. The dialog steals
/// focus, and without this guard Focused(false) auto-hide would dismiss Look
/// while the user is still picking.
pub static PICKING_FILE: AtomicBool = AtomicBool::new(false);
/// True once the window received focus after the current show. Auto-hide
/// requires it: a never-focused window has no focus to lose, and without
/// this the first launch from the Windows installer hides on the closing
/// installer's focus churn before the user ever sees it.
static FOCUSED_SINCE_SHOWN: AtomicBool = AtomicBool::new(false);

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
const BASE_H: f64 = 600.0;
/// Grace period (ms) after show - ignore focus-loss within this window.
const AUTO_HIDE_GRACE_MS: u64 = 300;
/// Guard (ms) to prevent re-showing after auto-hide (GNOME X11 race).
const AUTO_HIDE_RESHOW_GUARD_MS: u64 = 200;
const EVENT_WINDOW_SHOWN: &str = "window-shown";

/// Scale window size (logical pixels) to fit the current monitor.
/// Base size targets 1080p (1.0×). Scales up for larger logical screens
/// (1440p → 1.2×, 4K → 1.3× cap).
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
    let w = (BASE_W * ratio).round() as u32;
    let h = (BASE_H * ratio).round() as u32;
    (w, h)
}

/// Toggle the main window: hide if visible, show (centered) if hidden.
fn toggle_window(app_handle: &tauri::AppHandle) {
    let Some(window) = app_handle.get_webview_window(consts::MAIN_WINDOW) else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        #[cfg(target_os = "linux")]
        platform::linux::window_focus::notify_hidden();
        let _ = window.hide();
    } else if now_ms() - LAST_AUTO_HIDDEN_AT.load(Ordering::Relaxed) > AUTO_HIDE_RESHOW_GUARD_MS {
        // Only show if auto-hide didn't JUST fire.
        // On GNOME X11, Focused(false) races with this handler -
        // auto-hide hides the window before we run, so is_visible
        // is false.  The 200ms guard prevents re-showing.
        LAST_SHOWN_AT.store(now_ms(), Ordering::Relaxed);
        FOCUSED_SINCE_SHOWN.store(false, Ordering::Relaxed);

        // Tiling WMs (i3, sway, Hyprland) ignore set_position on unmapped
        // windows - they apply their own placement on map. So we must
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
        let _ = window.emit(EVENT_WINDOW_SHOWN, ());

        // For X11 windows (native X11, or XWayland when the AppImage forces
        // GDK_BACKEND=x11), bypass the compositor's focus-stealing
        // prevention by bumping _NET_WM_USER_TIME before activation.
        #[cfg(target_os = "linux")]
        if platform::linux::transparency::window_is_x11() {
            platform::linux::window_focus::activate_self();
            platform::linux::window_focus::notify_shown();
        }

        let _ = window.set_focus();
    }
}

/// Center and scale a window to fit the current monitor.
/// Called once at startup. Avoid calling on toggle - see toggle_window.
fn center_and_scale_window(window: &tauri::WebviewWindow) {
    let Some(monitor) = monitor_at_cursor(window) else {
        return;
    };
    let pos = monitor.position();
    let screen = monitor.size();
    let scale = monitor.scale_factor();
    let (win_w, win_h) = scaled_window_size(screen.width, screen.height, scale);
    let logical_screen_w = screen.width as f64 / scale;
    let logical_screen_h = screen.height as f64 / scale;
    eprintln!(
        "[look:scale] monitor={}x{} scale={} logical_screen={}x{} → window={}x{}",
        screen.width, screen.height, scale, logical_screen_w, logical_screen_h, win_w, win_h,
    );
    let size = tauri::LogicalSize::new(win_w as f64, win_h as f64);
    let _ = window.set_size(size);
    // Lock min/max to the scaled size: on Wayland, hide()/show() can
    // otherwise revert to tauri.conf's default (860×600) on remap,
    // producing a visible "big rectangle then snap" on toggle.
    let _ = window.set_min_size(Some(tauri::Size::Logical(size)));
    let _ = window.set_max_size(Some(tauri::Size::Logical(size)));
    let lx = pos.x as f64 / scale + (logical_screen_w - win_w as f64) / 2.0;
    let ly = pos.y as f64 / scale + (logical_screen_h - win_h as f64) / 2.0;
    let _ = window.set_position(tauri::LogicalPosition::new(lx, ly));
}

/// Find the monitor that contains the cursor. Falls back to the window's
/// current monitor, then the first available monitor.
fn monitor_at_cursor(window: &tauri::WebviewWindow) -> Option<tauri::Monitor> {
    // Try Tauri's cursor_position first (works on X11).
    // On Wayland, cursor_position() fails - fall back to GNOME Shell D-Bus.
    // GNOME Shell's global.get_pointer() returns *logical* coordinates,
    // while Tauri's monitor positions/sizes are *physical* pixels.
    // We track which space the cursor is in so the hit-test works correctly.
    // On Wayland, Tauri's cursor_position() returns Ok((0,0)) instead of
    // failing - it never reflects the real pointer location. Use the GNOME
    // Shell extension (which calls global.get_pointer()) on Wayland instead.
    // Keyed off the window backend: an XWayland window (AppImage) has a
    // working X11 cursor_position.
    #[cfg(target_os = "linux")]
    let wayland = !platform::linux::transparency::window_is_x11();
    #[cfg(not(target_os = "linux"))]
    let wayland = false;

    let (cursor, cursor_is_logical) = if !wayland {
        match window.cursor_position() {
            Ok(pos) => (Some(pos), false),
            Err(_) => (None, false),
        }
    } else {
        #[cfg(target_os = "linux")]
        {
            let pos = platform::linux::gnome_ext::get_pointer()
                .map(|(x, y)| tauri::PhysicalPosition::new(x as f64, y as f64));
            (pos, true) // GNOME Shell returns logical coords
        }
        #[cfg(not(target_os = "linux"))]
        {
            (None, false)
        }
    };

    if let Some(cursor) = cursor
        && let Ok(monitors) = window.available_monitors()
    {
        let cx = cursor.x;
        let cy = cursor.y;
        for m in &monitors {
            let pos = m.position();
            let size = m.size();
            let scale = m.scale_factor();
            // When cursor is in logical coords (GNOME Shell on Wayland),
            // convert each monitor's physical bounds to logical for comparison.
            let (mx, my, mw, mh) = if cursor_is_logical {
                (
                    pos.x as f64 / scale,
                    pos.y as f64 / scale,
                    size.width as f64 / scale,
                    size.height as f64 / scale,
                )
            } else {
                (
                    pos.x as f64,
                    pos.y as f64,
                    size.width as f64,
                    size.height as f64,
                )
            };
            if cx >= mx && cx < mx + mw && cy >= my && cy < my + mh {
                return Some(m.clone());
            }
        }
    }
    // Fallback: window's current monitor
    window.current_monitor().ok().flatten()
}

/// Re-center the window on the monitor where the cursor is.
/// Used on each toggle so the window follows the user across monitors.
///
/// Note: we recalculate the expected size via `scaled_window_size` instead of
/// querying `outer_size()` because the window is still hidden when this runs,
/// and on some X11 WMs (e.g. i3) a hidden window reports stale/zero sizes,
/// causing the position to drift downward on each toggle.
fn recenter_window(window: &tauri::WebviewWindow) {
    let Some(monitor) = monitor_at_cursor(window) else {
        return;
    };
    let pos = monitor.position();
    let screen = monitor.size();
    let scale = monitor.scale_factor();
    let (win_w, win_h) = scaled_window_size(screen.width, screen.height, scale);
    let logical_screen_w = screen.width as f64 / scale;
    let logical_screen_h = screen.height as f64 / scale;
    // Relax min/max constraints FIRST so the new size isn't clamped to the
    // old monitor's dimensions, then resize, then lock constraints again.
    let size = tauri::LogicalSize::new(win_w as f64, win_h as f64);
    let _ = window.set_min_size(None::<tauri::Size>);
    let _ = window.set_max_size(None::<tauri::Size>);
    let _ = window.set_size(size);
    let _ = window.set_min_size(Some(tauri::Size::Logical(size)));
    let _ = window.set_max_size(Some(tauri::Size::Logical(size)));
    let lx = pos.x as f64 / scale + (logical_screen_w - win_w as f64) / 2.0;
    let ly = pos.y as f64 / scale + (logical_screen_h - win_h as f64) / 2.0;
    let _ = window.set_position(tauri::LogicalPosition::new(lx, ly));
}

#[cfg(target_os = "linux")]
fn is_wayland() -> bool {
    use std::sync::OnceLock;
    static CACHED: OnceLock<bool> = OnceLock::new();
    *CACHED.get_or_init(platform::linux::transparency::is_wayland)
}

/// Set dev-mode config and database paths so dev doesn't pollute production.
/// SAFETY: Must be called at startup before any threads are spawned.
#[cfg(debug_assertions)]
fn setup_dev_env() {
    // Resolve home/data dirs per platform. On Linux cmd shells set HOME; on
    // Windows cmd/PowerShell set USERPROFILE instead - falling back to "."
    // would land dev artifacts inside the repo.
    let home = std::env::var("HOME")
        .ok()
        .or_else(|| std::env::var("USERPROFILE").ok())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| ".".to_string());

    if std::env::var(config::ENV_CONFIG_PATH)
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        unsafe {
            std::env::set_var(
                config::ENV_CONFIG_PATH,
                std::path::PathBuf::from(&home).join(".look.dev.config"),
            );
        }
    }
    if std::env::var(state::ENV_DB_PATH)
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        #[cfg(target_os = "windows")]
        let db_dir = std::env::var("LOCALAPPDATA")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::PathBuf::from(&home)
                    .join("AppData")
                    .join("Local")
            })
            .join("look");

        #[cfg(not(target_os = "windows"))]
        let db_dir = std::env::var("XDG_DATA_HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::path::PathBuf::from(&home).join(".local").join("share"))
            .join("look");

        let _ = std::fs::create_dir_all(&db_dir);
        unsafe {
            std::env::set_var(state::ENV_DB_PATH, db_dir.join("look.dev.db"));
        }
    }
    eprintln!(
        "[dev] config={} db={}",
        std::env::var(config::ENV_CONFIG_PATH).unwrap_or_default(),
        std::env::var(state::ENV_DB_PATH).unwrap_or_default(),
    );
}

/// Sync autostart registration with config on every launch.
///
/// On first launch (no `launch_at_login` key yet) - enable autostart and persist.
/// On subsequent launches - re-sync the registry/desktop-entry with the current
/// exe path so it stays valid after updates or reinstalls (matches WinUI3 behavior).
fn sync_autostart() {
    // Debug builds live under target/debug and (when produced by `tauri dev`)
    // load the frontend from devUrl. If we wrote them into autostart, login
    // would launch a binary that fails with "Could not connect to 127.0.0.1"
    // because the dev server isn't running. Leave the installed binary's
    // autostart entry alone.
    if cfg!(debug_assertions) {
        return;
    }

    const KEY: &str = "launch_at_login";

    let config_path = config::config_file_path();
    let config_content = std::fs::read_to_string(&config_path).unwrap_or_default();

    let config_value = config_content.lines().find_map(|l| {
        let l = l.trim();
        if !l.starts_with('#') {
            l.split_once('=')
                .filter(|(k, _)| k.trim() == KEY)
                .map(|(_, v)| v.trim() == "true")
        } else {
            None
        }
    });

    let enabled = if let Some(val) = config_value {
        val
    } else {
        // First launch - enable by default and persist.
        let _ = config::set_config(vec![config::ConfigUpdate {
            key: KEY.into(),
            value: "true".into(),
        }]);
        true
    };

    let _ = autostart::set_autostart(enabled);
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
    let window = app
        .get_webview_window(consts::MAIN_WINDOW)
        .expect("main window missing");
    platform::linux::window_focus::start_active_window_monitor(move || {
        if PICKING_FILE.load(Ordering::Relaxed) {
            return;
        }
        if now_ms() - LAST_SHOWN_AT.load(Ordering::Relaxed) > AUTO_HIDE_GRACE_MS {
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
    window.on_window_event(move |event| match event {
        tauri::WindowEvent::Focused(true) => {
            FOCUSED_SINCE_SHOWN.store(true, Ordering::Relaxed);
            let _ = w.eval(
                "{ let q = document.getElementById('query'); if (q) { q.focus(); q.select(); } }",
            );
        }
        tauri::WindowEvent::Focused(false)
            if !PICKING_FILE.load(Ordering::Relaxed)
                && FOCUSED_SINCE_SHOWN.load(Ordering::Relaxed)
                && now_ms() - LAST_SHOWN_AT.load(Ordering::Relaxed) > AUTO_HIDE_GRACE_MS
                && focus_loss_means_dismiss() =>
        {
            LAST_AUTO_HIDDEN_AT.store(now_ms(), Ordering::Relaxed);
            let _ = w.hide();
        }
        _ => {}
    });
}

/// Whether a `Focused(false)` event should auto-hide the launcher.
///
/// macOS / Windows: trustworthy, fire only on real focus loss → true.
/// Linux (X11 + Wayland): false. On X11 the GNOME/Mutter mouse-leave race
///   means Focused(false) fires even when the window still has keyboard
///   focus, so the X11 _NET_ACTIVE_WINDOW monitor handles auto-hide
///   instead. On Wayland we also stay false - Focused(false) fires when
///   any screenshot / screencast tool grabs focus, which would dismiss
///   Look before the capture lands. User dismisses via Esc.
fn focus_loss_means_dismiss() -> bool {
    !cfg!(target_os = "linux")
}

fn main() {
    if std::env::args().any(|a| a == "--version" || a == "-V") {
        println!("lookapp {}", env!("APP_VERSION"));
        return;
    }

    #[cfg(debug_assertions)]
    setup_dev_env();

    #[cfg(target_os = "linux")]
    let disable_gpu = gpu::detect_and_disable_virtual_gpu() || gpu::arch_disable_gpu_from_config();

    sync_autostart();

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window(consts::MAIN_WINDOW) {
                LAST_SHOWN_AT.store(now_ms(), Ordering::Relaxed);
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
                gpu::disable_gpu_acceleration(app);
            }

            AppState::init_app_handle(app);
            app.state::<AppState>().start_bootstrap();
            clipboard::start_monitor();

            register_shortcuts(app, use_wayland)?;

            // X11-window concerns (focus monitor, scrolling tweak) follow the
            // window backend: they also apply to the AppImage's XWayland
            // window on a Wayland session.
            #[cfg(target_os = "linux")]
            if platform::linux::transparency::window_is_x11() {
                setup_x11_focus_monitor(app);
                gpu::disable_smooth_scrolling_x11(app);
            }

            let window = app
                .get_webview_window(consts::MAIN_WINDOW)
                .expect("main window missing");
            // Arm the auto-hide grace for the startup show; LAST_SHOWN_AT is
            // otherwise only set by toggle_window, leaving zero grace here.
            LAST_SHOWN_AT.store(now_ms(), Ordering::Relaxed);
            // On transparency-capable Linux compositors, force the GTK window
            // background to transparent. Without this, GTK paints its theme
            // background (opaque, square corners) on the surface before WebKit
            // commits the HTML - visible as a brief "big rectangle without
            // corners" flash before the rounded launcher appears.
            // On X11 bare (no compositor), keep GTK's solid bg as a fallback.
            //
            // Hide the window first so the opaque frame never appears - the
            // race between GTK's first paint and set_background_color causes
            // intermittent sharp-cornered flashes on GNOME.
            #[cfg(target_os = "linux")]
            if supports_transparency() {
                let _ = window.hide();
                let _ = window.set_background_color(Some(tauri::window::Color(0, 0, 0, 0)));
            }
            center_and_scale_window(&window);
            apply_transparency(&window);
            #[cfg(target_os = "linux")]
            if supports_transparency() {
                let _ = window.show();
            }
            #[cfg(target_os = "windows")]
            {
                // WebView2 defaults to an opaque background. With the window
                // marked `transparent: true`, the WebView still paints opaque
                // pixels in the corner triangles. Forcing the default bg to
                // (0,0,0,0) lets the CSS-clipped rounded silhouette show.
                //
                // No DWM corner call here - `DWMWA_WINDOW_CORNER_PREFERENCE`
                // is a verified no-op on `transparent: true` windows
                // (per-pixel-alpha bypasses DWM compositing). Corners come
                // from `border-radius` on `.launcher-window` in `layout.css`.
                let _ = window.set_background_color(Some(tauri::window::Color(0, 0, 0, 0)));
            }
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
            files::list_folder,
            files::is_dev_build,
            files::copy_files_to_clipboard,
            files::get_home_dir,
            files::get_quick_folders,
            files::list_fonts,
            files::scan_music_folder,
            files::pick_folder,
            files::pick_image,
            // Shell
            shell::run_shell_command,
            // Platform: icons, detection, window effects
            platform::get_icon,
            platform::get_platform,
            platform::list_candidate_drives,
            platform::set_window_effect,
            // Commands
            calc::eval_calc,
            sysinfo::get_system_info,
            process::list_processes,
            process::list_processes_on_port,
            process::kill_process,
            process::list_running_apps,
            process::activate_running_app,
            // Todo (shared look-todo store, same table macOS uses)
            todo::todo_list,
            todo::todo_save,
            // Translation
            translate::translate,
            // AI / web answers (look-answers crate, shared with macOS)
            answers::instant_has_match,
            answers::definitional_entity,
            answers::instant_answer,
            answers::duckduckgo_answer,
            answers::wikipedia_answer,
            answers::web_suggestions,
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
            // Trash
            trash::trash_paths,
            trash::count_trash_items,
            trash::empty_trash,
            // Autostart
            autostart::set_autostart,
            autostart::get_autostart,
            // Highlight
            highlight::highlight_file_cmd,
            // About widget: version only. The update check itself runs in
            // the webview via fetch() - no Rust HTTP/TLS dep needed.
            files::get_lookapp_version,
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
