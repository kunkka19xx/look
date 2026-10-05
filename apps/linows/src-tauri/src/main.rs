// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod consts;
mod host;
mod launcher_hotkey;
mod pickers;
mod window;

use linows_backend::crash;
use linows_backend::geometry;
use linows_backend::health;
use linows_backend::launch_query;
use linows_backend::look_engine::modes;
use linows_backend::platform::IconCache;
#[cfg(target_os = "linux")]
use linows_backend::platform::linux;
use linows_backend::startup;
use linows_backend::state::AppState;
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
        linux::transparency::has_compositor()
    }

    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}

/// Grace period (ms) after show - ignore focus-loss within this window.
const AUTO_HIDE_GRACE_MS: u64 = 300;
/// Guard (ms) to prevent re-showing after auto-hide (GNOME X11 race).
const AUTO_HIDE_RESHOW_GUARD_MS: u64 = 200;
/// Arm the launchpad, then hide the window once the webview has painted that
/// frame - see `window::hide_armed`.
fn hide_launcher(window: &tauri::WebviewWindow) {
    window::hide_armed(window);
}

/// Logical top edge for a shown window.
fn window_top(monitor: &tauri::Monitor) -> f64 {
    let scale = monitor.scale_factor();
    monitor.position().y as f64 / scale + geometry::top_offset(monitor.size().height, scale)
}

