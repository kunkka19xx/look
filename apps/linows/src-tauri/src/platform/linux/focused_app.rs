//! Which app the user is in: a copied image is named after where it came from,
//! and a paste waits for the window it is aimed at. Asked at those two moments
//! only, never per poll.

/// Look holds focus whenever the user is looking at it, so it is never the
/// answer.
pub const SELF_APP_ID: &str = "lookapp";

/// `None` when the session offers no way to ask (KDE Wayland, which speaks
/// neither X11 nor the wlroots toplevel protocol).
pub fn focused_app_name() -> Option<String> {
    let app_id = focused_app_id()?;
    if app_id.eq_ignore_ascii_case(SELF_APP_ID) {
        return None;
    }
    Some(super::process::app_display_name(&app_id))
}

pub fn focused_app_id() -> Option<String> {
    if super::transparency::is_wayland() {
        // The wlroots family reports which toplevel it has activated, GNOME
        // answers through the Look extension, KDE Wayland not at all.
        return super::wlr_focus::focused_app_id().or_else(super::gnome_ext::focused_app_id);
    }
    super::window_focus::focused_wm_class()
}
