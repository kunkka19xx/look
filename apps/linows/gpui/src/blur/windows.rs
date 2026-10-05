//! The DWM system backdrop blurs the whole window, so the window is clipped
//! to the cards with `SetWindowRgn`: the gaps are outside the window and stay
//! sharp. Regions are binary, so the rounded staircase edges are not
//! anti-aliased.

use std::cell::{Cell, RefCell};

use gpui::Window;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{
    DWMSBT_TRANSIENTWINDOW, DWMWA_SYSTEMBACKDROP_TYPE, DwmExtendFrameIntoClientArea,
    DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    CombineRgn, CreateRectRgn, DeleteObject, HRGN, RGN_OR, SetWindowRgn,
};
use windows::Win32::UI::Controls::MARGINS;

use crate::host;

use super::BlurRect;

thread_local! {
    static TARGET: Cell<Option<(HWND, f32)>> = const { Cell::new(None) };
    static LAST: RefCell<Vec<BlurRect>> = const { RefCell::new(Vec::new()) };
}

pub fn attach_window(window: &Window) {
    let hwnd = host::hwnd(window);
    if let Some(hwnd) = hwnd {
        let backdrop = DWMSBT_TRANSIENTWINDOW;
        let margins = MARGINS {
            cxLeftWidth: -1,
            cxRightWidth: -1,
            cyTopHeight: -1,
            cyBottomHeight: -1,
        };
        unsafe {
            let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_SYSTEMBACKDROP_TYPE,
                (&raw const backdrop).cast(),
                size_of_val(&backdrop) as u32,
            );
        }
    }
    TARGET.set(hwnd.map(|hwnd| (hwnd, window.scale_factor())));
    LAST.take();
}

pub fn detach() {
    TARGET.set(None);
}

/// The DWM backdrop is always available to ask for; whether it draws is the
/// system's call (a VM without D3D shows a flat tint).
pub fn is_supported() -> bool {
    TARGET.get().is_some()
}

/// Unchanged regions are skipped, so this is cheap to call every frame.
pub fn set_region(rects: Vec<BlurRect>) {
    let Some((hwnd, scale)) = TARGET.get() else {
        return;
    };
    if rects.is_empty() || LAST.with_borrow(|last| *last == rects) {
        return;
    }
    let px = |v: i32| (v as f32 * scale).round() as i32;
    unsafe {
        let region = CreateRectRgn(0, 0, 0, 0);
        for r in &rects {
            let part: HRGN = CreateRectRgn(
                px(r.x),
                px(r.y),
                px(r.x + r.width as i32),
                px(r.y + r.height as i32),
            );
            CombineRgn(Some(region), Some(region), Some(part), RGN_OR);
            let _ = DeleteObject(part.into());
        }
        // The system owns the region from here on.
        SetWindowRgn(hwnd, Some(region), true);
    }
    LAST.set(rects);
}
