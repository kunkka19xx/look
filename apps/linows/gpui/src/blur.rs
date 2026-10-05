//! Behind-window blur regions. The rectangles and the Wayland protocols are
//! the backend's; this is only how gpui's window hands them its surface. On
//! Windows the region clips the window instead, see blur/windows.rs.

pub use linows_backend::platform::BlurRect;

#[cfg(windows)]
#[path = "blur/windows.rs"]
mod backend;

#[cfg(windows)]
pub use backend::{attach_window, detach, is_supported, set_region};

#[cfg(target_os = "linux")]
mod backend {
    use std::sync::Mutex;

    use gpui::Window;
    use linows_backend::platform::linux::blur_wayland;
    use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};

    use super::BlurRect;

    /// The last region sent, so a prepaint that moved nothing costs nothing.
    static LAST: Mutex<Vec<BlurRect>> = Mutex::new(Vec::new());

    /// Bind the managers on first call and follow this window's surface. Call
    /// on every open, from the main thread. gpui makes a new `wl_surface` per
    /// open, so the region is replayed on each attach.
    pub fn attach_window(window: &Window) {
        let Ok(display) = window.display_handle() else {
            return;
        };
        let RawDisplayHandle::Wayland(display) = display.as_raw() else {
            return;
        };
        blur_wayland::init(display.display.as_ptr());
        if !blur_wayland::is_supported() {
            return;
        }
        // gpui's inherent `window_handle` shadows the trait method.
        let Ok(handle) = HasWindowHandle::window_handle(window) else {
            return;
        };
        let RawWindowHandle::Wayland(handle) = handle.as_raw() else {
            return;
        };
        LAST.lock().unwrap_or_else(|p| p.into_inner()).clear();
        blur_wayland::attach(handle.surface.as_ptr());
    }

    pub fn detach() {
        blur_wayland::detach();
    }

    pub fn is_supported() -> bool {
        blur_wayland::is_supported()
    }

    pub fn set_region(rects: Vec<BlurRect>) {
        let mut last = LAST.lock().unwrap_or_else(|p| p.into_inner());
        if *last == rects {
            return;
        }
        blur_wayland::set_region(&rects);
        *last = rects;
    }
}

#[cfg(target_os = "linux")]
pub use backend::{attach_window, detach, is_supported, set_region};
