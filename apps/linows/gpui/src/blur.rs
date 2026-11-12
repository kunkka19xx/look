//! Behind-window blur regions. The rectangles and the Wayland protocols are
//! the backend's; this is only how gpui's window hands them its surface. On
//! Windows the cards go to a composition layer, see blur/windows.rs.

/// Whether a surface may fade in. The frost is on or off per region, never
/// partial, so a card fading in over it either sits on a bare frosted patch
/// or shows the sharp desktop until the frost pops in (swayfx frosts past an
/// alpha threshold, the same pop). Under frost the surfaces arrive by motion
/// alone, at full opacity.
pub fn surfaces_fade(frosted: bool) -> bool {
    !frosted
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
pub use backend::{Patch, attach_window, detach, is_supported, mark, set_region};

#[cfg(target_os = "linux")]
mod backend {
    use std::sync::Mutex;

    use gpui::{Bounds, Pixels, Window};
    use linows_backend::platform::BlurRect;
    use linows_backend::platform::linux::blur_wayland;
    use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};

    /// Wayland takes a card as the rectangles that fill its rounded shape.
    pub type Patch = BlurRect;

    pub fn mark(frame: &mut Vec<Patch>, card: &Bounds<Pixels>, radius: f32) {
        frame.extend(BlurRect::rounded(
            f32::from(card.origin.x).round() as i32,
            f32::from(card.origin.y).round() as i32,
            f32::from(card.size.width).round() as u32,
            f32::from(card.size.height).round() as u32,
            radius,
        ));
    }

    /// The last region sent, so a prepaint that moved nothing costs nothing.
    static LAST: Mutex<Vec<Patch>> = Mutex::new(Vec::new());

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

    pub fn set_region(rects: Vec<Patch>) {
        let mut last = LAST.lock().unwrap_or_else(|p| p.into_inner());
        if *last == rects {
            return;
        }
        blur_wayland::set_region(&rects);
        *last = rects;
    }
}

#[cfg(target_os = "linux")]
pub use backend::{Patch, attach_window, detach, is_supported, mark, set_region};
