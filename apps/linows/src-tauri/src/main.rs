// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod answers;
mod autostart;
mod calc;
mod cli_path;
mod clipboard;
mod clipimage;
mod commands;
mod config;
mod consts;
mod crash;
mod files;
mod health;
mod highlight;
mod launcher_hotkey;
mod lunar;
mod music;
mod netspeed;
mod nowplaying;
mod paste;
mod platform;
mod process;
mod qactions;
mod shell;
mod sources;
mod state;
mod sysinfo;
mod todo;
mod tools;
mod translate;
mod trash;
mod weather;
mod weburl;

use look_engine::modes;
#[cfg(target_os = "linux")]
use platform::linux::gpu;
use state::AppState;
use std::sync::Mutex;
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

/// tauri.conf's window size, and the 1.0x rung of `scaled_window_size`.
pub(crate) const BASE_W: f64 = 860.0;
pub(crate) const BASE_H: f64 = 600.0;
/// The compact layout's base size: the results list alone, tall enough for 6-7
/// rows. Mirrors `WindowAutoScale.compactBaseWidth/Height` on macOS.
const COMPACT_W: f64 = 620.0;
const COMPACT_H: f64 = 440.0;
/// Grace period (ms) after show - ignore focus-loss within this window.
const AUTO_HIDE_GRACE_MS: u64 = 300;
/// Guard (ms) to prevent re-showing after auto-hide (GNOME X11 race).
const AUTO_HIDE_RESHOW_GUARD_MS: u64 = 200;
/// Arm the launchpad, then hide the window once the webview has painted that
/// frame - see `commands::hide_armed`.
fn hide_launcher(window: &tauri::WebviewWindow) {
    commands::hide_armed(window);
}

/// Scale window size (logical pixels) to fit the current monitor.
/// Base size targets 1080p (1.0×). Scales up for larger logical screens
/// (1440p → 1.2×, 4K → 1.3× cap).
fn scaled_window_size(
    screen_w: u32,
    screen_h: u32,
    scale: f64,
    layout: config::LauncherLayout,
) -> (u32, u32) {
    let logical_h = screen_h as f64 / scale;
    let ratio = if logical_h <= 1080.0 {
        1.0
    } else {
        // Linear from 1.0 at 1080 to 1.2 at 1440, capped at 1.3
        let r = 1.0 + (logical_h - 1080.0) / (1440.0 - 1080.0) * 0.2;
        r.min(1.3)
    };
    let _ = screen_w; // used only for centering
    let (base_w, base_h) = match layout {
        config::LauncherLayout::Split => (BASE_W, BASE_H),
        config::LauncherLayout::Compact => (COMPACT_W, COMPACT_H),
    };
    let w = (base_w * ratio).round() as u32;
    let h = (base_h * ratio).round() as u32;
    (w, h)
}

/// Toggle the main window: hide if visible, show (centered) if hidden.
fn toggle_window(app_handle: &tauri::AppHandle) {
    let Some(window) = app_handle.get_webview_window(consts::MAIN_WINDOW) else {
        return;
    };
    if commands::launcher_visible(&window) {
        #[cfg(target_os = "linux")]
        platform::linux::window_focus::notify_hidden();
        hide_launcher(&window);
    } else if now_ms() - LAST_AUTO_HIDDEN_AT.load(Ordering::Relaxed) > AUTO_HIDE_RESHOW_GUARD_MS {
        // Only show if auto-hide didn't JUST fire.
        // On GNOME X11, Focused(false) races with this handler -
        // auto-hide hides the window before we run, so is_visible
        // is false.  The 200ms guard prevents re-showing.
        show_window(&window);
    }
}

