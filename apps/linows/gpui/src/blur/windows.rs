//! No frost on Windows. The DWM backdrop blurs the whole window, and the only
//! way to fit it to the cards is `SetWindowRgn`, whose binary edges leave the
//! rounded corners stair-stepped. Clear glass keeps gpui's anti-aliasing.

use gpui::Window;

use super::BlurRect;

pub fn attach_window(_: &Window) {}

pub fn detach() {}

pub fn is_supported() -> bool {
    false
}

pub fn set_region(_: Vec<BlurRect>) {}
