//! Behind-window blur, requested from the compositor. CSS cannot do this:
//! `backdrop-filter` reaches the page's own layers only, and the desktop
//! behind a transparent window is composited outside the webview.
//!
//! X11 (KWin): `_KDE_NET_WM_BLUR_BEHIND_REGION`, CARDINAL/32 quadruples of
//! `x, y, width, height` in window-local pixels; empty means the whole window.
//! Wayland asks over one of two protocols (`blur_wayland`).

use super::blur_wayland;
use super::transparency::window_is_x11;
use crate::platform::BlurRect;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, PropMode};
use x11rb::wrapper::ConnectionExt as _;

/// Bind the Wayland protocols against the window's surface. Call once at
/// startup, from the main thread; a no-op on X11.
pub fn init(window: &tauri::WebviewWindow) {
    use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};

    if window_is_x11() {
        return;
    }
    let (Ok(display), Ok(surface)) = (window.display_handle(), window.window_handle()) else {
        return;
    };
    let (RawDisplayHandle::Wayland(display), RawWindowHandle::Wayland(surface)) =
        (display.as_raw(), surface.as_raw())
    else {
        return;
    };
    blur_wayland::init(display.display.as_ptr(), surface.surface.as_ptr());
}

/// Whether the compositor takes the request. Drives the frontend's tint math
/// (Blur Opacity needs real frost to thin), so it must never be optimistic.
/// X11 has no negotiation - only KWin reads the atom - so the desktop name is
/// the only signal there.
pub fn is_supported() -> bool {
    if window_is_x11() {
        return is_kde();
    }
    blur_wayland::is_supported()
}

fn is_kde() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP")
        .map(|v| v.to_ascii_lowercase().contains("kde"))
        .unwrap_or(false)
}

/// Ask for `rects` to be blurred; an empty slice clears the request. Silent on
/// failure: a session without the effect is the clear-glass path, not an error.
pub fn set_region(wid: u32, rects: &[BlurRect]) {
    if !window_is_x11() {
        blur_wayland::set_region(rects);
        return;
    }
    if !is_supported() {
        return;
    }
    let Ok((conn, _)) = x11rb::connect(None) else {
        return;
    };
    let Ok(cookie) = conn.intern_atom(false, b"_KDE_NET_WM_BLUR_BEHIND_REGION") else {
        return;
    };
    let Ok(atom) = cookie.reply() else {
        return;
    };

    let mut data: Vec<u32> = Vec::with_capacity(rects.len() * 4);
    for rect in rects {
        // Clamped, not dropped: a surface can sit a pixel outside the window
        // mid-resize, and the wrap-around would blur a stripe across the screen.
        data.push(rect.x.max(0) as u32);
        data.push(rect.y.max(0) as u32);
        data.push(rect.width);
        data.push(rect.height);
    }

    let _ = conn.change_property32(PropMode::REPLACE, wid, atom.atom, AtomEnum::CARDINAL, &data);
    let _ = conn.flush();
}
