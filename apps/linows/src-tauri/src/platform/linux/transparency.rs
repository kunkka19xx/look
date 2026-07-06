//! Linux-specific compositor and transparency detection.
//!
//! Centralises the logic so that both `main.rs` (startup) and `platform.rs`
//! (runtime query) share the same detection code.

/// Returns `true` when the current Linux session has a compositor that can
/// render transparent windows (RGBA visuals / alpha blending).
#[cfg(target_os = "linux")]
pub fn has_compositor() -> bool {
    if is_wayland() {
        return true;
    }
    // Desktop environments that always ship a compositor on X11
    if desktop_has_builtin_compositor() {
        return true;
    }
    // X11 with a standalone compositor (picom, compton, etc.)
    x11_has_standalone_compositor()
}

/// Wayland detection - checks both `WAYLAND_DISPLAY` (more reliable on NixOS
/// and non-systemd setups) and `XDG_SESSION_TYPE`.
#[cfg(target_os = "linux")]
pub fn is_wayland() -> bool {
    if std::env::var("WAYLAND_DISPLAY").is_ok_and(|v| !v.is_empty()) {
        return true;
    }
    std::env::var("XDG_SESSION_TYPE")
        .map(|v| v == "wayland")
        .unwrap_or(false)
}

/// Whether the GTK window is an X11 window. True on X11 sessions, and on
/// Wayland sessions when GDK_BACKEND forces x11 (the AppImage's AppRun does;
/// the window is then XWayland). Focus/activation must follow the window
/// backend, while shortcut registration follows the session (`is_wayland`).
#[cfg(target_os = "linux")]
pub fn window_is_x11() -> bool {
    if !is_wayland() {
        return true;
    }
    // GDK_BACKEND is a preference list; on a Wayland session only a leading
    // "x11" forces the X11 backend.
    std::env::var("GDK_BACKEND").is_ok_and(|v| v.trim().starts_with("x11"))
}

/// Desktop environments that embed a compositor (GNOME→mutter, KDE→kwin,
/// Cinnamon→muffin, Deepin→kwin, Budgie→mutter, COSMIC, Pantheon→gala).
/// Detected via `XDG_CURRENT_DESKTOP` which is set reliably on all distros.
#[cfg(target_os = "linux")]
fn desktop_has_builtin_compositor() -> bool {
    const COMPOSITED_DESKTOPS: &[&str] = &[
        "GNOME", "KDE", "Cinnamon", "Deepin", "Budgie", "COSMIC", "Pantheon",
    ];

    let desktop = match std::env::var("XDG_CURRENT_DESKTOP") {
        Ok(v) => v,
        Err(_) => return false,
    };

    // XDG_CURRENT_DESKTOP can be colon-separated (e.g. "ubuntu:GNOME")
    desktop.split(':').any(|segment| {
        let s = segment.trim();
        COMPOSITED_DESKTOPS
            .iter()
            .any(|&d| s.eq_ignore_ascii_case(d))
    })
}

/// Scan for standalone X11 compositors via `pgrep -f` (matches against the
/// full command line). This works on NixOS where process names are wrapped
/// and `pgrep -x` fails to match.
#[cfg(target_os = "linux")]
fn x11_has_standalone_compositor() -> bool {
    const COMPOSITORS: &[&str] = &["picom", "compton", "compiz", "xfwm4", "marco"];

    for name in COMPOSITORS {
        let ok = super::host_command("pgrep")
            .args(["-f", name])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if ok {
            return true;
        }
    }
    false
}