/// Every summon goes through here. Placement, the X11 focus-stealing bypass and
/// the focus call are one unit: a path that shows the window without them opens
/// off-centre, or on top without the keyboard.
fn show_window(window: &tauri::WebviewWindow) {
    LAST_SHOWN_AT.store(now_ms(), Ordering::Relaxed);
    FOCUSED_SINCE_SHOWN.store(false, Ordering::Relaxed);

    // A layer surface is placed and stacked by the compositor; neither is
    // ours to ask for.
    #[cfg(target_os = "linux")]
    let placed_by_compositor = platform::linux::layer_shell::is_active();
    #[cfg(not(target_os = "linux"))]
    let placed_by_compositor = false;

    // Tiling WMs (i3, sway, Hyprland) ignore set_position on unmapped
    // windows - they apply their own placement on map. So we must
    // recenter AFTER show. Desktop environments (GNOME, KDE, …) work
    // best with recenter BEFORE show to avoid a visible jump.
    #[cfg(target_os = "linux")]
    let tiling = platform::linux::wm::is_tiling_wm();
    #[cfg(not(target_os = "linux"))]
    let tiling = false;

    if !placed_by_compositor {
        if !tiling {
            recenter_window(window);
        }
        let _ = window.set_always_on_top(true);
    }
    commands::show_launcher_before_event(window, || {
        if !placed_by_compositor && tiling {
            recenter_window(window);
        }
    });
    // For X11 windows (native X11, or XWayland when the AppImage forces
    // GDK_BACKEND=x11), bypass the compositor's focus-stealing
    // prevention by bumping _NET_WM_USER_TIME before activation.
    #[cfg(target_os = "linux")]
    if platform::linux::transparency::window_is_x11() {
        platform::linux::window_focus::activate_self();
        platform::linux::window_focus::notify_shown();
    }

    commands::focus_launcher(window);
}

/// Center and scale a window to fit the current monitor.
/// Called once at startup. Avoid calling on toggle - see toggle_window.
///
/// Returns the logical size it settled on. The layer surface needs it from
/// here rather than reading `inner_size` back: a Wayland window the compositor
/// has not configured yet still reports tauri.conf's default.
fn center_and_scale_window(window: &tauri::WebviewWindow) -> Option<(i32, i32)> {
    let monitor = monitor_at_cursor(window)?;
    let pos = monitor.position();
    let screen = monitor.size();
    let scale = monitor.scale_factor();
    let (win_w, win_h) = scaled_window_size(
        screen.width,
        screen.height,
        scale,
        config::launcher_layout(),
    );
    let logical_screen_w = screen.width as f64 / scale;
    let logical_screen_h = screen.height as f64 / scale;
    eprintln!(
        "[look:scale] monitor={}x{} scale={} logical_screen={}x{} → window={}x{}",
        screen.width, screen.height, scale, logical_screen_w, logical_screen_h, win_w, win_h,
    );
    // Lock min/max to the scaled size: on Wayland, hide()/show() can
    // otherwise revert to tauri.conf's default (860×600) on remap,
    // producing a visible "big rectangle then snap" on toggle.
    resize_locked(window, tauri::LogicalSize::new(win_w as f64, win_h as f64));
    let lx = pos.x as f64 / scale + (logical_screen_w - win_w as f64) / 2.0;
    let ly = pos.y as f64 / scale + (logical_screen_h - win_h as f64) / 2.0;
    let _ = window.set_position(tauri::LogicalPosition::new(lx, ly));
    Some((win_w as i32, win_h as i32))
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
    let (win_w, win_h) = scaled_window_size(
        screen.width,
        screen.height,
        scale,
        config::launcher_layout(),
    );
    let logical_screen_w = screen.width as f64 / scale;
    let logical_screen_h = screen.height as f64 / scale;
    resize_locked(window, tauri::LogicalSize::new(win_w as f64, win_h as f64));
    let lx = pos.x as f64 / scale + (logical_screen_w - win_w as f64) / 2.0;
    let ly = pos.y as f64 / scale + (logical_screen_h - win_h as f64) / 2.0;
    let _ = window.set_position(tauri::LogicalPosition::new(lx, ly));
}

