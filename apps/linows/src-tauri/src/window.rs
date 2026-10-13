//! The launcher window as Tauri drives it: the armed hide, the show that
//! tells the frontend whether to clear the query, and the two adapters that
//! let the backend ask for either without knowing Tauri.

use std::sync::atomic::{AtomicU64, Ordering};

use linows_backend::health::HealthIssue;
use linows_backend::host::{Host, LauncherWindow};
use linows_backend::query_retention;
use tauri::{Emitter, Manager};

use crate::consts;

/// Longest the window stays up waiting for the frontend to paint the armed
/// frame. `confirm_hide` ends the wait as soon as that frame lands, so this
/// only runs out for a webview that never answers.
const HIDE_ARM_GRACE: std::time::Duration = std::time::Duration::from_millis(60);

/// Id of the dismissal still in flight, 0 when none is; a show clears it so a
/// fallback the user already undid can't pull the window back down.
static PENDING_HIDE: AtomicU64 = AtomicU64::new(0);
static HIDE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Arm the entrance, then hide once the frontend has painted that frame.
///
/// The compositor keeps the last buffer the webview painted and presents it
/// when the window maps again, so hiding in the same frame leaves the fully
/// revealed panel to flash on the next summon before the entrance rewinds and
/// replays. Every dismiss goes through here.
pub fn hide_armed(window: &tauri::WebviewWindow) {
    let arm = HIDE_COUNTER.fetch_add(1, Ordering::Relaxed) + 1;
    PENDING_HIDE.store(arm, Ordering::Relaxed);
    let _ = window.emit(consts::EVENT_WINDOW_HIDDEN, arm);
    let window = window.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(HIDE_ARM_GRACE).await;
        if PENDING_HIDE
            .compare_exchange(arm, 0, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {
            hide_now(&window);
        }
    });
}

/// The frontend has painted the armed frame; the window can go now. Keyed on
/// `arm` so a late confirmation can't hide a window a later dismiss just armed.
pub fn confirm_hide(window: &tauri::WebviewWindow, arm: u64) {
    if PENDING_HIDE
        .compare_exchange(arm, 0, Ordering::Relaxed, Ordering::Relaxed)
        .is_ok()
    {
        hide_now(window);
    }
}

/// Take the launcher off screen. With layer shell attached, the surface on
/// screen is not the one Tauri hands out.
pub fn hide_now(window: &tauri::WebviewWindow) {
    // The one point every route off screen passes through, armed or not.
    query_retention::mark_hidden_now();
    #[cfg(target_os = "linux")]
    if crate::host::layer_shell::is_active() {
        crate::host::layer_shell::hide();
        return;
    }
    let _ = window.hide();
}

/// Give the launcher keyboard focus. A layer surface takes it on its own
/// through `keyboard-interactivity`, and `set_focus` would be harmful there:
/// it reaches `gtk_window_present` and maps the husk toplevel.
pub fn focus_launcher(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "linux")]
    if crate::host::layer_shell::is_active() {
        return;
    }
    let _ = window.set_focus();
}

/// Whether the launcher is on screen. The husk toplevel never maps, so its own
/// `is_visible` reports false for a launcher that is plainly up.
pub fn launcher_visible(window: &tauri::WebviewWindow) -> bool {
    #[cfg(target_os = "linux")]
    if let Some(visible) = crate::host::layer_shell::visible() {
        return visible;
    }
    window.is_visible().unwrap_or(false)
}

/// Default show path when no native geometry adjustment is needed before the
/// frontend event.
pub fn show_launcher(window: &tauri::WebviewWindow) {
    show_launcher_before_event(window, || {});
}

/// Show the launcher, apply any final native geometry, then notify the frontend.
/// Tiling WMs can only reposition a mapped window, so their toggle path uses
/// this hook to recenter after `show` but before focus and reveal animations.
/// Every show ultimately goes through here.
pub fn show_launcher_before_event(window: &tauri::WebviewWindow, before_event: impl FnOnce()) {
    PENDING_HIDE.store(0, Ordering::Relaxed);
    #[cfg(target_os = "linux")]
    if crate::host::layer_shell::is_active() {
        crate::host::layer_shell::show();
        before_event();
        let Some(clear_query) = query_retention::query_clear_decision_after_show(true) else {
            return;
        };
        let _ = window.emit(consts::EVENT_WINDOW_SHOWN, clear_query);
        return;
    }
    let Some(clear_query) = query_retention::query_clear_decision_after_show(window.show().is_ok())
    else {
        return;
    };
    #[cfg(target_os = "linux")]
    if linows_backend::platform::linux::wm::is_niri() {
        linows_backend::platform::linux::niri::ensure_self_floating();
    }
    before_event();
    let _ = window.emit(consts::EVENT_WINDOW_SHOWN, clear_query);
}

/// The launcher window handed to the backend. `WebviewWindow` is a cloneable
/// handle, so one of these can cross a thread.
pub struct TauriWindow(pub tauri::WebviewWindow);

impl LauncherWindow for TauriWindow {
    fn hide(&self) {
        hide_armed(&self.0);
    }

    fn hide_now(&self) {
        hide_now(&self.0);
    }

    fn show(&self) {
        show_launcher(&self.0);
    }

    fn focus(&self) {
        focus_launcher(&self.0);
    }

    fn is_visible(&self) -> bool {
        launcher_visible(&self.0)
    }
}

/// The backend's process-wide hooks, answered with Tauri events.
pub struct TauriHost(pub tauri::AppHandle);

impl Host for TauriHost {
    fn index_ready(&self) {
        if let Some(window) = self.0.get_webview_window(consts::MAIN_WINDOW) {
            let _ = window.emit(consts::EVENT_INDEX_READY, ());
        }
    }

    fn health_changed(&self, issues: Vec<HealthIssue>) {
        let _ = self.0.emit(consts::EVENT_HEALTH_CHANGED, issues);
    }

    #[cfg(target_os = "linux")]
    fn own_clipboard(&self, forms: Vec<linows_backend::host::ClipForm>) -> bool {
        crate::host::clipboard::own_clipboard(&self.0, forms)
    }
}
