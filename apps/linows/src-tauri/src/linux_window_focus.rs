//! Linux-specific window focusing via X11.
//!
//! Uses `x11rb` to find windows by WM_CLASS and send a proper
//! `_NET_ACTIVE_WINDOW` client message to the window manager.
//! Works on GNOME, KDE, and any EWMH-compliant WM — including NixOS
//! where `xdotool` / `wmctrl` are typically not installed.

use std::sync::atomic::{AtomicU32, Ordering};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;

/// Cached X11 window ID for Look's own window, resolved once at startup.
static SELF_WID: AtomicU32 = AtomicU32::new(0);

/// Call once after the window is mapped to cache Look's X11 window ID.
pub fn cache_self_window() {
    if let Some(wid) = find_window_by_class("look-desktop") {
        SELF_WID.store(wid, Ordering::Relaxed);
    }
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

    // Bypass focus-stealing prevention: set our _NET_WM_USER_TIME
    // to be newer than the currently focused window's timestamp.
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
    for wid in windows {
        if wm_class_matches(&conn, wid, &target) {
            return Some(wid);
        }
    }
    None
}

fn get_client_list(conn: &impl Connection, root: Window) -> Option<Vec<Window>> {
    let atom = conn.intern_atom(false, b"_NET_CLIENT_LIST").ok()?.reply().ok()?.atom;
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
    String::from_utf8_lossy(&reply.value).to_lowercase().contains(target)
}

/// Read _NET_WM_USER_TIME from the currently active window and set ours
/// to that value + 1.  This convinces Mutter that our window has had more
/// recent user activity than the focused window, bypassing focus-stealing
/// prevention.
fn bump_user_time(conn: &impl Connection, root: Window, our_wid: Window) {
    let Ok(time_cookie) = conn.intern_atom(false, b"_NET_WM_USER_TIME") else { return };
    let Ok(time_atom) = time_cookie.reply() else { return };
    let Ok(active_cookie) = conn.intern_atom(false, b"_NET_ACTIVE_WINDOW") else { return };
    let Ok(active_atom) = active_cookie.reply() else { return };

    // Get the currently active window
    let active_wid = conn
        .get_property(false, root, active_atom.atom, AtomEnum::WINDOW, 0, 1)
        .ok()
        .and_then(|c| c.reply().ok())
        .and_then(|r| r.value32().and_then(|mut v| v.next()))
        .unwrap_or(0);

    // Read its _NET_WM_USER_TIME
    let their_time = if active_wid != 0 {
        conn.get_property(false, active_wid, time_atom.atom, AtomEnum::CARDINAL, 0, 1)
            .ok()
            .and_then(|c| c.reply().ok())
            .and_then(|r| r.value32().and_then(|mut v| v.next()))
            .unwrap_or(1)
    } else {
        1
    };

    // Set our timestamp to theirs + 1
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
    let Ok(cookie) = conn.intern_atom(false, b"_NET_ACTIVE_WINDOW") else { return false };
    let Ok(atom) = cookie.reply() else { return false };

    let event = ClientMessageEvent::new(32, wid, atom.atom, [2u32, 0, 0, 0, 0]);
    let mask = EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY;
    let _ = conn.send_event(false, root, mask, event);
    let _ = conn.flush();
    true
}
