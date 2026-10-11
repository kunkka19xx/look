//! Win32 host: a topmost tool popup centred on the primary monitor, commands
//! over loopback TCP since std has no named pipe server.

use std::net::TcpListener;

use gpui::{App, Bounds, Pixels, Size, Window, WindowBackgroundAppearance, WindowKind};
use linows_backend::platform::windows::window_focus::bring_to_front;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::UI::WindowsAndMessaging::{
    GWL_STYLE, GetClientRect, HWND_TOPMOST, SW_HIDE, SWP_FRAMECHANGED, SWP_NOACTIVATE,
    SWP_NOZORDER, SWP_SHOWWINDOW, SetWindowLongPtrW, SetWindowPos, ShowWindow, WS_CLIPSIBLINGS,
    WS_POPUP, WS_VISIBLE,
};

/// Per-pixel alpha, so the cards' rounded edges stay anti-aliased.
pub const BACKGROUND: WindowBackgroundAppearance = WindowBackgroundAppearance::Transparent;

/// A hide keeps the window and its swap chain: each new one costs ~100 MB that
/// the D3D runtime keeps after the close, worst under software rendering.
pub const KEEP_WINDOW: bool = true;

pub fn conceal(window: &Window) {
    if let Some(hwnd) = hwnd(window) {
        unsafe {
            let _ = ShowWindow(hwnd, SW_HIDE);
        }
    }
}

/// The kept window back at `bounds`, on top and with the keyboard.
pub fn reshow(window: &Window, bounds: Bounds<Pixels>) {
    let Some(hwnd) = hwnd(window) else {
        return;
    };
    let scale = window.scale_factor();
    let px = |v: Pixels| (f32::from(v) * scale).round() as i32;
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            px(bounds.origin.x),
            px(bounds.origin.y),
            px(bounds.size.width),
            px(bounds.size.height),
            SWP_SHOWWINDOW,
        );
    }
    bring_to_front(hwnd.0 as isize);
}

/// Any client that writes the command then closes, e.g. a `TcpClient` in PowerShell.
const PORT: u16 = 47811;
/// A second instance beside the real one (a pacing probe) answers on its own
/// port so the two do not trade commands.
const PORT_ENV: &str = "LOOK_CONTROL_PORT";

pub fn bind() -> std::io::Result<TcpListener> {
    let port = std::env::var(PORT_ENV)
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(PORT);
    TcpListener::bind(("127.0.0.1", port))
}

pub fn bounds(size: Size<Pixels>, cx: &App) -> Bounds<Pixels> {
    Bounds::centered(None, size, cx)
}

/// The frame sits inside the split window; nothing to fit.
pub fn fitted(_: Size<Pixels>) {}

pub fn fit(_: &mut Window, _: Size<Pixels>, _: &App) {}

/// DWM always composites.
pub fn probe_opaque() {}

pub fn opaque() -> bool {
    false
}

pub fn kind() -> WindowKind {
    WindowKind::PopUp
}

pub fn hwnd(window: &Window) -> Option<HWND> {
    let handle = HasWindowHandle::window_handle(window).ok()?;
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return None;
    };
    Some(HWND(handle.hwnd.get() as _))
}

/// gpui creates the popup with style 0, which Windows turns into a captioned
/// overlapped window: DWM then paints frame, shadow and theme backdrop behind
/// the transparent pixels. A bare WS_POPUP gets none of that; the window
/// shrinks to its old client rect so the content does not move. Then it takes
/// the foreground from whichever app the hotkey was pressed in.
pub fn decorate(window: &Window, _: Bounds<Pixels>) {
    let Some(hwnd) = hwnd(window) else {
        return;
    };
    unsafe {
        let mut client = RECT::default();
        let mut origin = POINT::default();
        if GetClientRect(hwnd, &mut client).is_err() || !ClientToScreen(hwnd, &mut origin).as_bool()
        {
            return;
        }
        SetWindowLongPtrW(
            hwnd,
            GWL_STYLE,
            (WS_POPUP | WS_VISIBLE | WS_CLIPSIBLINGS).0 as isize,
        );
        let _ = SetWindowPos(
            hwnd,
            None,
            origin.x,
            origin.y,
            client.right,
            client.bottom,
            SWP_FRAMECHANGED | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
    bring_to_front(hwnd.0 as isize);
}
