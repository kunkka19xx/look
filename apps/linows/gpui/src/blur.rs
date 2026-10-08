//! Behind-window blur regions. The rectangles and the Wayland protocols are
//! the backend's; this is only how gpui's window hands them its surface. On
//! Windows the region clips the window instead, see blur/windows.rs.

pub use linows_backend::platform::BlurRect;

/// Whether a surface may fade in. swayfx frosts a pixel only once its
/// alpha passes a threshold, so a card fading in from nothing shows dark
/// over the sharp desktop and then the frost pops in; under its frost the
/// surfaces arrive by motion alone, at full opacity.
pub fn surfaces_fade(frosted: bool) -> bool {
    #[cfg(target_os = "linux")]
    {
        !(frosted && linows_backend::platform::linux::wm::is_swayfx())
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = frosted;
        true
    }
}

/// The compositor frosts behind the window one way or another: through a
/// blur protocol, or swayfx's `layer_effects` on the whole surface with
/// transparent pixels skipped, where any shadow alpha would be frosted.
pub fn can_frost() -> bool {
    #[cfg(target_os = "linux")]
    {
        is_supported() || linows_backend::platform::linux::wm::is_swayfx()
    }
    #[cfg(not(target_os = "linux"))]
    {
        is_supported()
    }
}

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