/// Toggle the main window: hide if visible, show (centered) if hidden.
fn toggle_window(app_handle: &tauri::AppHandle) {
    let Some(window) = app_handle.get_webview_window(consts::MAIN_WINDOW) else {
        return;
    };
    if window::launcher_visible(&window) {
        #[cfg(target_os = "linux")]
        linux::window_focus::notify_hidden();
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
    let placed_by_compositor = host::layer_shell::is_active();
    #[cfg(not(target_os = "linux"))]
    let placed_by_compositor = false;

    // Tiling WMs (i3, sway, Hyprland) ignore set_position on unmapped
    // windows - they apply their own placement on map. So we must
    // recenter AFTER show. Desktop environments (GNOME, KDE, …) work
    // best with recenter BEFORE show to avoid a visible jump.
    #[cfg(target_os = "linux")]
    let tiling = linux::wm::is_tiling_wm();
    #[cfg(not(target_os = "linux"))]
    let tiling = false;

    if !placed_by_compositor {
        if !tiling {
            recenter_window(window);
        }
        let _ = window.set_always_on_top(true);
    }
    window::show_launcher_before_event(window, || {
        if !placed_by_compositor && tiling {
            recenter_window(window);
        }
    });
    // For X11 windows (native X11, or XWayland when the AppImage forces
    // GDK_BACKEND=x11), bypass the compositor's focus-stealing
    // prevention by bumping _NET_WM_USER_TIME before activation.
    #[cfg(target_os = "linux")]
    if linux::transparency::window_is_x11() {
        linux::window_focus::activate_self();
        linux::window_focus::notify_shown();
    }

    window::focus_launcher(window);
}

/// Center and scale a window to fit the current monitor.
/// Called once at startup. Avoid calling on toggle - see toggle_window.
///
/// Returns the logical size and monitor-relative top edge it settled on. The
/// layer surface needs them from here rather than reading `inner_size` back: a
/// Wayland window the compositor has not configured yet still reports
/// tauri.conf's default.
fn center_and_scale_window(window: &tauri::WebviewWindow) -> Option<((i32, i32), i32)> {
    let monitor = monitor_at_cursor(window)?;
    let pos = monitor.position();
    let screen = monitor.size();
    let scale = monitor.scale_factor();
    let (win_w, win_h) = geometry::scaled_window_size(screen.height, scale);
    let logical_screen_w = screen.width as f64 / scale;
    let logical_screen_h = screen.height as f64 / scale;
    eprintln!(
        "[look:scale] monitor={}x{} scale={} logical_screen={}x{} → window={}x{}",
        screen.width, screen.height, scale, logical_screen_w, logical_screen_h, win_w, win_h,
    );
    // Lock min/max to the scaled size: on Wayland, hide()/show() can
    // otherwise revert to tauri.conf's default (840×560) on remap,
    // producing a visible "big rectangle then snap" on toggle.
    resize_locked(window, tauri::LogicalSize::new(win_w as f64, win_h as f64));
    let lx = pos.x as f64 / scale + (logical_screen_w - win_w as f64) / 2.0;
    let ly = window_top(&monitor);
    let _ = window.set_position(tauri::LogicalPosition::new(lx, ly));
    Some((
        (win_w as i32, win_h as i32),
        geometry::top_offset(screen.height, scale).round() as i32,
    ))
}

/// Find the monitor that contains the cursor. Falls back to the window's
/// current monitor, then the first available monitor.
pub(crate) fn monitor_at_cursor(window: &tauri::WebviewWindow) -> Option<tauri::Monitor> {
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
    let wayland = !linux::transparency::window_is_x11();
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
            let pos = linux::gnome_ext::get_pointer()
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
    let (win_w, win_h) = geometry::scaled_window_size(screen.height, scale);
    let logical_screen_w = screen.width as f64 / scale;
    resize_locked(window, tauri::LogicalSize::new(win_w as f64, win_h as f64));
    let lx = pos.x as f64 / scale + (logical_screen_w - win_w as f64) / 2.0;
    let ly = window_top(&monitor);
    let _ = window.set_position(tauri::LogicalPosition::new(lx, ly));
}

/// Relaxes min/max first so the old lock can't clamp the new size, then locks
/// them to it again.
pub(crate) fn resize_locked(window: &tauri::WebviewWindow, size: tauri::LogicalSize<f64>) {
    let _ = window.set_min_size(None::<tauri::Size>);
    let _ = window.set_max_size(None::<tauri::Size>);
    let _ = window.set_size(size);
    let _ = window.set_min_size(Some(tauri::Size::Logical(size)));
    let _ = window.set_max_size(Some(tauri::Size::Logical(size)));
}

#[cfg(target_os = "linux")]
fn is_wayland() -> bool {
    use std::sync::OnceLock;
    static CACHED: OnceLock<bool> = OnceLock::new();
    *CACHED.get_or_init(linux::transparency::is_wayland)
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
                linux::gnome_ext::ensure_installed();
            }

            let handle = app_handle.clone();
            let bind_key = linows_backend::hotkey::configured().enabled;
            linux::wayland_shortcut::start(bind_key, move || {
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
    linux::window_focus::cache_self_window();
    let window = app
        .get_webview_window(consts::MAIN_WINDOW)
        .expect("main window missing");
    linux::window_focus::start_active_window_monitor(move || {
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
    if host::layer_shell::is_active() {
        let w = window.clone();
        host::layer_shell::on_focus(move |focused| on_focus_change(&w, focused));
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

fn main() {
    crash::install_panic_hook();

    if std::env::args().any(|a| a == "--version" || a == "-V") {
        println!("lookapp {}", linows_backend::APP_VERSION);
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
    startup::setup_dev_env();

    #[cfg(target_os = "linux")]
    let disable_gpu =
        host::gpu::detect_and_disable_virtual_gpu() || host::gpu::disable_gpu_from_config();

    startup::sync_integrations();

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
                launch_query::park_launch(&launch);
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
        .manage(IconCache::new());

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
                host::gpu::disable_gpu_acceleration(app);
            }
            #[cfg(target_os = "linux")]
            host::gpu::trim_memory_features(app);

            linows_backend::host::install(Box::new(window::TauriHost(app.handle().clone())));
            app.state::<AppState>().start_bootstrap();
            linows_backend::clipboard::start_monitor();

            // Probes the user's systemd manager, so the first launch of a
            // session does not wait on it.
            #[cfg(target_os = "linux")]
            linux::prime_user_session();

            register_shortcuts(app, use_wayland);

            #[cfg(debug_assertions)]
            health::report_fake_issues_from_env();

            // X11-window concerns (focus monitor, scrolling tweak) follow the
            // window backend: they also apply to the AppImage's XWayland
            // window on a Wayland session.
            #[cfg(target_os = "linux")]
            if linux::transparency::window_is_x11() {
                setup_x11_focus_monitor(app);
                host::gpu::disable_smooth_scrolling_x11(app);
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
            let placement = center_and_scale_window(&window);
            #[cfg(target_os = "linux")]
            host::layer_shell::attach(
                &window,
                placement.map(|(size, _)| size),
                placement.map(|(_, top)| top),
            );
            #[cfg(not(target_os = "linux"))]
            let _ = placement;
            apply_transparency(&window);
            // Needs the main thread and a live window: the surface pointer
            // comes off the window handle.
            #[cfg(target_os = "linux")]
            host::blur::init(&window);
            // Before the first show: on the layer-shell path focus arrives as
            // a GTK signal, and a handler connected afterwards misses the one
            // that would focus the query input.
            setup_window_events(&window);
            #[cfg(target_os = "linux")]
            if supports_transparency() {
                window::show_launcher(&window);
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
                launch_query::park_launch(&launch);
                show_window(&window);
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
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
            commands::take_launch_query,
            commands::apply_layout,
            commands::confirm_hide,
            commands::set_blur_region,
            commands::quit_app,
            commands::get_config,
            commands::set_config,
            commands::launcher_hotkey_state,
            commands::hotkey_check,
            commands::launcher_hotkey_set_active,
            commands::reset_config,
            commands::get_file_meta,
            commands::get_app_version,
            commands::list_folder,
            commands::is_dev_build,
            commands::copy_files_to_clipboard,
            commands::get_home_dir,
            commands::get_quick_folders,
            commands::list_fonts,
            commands::scan_music_folder,
            commands::pick_folder,
            commands::pick_image,
            commands::run_shell_command,
            commands::tool_actions,
            commands::perform_tool_action,
            commands::source_block,
            commands::source_blocks,
            commands::perform_block,
            commands::source_rows,
            commands::source_preview,
            commands::refresh_run_blocks,
            commands::get_icon,
            commands::get_platform,
            commands::list_candidate_drives,
            commands::set_window_effect,
            commands::eval_calc,
            commands::calc_inline,
            commands::get_system_info,
            commands::system_uptime,
            commands::list_processes,
            commands::kill_process,
            commands::search_processes,
            commands::search_kill_targets,
            commands::process_detail,
            commands::process_cpu,
            commands::list_running_apps,
            commands::activate_running_app,
            commands::todo_list,
            commands::todo_save,
            commands::translate,
            commands::instant_has_match,
            commands::definitional_entity,
            commands::instant_answer,
            commands::duckduckgo_answer,
            commands::wikipedia_answer,
            commands::web_suggestions,
            commands::classify_url,
            commands::record_url_hit,
            commands::recent_urls,
            commands::quick_actions,
            commands::launchpad_layout,
            commands::launchpad_tile_values,
            commands::refresh_launchpad_tiles,
            commands::press_launchpad_tile,
            commands::launchpad_warnings,
            commands::quick_action_state,
            commands::quick_action_apply,
            commands::quick_action_apply_item,
            commands::weather_current,
            commands::now_playing_current,
            commands::now_playing_command,
            commands::lunar_date,
            commands::speed_test,
            commands::local_ipv4,
            commands::get_clipboard_history,
            commands::delete_clipboard_entry,
            commands::get_clipboard_images,
            commands::delete_clipboard_image,
            commands::clipboard_image_data_url,
            commands::copy_clipboard_image,
            commands::copy_to_clipboard,
            commands::copy_to_clipboard_labeled,
            commands::clipboard_paste_blocker,
            commands::paste_into_focused_app,
            commands::music_play,
            commands::music_pause,
            commands::music_resume,
            commands::music_stop,
            commands::music_is_finished,
            commands::trash_paths,
            commands::count_trash_items,
            commands::empty_trash,
            commands::set_autostart,
            commands::get_autostart,
            commands::set_cli_path,
            commands::get_cli_path,
            commands::get_health_issues,
            commands::highlight_file_cmd,
            commands::highlight_shell_cmd,
            commands::get_lookapp_version,
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
                linux::wayland_shortcut::cleanup_keybinding();
            }
        });
}