/// Relaxes the min/max constraints FIRST so the new size isn't clamped to the
/// old one, then resizes, then locks them to the new size again.
fn resize_locked(window: &tauri::WebviewWindow, size: tauri::LogicalSize<f64>) {
    let _ = window.set_min_size(None::<tauri::Size>);
    let _ = window.set_max_size(None::<tauri::Size>);
    let _ = window.set_size(size);
    let _ = window.set_min_size(Some(tauri::Size::Logical(size)));
    let _ = window.set_max_size(Some(tauri::Size::Logical(size)));
}

/// Resizes the launcher after the `layout` setting changed. A visible window
/// keeps its top edge and horizontal centre, so the search bar stays put. A
/// hidden one takes the new size from `recenter_window` on its next show,
/// except the layer surface, which is sized only here and at startup.
#[tauri::command]
fn apply_layout(window: tauri::WebviewWindow) {
    let Some(monitor) = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| monitor_at_cursor(&window))
    else {
        return;
    };
    let screen = monitor.size();
    let scale = monitor.scale_factor();
    let (win_w, win_h) = scaled_window_size(
        screen.width,
        screen.height,
        scale,
        config::launcher_layout(),
    );

    #[cfg(target_os = "linux")]
    if platform::linux::layer_shell::is_active() {
        platform::linux::layer_shell::resize(win_w as i32, win_h as i32);
        return;
    }

    if !commands::launcher_visible(&window) {
        return;
    }
    let (Ok(position), Ok(old_size)) = (window.outer_position(), window.outer_size()) else {
        return;
    };
    let top = position.y as f64 / scale;
    let center_x = (position.x as f64 + old_size.width as f64 / 2.0) / scale;
    resize_locked(&window, tauri::LogicalSize::new(win_w as f64, win_h as f64));
    let left = center_x - win_w as f64 / 2.0;
    let _ = window.set_position(tauri::LogicalPosition::new(left, top));
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
    // The engine's answer, not a second one: Windows can have $HOME and
    // $USERPROFILE pointing at different directories.
    let home = look_engine::config_path::home().unwrap_or_else(|| ".".to_string());

    if std::env::var(config::ENV_CONFIG_PATH)
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        // `~/.look/config.dev`, never migrated: a dev file is written by hand.
        let dev =
            look_engine::config_path::resolve_home_variant(std::path::Path::new(&home), true).path;
        unsafe {
            std::env::set_var(config::ENV_CONFIG_PATH, dev);
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

/// Sync OS integrations (autostart, PATH) with config on every launch, so the
/// registered exe path stays valid after updates or reinstalls.
///
/// Debug builds live under target/debug and (when produced by `tauri dev`)
/// load the frontend from devUrl. Registering them would launch or shadow the
/// installed binary with one that fails without the dev server, so skip.
fn sync_integrations() {
    if cfg!(debug_assertions) {
        return;
    }

    let content = std::fs::read_to_string(config::config_file_path()).unwrap_or_default();

    const KEY: &str = "launch_at_login";
    let enabled = config_flag(&content, KEY).unwrap_or_else(|| {
        // First launch: enable by default and persist.
        let _ = config::set_config(vec![config::ConfigUpdate {
            key: KEY.into(),
            value: "true".into(),
        }]);
        true
    });
    let _ = autostart::set_autostart(enabled);

    // No default for PATH: an absent key leaves it alone, since the install
    // script may have added the entry already. Off the main thread because the
    // environment broadcast can block for up to a second.
    if let Some(enabled) = config_flag(&content, "add_to_path") {
        std::thread::spawn(move || {
            let _ = cli_path::set_cli_path(enabled);
        });
    }
}

/// Reads a `key=true|false` line straight off the config text. Cheaper than a
/// full parse, and runs before the window opens.
fn config_flag(content: &str, key: &str) -> Option<bool> {
    content.lines().find_map(|line| {
        let line = line.trim();
        if line.starts_with('#') {
            return None;
        }
        line.split_once('=')
            .filter(|(k, _)| k.trim() == key)
            .map(|(_, v)| v.trim() == "true")
    })
}

/// Register global shortcuts (the launcher toggle, Alt+Shift+Q to quit).
/// Uses compositor-specific keybinding on Wayland, tauri-plugin on X11/macOS/Windows.
///
/// Registration failures never abort startup: a launcher with a dead hotkey
/// is still reachable (relaunching it shows the window via single-instance),
/// while one that exits during setup is gone with no message. Failures are
/// reported as health issues so the frontend can explain what happened.
fn register_shortcuts(app: &tauri::App, use_wayland: bool) {
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
            let bind_key = launcher_hotkey::configured().enabled;
            platform::linux::wayland_shortcut::start(bind_key, move || {
                toggle_window(&handle);
            });
        }
    } else {
        use tauri_plugin_global_shortcut::GlobalShortcutExt;
        launcher_hotkey::register(&app_handle);
        if let Err(e) = app
            .global_shortcut()
            .on_shortcut("Alt+Shift+Q", |app, _shortcut, event| {
                if event.state != tauri_plugin_global_shortcut::ShortcutState::Pressed {
                    return;
                }
                eprintln!("look: quit via Alt+Shift+Q");
                app.exit(0);
            })
        {
            // Quit-shortcut loss is minor: quitting stays available in-app.
            eprintln!("look: failed to register Alt+Shift+Q: {e}");
        }
    }
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
            hide_launcher(&window);
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
    // Tauri reports focus for the husk toplevel, so on the layer-shell path
    // GTK's own signals are the only focus events left.
    #[cfg(target_os = "linux")]
    if platform::linux::layer_shell::is_active() {
        let w = window.clone();
        platform::linux::layer_shell::on_focus(move |focused| on_focus_change(&w, focused));
        return;
    }

    let w = window.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::Focused(focused) = event {
            on_focus_change(&w, *focused);
        }
    });
}

