//! Ctrl+I on a clipboard row: put the clip where the cursor was.
//!
//! The frontend has copied the row already, so what is left is the part a web
//! view cannot do: get out of the way, then type the paste chord into the
//! window underneath once it has the keyboard back.

use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
use crate::platform::linux::focused_app::SELF_APP_ID;

#[cfg(target_os = "linux")]
const GNOME: &str = "gnome";

const HIDE_POLL: Duration = Duration::from_millis(10);
const HIDE_TIMEOUT: Duration = Duration::from_millis(600);
const FOCUS_POLL: Duration = Duration::from_millis(15);
const FOCUS_TIMEOUT: Duration = Duration::from_millis(400);
/// A window that has just taken focus still has to give some text view the
/// cursor. Also the whole wait where the session will not name the focus.
const SETTLE: Duration = Duration::from_millis(80);

/// Why the chord cannot be sent, asked while the launcher is still up to show
/// the answer. `None` means go ahead.
#[tauri::command]
pub fn clipboard_paste_blocker() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        use crate::platform::linux::{keysynth, transparency, wm};

        if keysynth::available() {
            return None;
        }
        if !transparency::is_wayland() {
            return Some(
                "The X server has no XTEST extension, so nothing can type the paste \
                 for you - the clip is copied, press Ctrl+V"
                    .to_string(),
            );
        }
        if wm::detect_compositor().as_deref() == Some(GNOME) {
            return Some(
                "Look's GNOME extension is not answering, so nothing can type the \
                 paste for you - log out and back in, and meanwhile press Ctrl+V"
                    .to_string(),
            );
        }
        Some(
            "This compositor lets no app type into another window, so the clip is \
             copied only - press Ctrl+V to paste it"
                .to_string(),
        )
    }

    #[cfg(not(target_os = "linux"))]
    None
}

#[tauri::command]
pub fn paste_into_focused_app(window: tauri::WebviewWindow) {
    crate::commands::hide_armed(&window);
    std::thread::spawn(move || {
        // Sent any earlier, the chord types into the launcher on its way out.
        if !wait_until_hidden(&window) {
            return;
        }
        let shift = wait_for_target().is_some_and(|app| is_terminal(&app));
        std::thread::sleep(SETTLE);
        if !send_paste(shift) {
            eprintln!("[paste] the chord reached nothing; the clip is still on the clipboard");
        }
    });
}

fn send_paste(shift: bool) -> bool {
    #[cfg(target_os = "linux")]
    {
        crate::platform::linux::keysynth::send_paste(shift)
    }

    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::keysynth::send_paste(shift)
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = shift;
        false
    }
}

fn wait_until_hidden(window: &tauri::WebviewWindow) -> bool {
    let deadline = Instant::now() + HIDE_TIMEOUT;
    while Instant::now() < deadline {
        if !crate::commands::launcher_visible(window) {
            return true;
        }
        std::thread::sleep(HIDE_POLL);
    }
    false
}

/// Waits for the window underneath to take the keyboard back, and names it,
/// since the app decides the chord. `None` where the session will not say.
#[cfg(target_os = "linux")]
fn wait_for_target() -> Option<String> {
    use crate::platform::linux::focused_app;

    // Focus lands on the window underneath a moment after the launcher goes,
    // so an early unnamed focus is worth waiting on. Where the session names
    // no focus at all, waiting only delays the chord by the whole timeout.
    if !focused_app::focus_is_reportable() {
        return None;
    }
    let deadline = Instant::now() + FOCUS_TIMEOUT;
    loop {
        if let Some(app) = focused_app::focused_app_id()
            && !app.eq_ignore_ascii_case(SELF_APP_ID)
        {
            return Some(app);
        }
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(FOCUS_POLL);
    }
}

/// Nothing to name: every Windows app takes plain Ctrl+V, terminals included.
#[cfg(target_os = "windows")]
fn wait_for_target() -> Option<String> {
    let deadline = Instant::now() + FOCUS_TIMEOUT;
    while Instant::now() < deadline && crate::platform::windows::keysynth::self_is_foreground() {
        std::thread::sleep(FOCUS_POLL);
    }
    None
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn wait_for_target() -> Option<String> {
    None
}

/// Ctrl+V in a terminal is the literal-next key; paste there is Ctrl+Shift+V.
fn is_terminal(app_id: &str) -> bool {
    let id = app_id.trim();
    if id.is_empty() {
        return false;
    }
    if look_tools::surface(id) == look_tools::Surface::Terminal {
        return true;
    }
    // A reverse-DNS app id names the binary in its last segment.
    id.rsplit('.')
        .next()
        .is_some_and(|stem| look_tools::surface(stem) == look_tools::Surface::Terminal)
}

#[cfg(test)]
mod tests {
    use super::is_terminal;

    #[test]
    fn terminals_are_recognised_by_app_id() {
        assert!(is_terminal("kitty"));
        assert!(is_terminal("Alacritty"));
        assert!(is_terminal("foot"));
        assert!(is_terminal("org.kde.konsole"));
        assert!(is_terminal("org.gnome.Terminal"));
        assert!(is_terminal("org.gnome.Console"));
        assert!(is_terminal("com.mitchellh.ghostty"));
    }

    #[test]
    fn ordinary_apps_are_not_terminals() {
        assert!(!is_terminal("firefox"));
        assert!(!is_terminal("org.gnome.Nautilus"));
        assert!(!is_terminal(""));
    }
}
