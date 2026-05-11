//! Linux-specific window focusing via X11.
//!
//! Uses `x11rb` to find windows by WM_CLASS and send a proper
//! `_NET_ACTIVE_WINDOW` client message to the window manager.
//! Works on GNOME, KDE, and any EWMH-compliant WM — including NixOS
//! where `xdotool` / `wmctrl` are typically not installed.
//!
//! Also provides a background monitor that:
//! - Retries focus activation after show (handles GNOME's async mapping)
//! - Auto-hides Look when another window becomes active
//!
//! Tested: GNOME Xorg, i3 X11.
//! Not yet tested: GNOME Wayland (auto-hide disabled there; see TODO in main.rs).

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;

/// Cached X11 window ID for Look's own window, resolved once at startup.
static SELF_WID: AtomicU32 = AtomicU32::new(0);

/// Set to true when Look is shown and needs focus.
/// The monitor thread will retry activation until focus is granted.
static NEEDS_FOCUS: AtomicBool = AtomicBool::new(false);

/// Set to true once Look has confirmed focus (via _NET_ACTIVE_WINDOW).
/// Auto-hide only fires when this transitions from true → false.
static HAS_FOCUS: AtomicBool = AtomicBool::new(false);

/// Call once after the window is mapped to cache Look's X11 window ID
/// and hide from GNOME dock/taskbar.
pub fn cache_self_window() {
    if let Some(wid) = find_window_by_class("lookapp") {
        SELF_WID.store(wid, Ordering::SeqCst);
        set_skip_taskbar(wid);
    }
}

/// Set _NET_WM_STATE_SKIP_TASKBAR so Look doesn't appear in the GNOME dock.
fn set_skip_taskbar(wid: Window) {
    let Ok((conn, screen_num)) = x11rb::connect(None) else {
        return;
    };
    let root = conn.setup().roots[screen_num].root;

    let Ok(state_cookie) = conn.intern_atom(false, b"_NET_WM_STATE") else {
        return;
    };
    let Ok(skip_cookie) = conn.intern_atom(false, b"_NET_WM_STATE_SKIP_TASKBAR") else {
        return;
    };
    let Ok(state_atom) = state_cookie.reply() else {
        return;
    };
    let Ok(skip_atom) = skip_cookie.reply() else {
        return;
    };

    // Send _NET_WM_STATE client message: action=1 (add), atom=SKIP_TASKBAR
    let data = ClientMessageData::from([1u32, skip_atom.atom, 0, 0, 0]);
    let event = ClientMessageEvent::new(32, wid, state_atom.atom, data);
    let _ = conn.send_event(
        false,
        root,
        EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
        event,
    );
    let _ = conn.flush();
}

/// Notify that Look was just shown and needs focus activation.
pub fn notify_shown() {
    HAS_FOCUS.store(false, Ordering::SeqCst);
    NEEDS_FOCUS.store(true, Ordering::SeqCst);
}

/// Notify that Look was hidden (cancel any pending focus retry).
pub fn notify_hidden() {
    NEEDS_FOCUS.store(false, Ordering::SeqCst);
    HAS_FOCUS.store(false, Ordering::SeqCst);
}

/// Activate Look's own window, bypassing Mutter's focus-stealing prevention
/// by updating `_NET_WM_USER_TIME` before sending the activation request.
pub fn activate_self() -> bool {
    let wid = SELF_WID.load(Ordering::Relaxed);
    if wid == 0 {
        return false;
    }

    let Ok((conn, screen_num)) = x11rb::connect(None) else {
        return false;
    };
    let root = conn.setup().roots[screen_num].root;

    bump_user_time(&conn, root, wid);
    activate_window(&conn, root, wid)
}

/// Try to focus an existing window whose `WM_CLASS` matches `wm_class`.
pub fn try_focus(wm_class: &str) -> bool {
    let Some(wid) = find_window_by_class(wm_class) else {
        return false;
    };

    let Ok((conn, screen_num)) = x11rb::connect(None) else {
        return false;
    };
    let root = conn.setup().roots[screen_num].root;
    activate_window(&conn, root, wid)
}

// --- internals ---

fn find_window_by_class(wm_class: &str) -> Option<u32> {
    let (conn, screen_num) = x11rb::connect(None).ok()?;
    let root = conn.setup().roots[screen_num].root;
    let target = wm_class.to_lowercase();

    let windows = get_client_list(&conn, root)?;
    windows
        .into_iter()
        .find(|&wid| wm_class_matches(&conn, wid, &target))
}

fn get_client_list(conn: &impl Connection, root: Window) -> Option<Vec<Window>> {
    let atom = conn
        .intern_atom(false, b"_NET_CLIENT_LIST")
        .ok()?
        .reply()
        .ok()?
        .atom;
    let reply = conn
        .get_property(false, root, atom, AtomEnum::WINDOW, 0, 1024)
        .ok()?
        .reply()
        .ok()?;
    Some(reply.value32()?.collect())
}

fn wm_class_matches(conn: &impl Connection, wid: Window, target: &str) -> bool {
    let Ok(cookie) = conn.get_property(false, wid, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, 256)
    else {
        return false;
    };
    let Ok(reply) = cookie.reply() else {
        return false;
    };
    String::from_utf8_lossy(&reply.value)
        .to_lowercase()
        .contains(target)
}