/// Shared by both focus sources: Tauri's window event, and GTK's signals once
/// the webview lives on a layer surface.
fn on_focus_change(window: &tauri::WebviewWindow, focused: bool) {
    if focused {
        FOCUSED_SINCE_SHOWN.store(true, Ordering::Relaxed);
        let _ = window.eval(
            "{ let q = document.getElementById('query'); if (q) { q.focus(); q.select(); } }",
        );
        return;
    }
    if PICKING_FILE.load(Ordering::Relaxed)
        || !FOCUSED_SINCE_SHOWN.load(Ordering::Relaxed)
        || now_ms() - LAST_SHOWN_AT.load(Ordering::Relaxed) <= AUTO_HIDE_GRACE_MS
        || !focus_loss_means_dismiss()
    {
        return;
    }
    LAST_AUTO_HIDDEN_AT.store(now_ms(), Ordering::Relaxed);
    hide_launcher(window);
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

/// A launch query, parked for the frontend to pull.
///
/// Parked on every path rather than pushed: cold start cannot push at all (at
/// `setup()` the webview is still booting, so an emitted event has no listener
/// and Tauri does not buffer it), and on a warm launch a pushed event races
/// `window-shown`, whose reset clears the query and whose `select()` leaves it
/// selected for the next keystroke to replace. The frontend pulls after that
/// reset, so the show always runs first.
static PENDING_LAUNCH: Mutex<Option<String>> = Mutex::new(None);

#[tauri::command]
fn take_launch_query() -> Option<String> {
    PENDING_LAUNCH
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take()
}

/// Park before showing: the pull hangs off `window-shown`.
fn park_launch(launch: &modes::Launch) {
    if let modes::Launch::Query { text } = launch {
        *PENDING_LAUNCH
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(text.clone());
    }
}

fn main() {
    crash::install_panic_hook();

    if std::env::args().any(|a| a == "--version" || a == "-V") {
        println!("lookapp {}", env!("APP_VERSION"));
        return;
    }

    // Answered before the app starts, so asking what you can bind never costs
    // a window.
    let launch = modes::parse_args(std::env::args().skip(1));
    match &launch {
        modes::Launch::ListModes => {
            print!("{}", modes::list_text());
            return;
        }
        modes::Launch::UnknownMode(name) => {
            eprintln!("lookapp: unknown mode \"{name}\"\n\n{}", modes::list_text());
            std::process::exit(2);
        }
        modes::Launch::UnavailableMode(name) => {
            eprintln!("lookapp: mode \"{name}\" is not available on this platform");
            std::process::exit(2);
        }
        _ => {}
    }

    #[cfg(debug_assertions)]
    setup_dev_env();

    #[cfg(target_os = "linux")]
    let disable_gpu = gpu::detect_and_disable_virtual_gpu() || gpu::disable_gpu_from_config();

    sync_integrations();

    let single_instance =
        tauri_plugin_single_instance::Builder::<tauri::Wry>::new().callback(|app, args, _cwd| {
            if let Some(window) = app.get_webview_window(consts::MAIN_WINDOW) {
                let launch = modes::parse_args(args.iter().skip(1));
                if launch == modes::Launch::ReloadConfig {
                    let _ = window.emit(consts::EVENT_CONFIG_RELOAD_REQUESTED, ());
                    return;
                }
                if launch == modes::Launch::Toggle {
                    toggle_window(app);
                    return;
                }
                // The second launch's argv, discarded here until now. Parked
                // before the show, which is what the frontend pulls on.
                park_launch(&launch);
                // The hotkey's summon, not a bare show: an explicit
                // `lookapp <mode>` races no auto-hide, so it never toggles.
                show_window(&window);
            }
        });
    // The plugin keys its lock on tauri.conf.json's `identifier`, which dev and
    // release share, so an installed release would swallow a dev build's argv
    // (`setup_dev_env` separates the config and DB but not this). Only the debug
    // name is set: release keeps the plugin default, leaving the identifier the
    // single source of truth. Linux only - Windows derives its mutex from the
    // identifier with no override, so there the release app has to be quit.
    #[cfg(debug_assertions)]
    let single_instance = single_instance.dbus_id("com.look.desktop.dev");

    let mut builder = tauri::Builder::default()
        .plugin(single_instance.build())
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
            // Reaching setup means the single-instance plugin found no running
            // Look to forward to. Headless by contract, so do not start one.
            if launch == modes::Launch::ReloadConfig {
                eprintln!("lookapp: Look is not running, config will load on next launch");
                std::process::exit(0);
            }
            #[cfg(target_os = "linux")]
            if disable_gpu {
                gpu::disable_gpu_acceleration(app);
            }
            #[cfg(target_os = "linux")]
            gpu::trim_memory_features(app);

            AppState::init_app_handle(app);
            app.state::<AppState>().start_bootstrap();
            clipboard::start_monitor();

            // Probes the user's systemd manager, so the first launch of a
            // session does not wait on it.
            #[cfg(target_os = "linux")]
            platform::linux::prime_user_session();

            register_shortcuts(app, use_wayland);

            #[cfg(debug_assertions)]
            health::report_fake_issues_from_env();

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
            let scaled_size = center_and_scale_window(&window);
            #[cfg(target_os = "linux")]
            platform::linux::layer_shell::attach(&window, scaled_size);
            #[cfg(not(target_os = "linux"))]
            let _ = scaled_size;
            apply_transparency(&window);
            // Needs the main thread and a live window: the surface pointer
            // comes off the window handle.
            #[cfg(target_os = "linux")]
            platform::linux::blur::init(&window);
            // Before the first show: on the layer-shell path focus arrives as
            // a GTK signal, and a handler connected afterwards misses the one
            // that would focus the query input.
            setup_window_events(&window);
            #[cfg(target_os = "linux")]
            if supports_transparency() {
                commands::show_launcher(&window);
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

            // Only when asked for: a normal launch keeps whatever startup
            // visibility it has today. A cold `--toggle` has nothing to hide.
            if matches!(launch, modes::Launch::Query { .. } | modes::Launch::Toggle) {
                park_launch(&launch);
                show_window(&window);
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Core: search, usage, open, reveal, window
            commands::search,
            commands::record_usage,
            commands::open_path,
            commands::open_elevated,
            commands::reveal_path,
            commands::reload_config,
            commands::request_index_refresh,
            commands::force_index_refresh,
            commands::toggle_window,
            commands::hide_window,
            take_launch_query,
            apply_layout,
            commands::confirm_hide,
            commands::set_blur_region,
            commands::quit_app,
            // Config
            config::get_config,
            config::set_config,
            launcher_hotkey::launcher_hotkey_state,
            launcher_hotkey::hotkey_check,
            launcher_hotkey::launcher_hotkey_set_active,
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
            // Preferred tools (shared look-tools composition; the native half
            // lives in platform::{linux,windows}::tools)
            tools::tool_actions,
            tools::perform_tool_action,
            // User-declared sources (shared look-engine orchestration over
            // look-sources; see specs/user-sources.md)
            sources::source_block,
            sources::source_blocks,
            sources::perform_block,
            sources::source_rows,
            sources::source_preview,
            sources::refresh_run_blocks,
            // Platform: icons, detection, window effects
            platform::get_icon,
            platform::get_platform,
            platform::list_candidate_drives,
            platform::set_window_effect,
            // Commands
            calc::eval_calc,
            calc::calc_inline,
            sysinfo::get_system_info,
            sysinfo::system_uptime,
            process::list_processes,
            process::kill_process,
            process::search_processes,
            process::search_kill_targets,
            process::process_detail,
            process::process_cpu,
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
            // URL-like queries + opened-URL history (shared core, same
            // url_history table macOS uses)
            weburl::classify_url,
            weburl::record_url_hit,
            weburl::recent_urls,
            // Quick Actions (shared look-qactions catalog; adapters live in
            // qactions/controls, see docs/writing-controls.md)
            qactions::quick_actions,
            qactions::launchpad_layout,
            qactions::launchpad_tile_values,
            qactions::refresh_launchpad_tiles,
            qactions::press_launchpad_tile,
            qactions::launchpad_warnings,
            qactions::quick_action_state,
            qactions::quick_action_apply,
            qactions::quick_action_apply_item,
            // Launchpad external feeds (Phase 3)
            weather::weather_current,
            nowplaying::now_playing_current,
            nowplaying::now_playing_command,
            lunar::lunar_date,
            netspeed::speed_test,
            netspeed::local_ipv4,
            // Clipboard
            clipboard::get_clipboard_history,
            clipboard::delete_clipboard_entry,
            clipboard::get_clipboard_images,
            clipboard::delete_clipboard_image,
            clipboard::clipboard_image_data_url,
            clipboard::copy_clipboard_image,
            clipboard::copy_to_clipboard,
            clipboard::copy_to_clipboard_labeled,
            paste::clipboard_paste_blocker,
            paste::paste_into_focused_app,
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
            cli_path::set_cli_path,
            cli_path::get_cli_path,
            // Setup health (hotkey/extension problems shown in the UI)
            health::get_health_issues,
            // Highlight
            highlight::highlight_file_cmd,
            highlight::highlight_shell_cmd,
            // About widget: version only. The update check itself runs in
            // the webview via fetch() - no Rust HTTP/TLS dep needed.
            files::get_lookapp_version,
            commands::get_install_method,
            commands::start_windows_update,
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
