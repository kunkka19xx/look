//! Which app the user was in when something landed on the clipboard: a copied
//! image carries no name, so its row is named after where it came from. Runs
//! only when a new picture is filed, never per poll.

/// Look holds focus whenever the user is looking at it, so it is never the
/// answer.
const SELF_APP_ID: &str = "lookapp";

/// `None` when the session offers no way to ask (GNOME and KDE Wayland, which
/// speak neither X11 nor the wlroots toplevel protocol).
pub fn focused_app_name() -> Option<String> {
    let app_id = focused_app_id()?;
    if app_id.eq_ignore_ascii_case(SELF_APP_ID) {
        return None;
    }
    Some(super::process::app_display_name(&app_id))
}

fn focused_app_id() -> Option<String> {
    if super::transparency::is_wayland() {
        // sway, Hyprland, niri and the rest of the wlroots family all report
        // which toplevel they have activated.
        super::wlr_focus::focused_app_id()
    } else {
        super::window_focus::focused_wm_class()
    }
}