fn bump_user_time(conn: &impl Connection, root: Window, our_wid: Window) {
    let Ok(time_cookie) = conn.intern_atom(false, b"_NET_WM_USER_TIME") else {
        return;
    };
    let Ok(time_atom) = time_cookie.reply() else {
        return;
    };
    let Ok(active_cookie) = conn.intern_atom(false, b"_NET_ACTIVE_WINDOW") else {
        return;
    };
    let Ok(active_atom) = active_cookie.reply() else {
        return;
    };

    let active_wid = conn
        .get_property(false, root, active_atom.atom, AtomEnum::WINDOW, 0, 1)
        .ok()
        .and_then(|c| c.reply().ok())
        .and_then(|r| r.value32().and_then(|mut v| v.next()))
        .unwrap_or(0);

    let their_time = if active_wid != 0 {
        conn.get_property(false, active_wid, time_atom.atom, AtomEnum::CARDINAL, 0, 1)
            .ok()
            .and_then(|c| c.reply().ok())
            .and_then(|r| r.value32().and_then(|mut v| v.next()))
            .unwrap_or(1)
    } else {
        1
    };

    let our_time = their_time.wrapping_add(1);
    let _ = conn.change_property(
        PropMode::REPLACE,
        our_wid,
        time_atom.atom,
        AtomEnum::CARDINAL,
        32,
        1,
        &our_time.to_ne_bytes(),
    );
    let _ = conn.flush();
}

fn activate_window(conn: &impl Connection, root: Window, wid: Window) -> bool {
    let Ok(cookie) = conn.intern_atom(false, b"_NET_ACTIVE_WINDOW") else {
        return false;
    };
    let Ok(atom) = cookie.reply() else {
        return false;
    };

    let event = ClientMessageEvent::new(32, wid, atom.atom, [2u32, 0, 0, 0, 0]);
    let mask = EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY;
    let _ = conn.send_event(false, root, mask, event);
    let _ = conn.set_input_focus(InputFocus::PARENT, wid, x11rb::CURRENT_TIME);
    let _ = conn.flush();
    true
}

fn read_active_window(conn: &impl Connection, root: Window, atom: Atom) -> u32 {
    conn.get_property(false, root, atom, AtomEnum::WINDOW, 0, 1)
        .ok()
        .and_then(|c| c.reply().ok())
        .and_then(|r| r.value32().and_then(|mut v| v.next()))
        .unwrap_or(0)
}

// --- Active window monitor ---

static MONITOR_RUNNING: AtomicBool = AtomicBool::new(false);

/// Returns true for WMs that use focus-follows-mouse (i3, sway, etc.)
/// where auto-hide on focus loss would fight the WM's focus policy.
fn is_focus_follows_mouse_wm() -> bool {
    // I3SOCK is always set when running under i3
    if std::env::var("I3SOCK").is_ok() {
        return true;
    }
    // SWAYSOCK for sway
    if std::env::var("SWAYSOCK").is_ok() {
        return true;
    }
    false
}

/// Start a background thread that:
/// 1. Retries focus activation after show until focus is confirmed
/// 2. Auto-hides Look when another window becomes active (focus lost)
pub fn start_active_window_monitor<F>(on_lost_focus: F)
where
    F: Fn() + Send + 'static,
{
    if MONITOR_RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }

    std::thread::spawn(move || {
        let Ok((conn, screen_num)) = x11rb::connect(None) else {
            MONITOR_RUNNING.store(false, Ordering::SeqCst);
            return;
        };
        let root = conn.setup().roots[screen_num].root;
        let skip_auto_hide = is_focus_follows_mouse_wm();

        let _ = conn.change_window_attributes(
            root,
            &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE),
        );
        let _ = conn.flush();

        let Ok(active_cookie) = conn.intern_atom(false, b"_NET_ACTIVE_WINDOW") else {
            return;
        };
        let Ok(active_atom) = active_cookie.reply() else {
            return;
        };

        loop {
            // Process any pending X11 events
            while let Ok(Some(event)) = conn.poll_for_event() {
                if let x11rb::protocol::Event::PropertyNotify(ev) = event
                    && ev.atom == active_atom.atom
                {
                    handle_active_change(
                        &conn,
                        root,
                        active_atom.atom,
                        &on_lost_focus,
                        skip_auto_hide,
                    );
                }
            }

            // If Look was just shown and needs focus, retry activation
            if NEEDS_FOCUS.load(Ordering::SeqCst) {
                let our_wid = SELF_WID.load(Ordering::Relaxed);
                let active = read_active_window(&conn, root, active_atom.atom);

                if active == our_wid {
                    // Focus confirmed — stop retrying
                    NEEDS_FOCUS.store(false, Ordering::SeqCst);
                    HAS_FOCUS.store(true, Ordering::SeqCst);
                } else {
                    // Retry activation
                    bump_user_time(&conn, root, our_wid);
                    activate_window(&conn, root, our_wid);
                }
            }

            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    });
}

fn handle_active_change<F>(
    conn: &impl Connection,
    root: Window,
    atom: Atom,
    on_lost_focus: &F,
    skip_auto_hide: bool,
) where
    F: Fn(),
{
    let our_wid = SELF_WID.load(Ordering::Relaxed);
    let active = read_active_window(conn, root, atom);

    if active == our_wid {
        // We gained focus
        NEEDS_FOCUS.store(false, Ordering::SeqCst);
        HAS_FOCUS.store(true, Ordering::SeqCst);
    } else if active != 0 && HAS_FOCUS.swap(false, Ordering::SeqCst) {
        // We HAD focus and now lost it — auto-hide.
        if skip_auto_hide {
            // On focus-follows-mouse WMs (i3, sway), only auto-hide when
            // the user clicked outside (mouse button is still pressed).
            // Moving the mouse away just changes focus passively.
            let clicked = conn
                .query_pointer(root)
                .ok()
                .and_then(|c| c.reply().ok())
                .is_some_and(|r| u16::from(r.mask) & 0x700 != 0);
            if clicked {
                on_lost_focus();
            }
        } else {
            on_lost_focus();
        }
    }
}
