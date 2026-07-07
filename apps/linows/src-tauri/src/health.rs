//! Setup health issues surfaced to the user instead of stderr-only
//! breadcrumbs.
//!
//! The app is usually launched from a menu entry, so `eprintln!` is invisible.
//! When something the user must act on fails (the global hotkey can't be
//! registered, the GNOME extension needs a re-login), modules report it here;
//! the frontend shows the collected issues as a persistent, dismissible
//! notice. Issues land at any time - during `setup` (before the webview runs
//! JS) or minutes later from a background thread - so the frontend both pulls
//! the list on init and listens for the change event.

use serde::Serialize;
use std::sync::Mutex;
use tauri::Emitter;

pub const EVENT_HEALTH_CHANGED: &str = "health-changed";

/// Stable issue ids: reports are deduped by id and the frontend keys
/// dismissal persistence on them.
pub const ISSUE_HOTKEY: &str = "hotkey";
#[cfg(target_os = "linux")]
pub const ISSUE_GNOME_EXT: &str = "gnome-ext";

#[derive(Debug, Clone, Serialize)]
pub struct HealthIssue {
    pub id: &'static str,
    pub message: String,
}

static ISSUES: Mutex<Vec<HealthIssue>> = Mutex::new(Vec::new());

/// Record an issue and notify the frontend. The first report for an id wins;
/// repeat reports (e.g. the stale-extension warning firing on every search)
/// are silently dropped.
pub fn report(id: &'static str, message: String) {
    {
        let Ok(mut issues) = ISSUES.lock() else {
            return;
        };
        if issues.iter().any(|i| i.id == id) {
            return;
        }
        eprintln!("[look:health] {message}");
        issues.push(HealthIssue { id, message });
    }
    if let Some(handle) = crate::state::app_handle() {
        let _ = handle.emit(EVENT_HEALTH_CHANGED, snapshot());
    }
}

fn snapshot() -> Vec<HealthIssue> {
    ISSUES.lock().map(|i| i.clone()).unwrap_or_default()
}

#[tauri::command]
pub fn get_health_issues() -> Vec<HealthIssue> {
    snapshot()
}

/// Debug-only test hook: `LOOK_FAKE_HEALTH_ISSUE=hotkey,late:gnome-ext`
/// reports canned issues so the notice UI can be tested without reproducing
/// real failures on each OS. Bare ids are reported during setup (exercises
/// the get_health_issues pull path, before the webview runs JS); ids
/// prefixed with `late:` arrive seconds after startup (exercises the
/// health-changed push path). Compiled out of release builds.
#[cfg(debug_assertions)]
pub fn report_fake_issues_from_env() {
    const LATE_DELAY: std::time::Duration = std::time::Duration::from_secs(5);

    let Ok(spec) = std::env::var("LOOK_FAKE_HEALTH_ISSUE") else {
        return;
    };
    for part in spec.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let (late, name) = match part.strip_prefix("late:") {
            Some(rest) => (true, rest),
            None => (false, part),
        };
        let id: &'static str = match name {
            "hotkey" => ISSUE_HOTKEY,
            #[cfg(target_os = "linux")]
            "gnome-ext" => ISSUE_GNOME_EXT,
            other => {
                eprintln!("[look:health] unknown fake issue id: {other}");
                continue;
            }
        };
        let message = format!(
            "Fake '{id}' issue from LOOK_FAKE_HEALTH_ISSUE, with enough text \
             to check how the notice wraps across lines. Unset the variable \
             and restart to clear it."
        );
        if late {
            std::thread::spawn(move || {
                std::thread::sleep(LATE_DELAY);
                report(id, message);
            });
        } else {
            report(id, message);
        }
    }
}
