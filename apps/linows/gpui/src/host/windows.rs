//! Win32 host: a topmost tool popup centred on the primary monitor, commands
//! over loopback TCP since std has no named pipe server.

use std::net::TcpListener;

use gpui::{App, Bounds, Pixels, Size, Window, WindowBackgroundAppearance, WindowKind};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::UI::WindowsAndMessaging::{
    GWL_STYLE, GetClientRect, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOZORDER, SetWindowLongPtrW,
    SetWindowPos, WS_CLIPSIBLINGS, WS_POPUP, WS_VISIBLE,
};

/// The DWM system backdrop does the blur, see blur/windows.rs.
pub const BACKGROUND: WindowBackgroundAppearance = WindowBackgroundAppearance::Transparent;

/// Any client that writes the command then closes, e.g. a `TcpClient` in PowerShell.
const PORT: u16 = 47811;

pub fn bind() -> std::io::Result<TcpListener> {
    TcpListener::bind(("127.0.0.1", PORT))
}

pub fn bounds(size: Size<Pixels>, cx: &App) -> Bounds<Pixels> {
    Bounds::centered(None, size, cx)
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
/// shrinks to its old client rect so the content does not move.
pub fn decorate(window: &Window) {
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
}
