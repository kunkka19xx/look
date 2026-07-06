//! Linux window-manager / compositor detection.

/// Returns true if Sway is actually running (socket exists).
pub fn is_sway() -> bool {
    std::env::var("SWAYSOCK")
        .map(|s| std::path::Path::new(&s).exists())
        .unwrap_or(false)
}

/// Returns true when the session is KDE Plasma (any XDG_CURRENT_DESKTOP
/// segment is "KDE").
pub fn is_kde() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .split(':')
        .any(|s| s.trim().eq_ignore_ascii_case("KDE"))
}

pub(crate) fn detect_compositor() -> Option<String> {
    if std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok() {
        return Some("hyprland".into());
    }
    if is_sway() {
        return Some("sway".into());
    }
    if std::env::var("I3SOCK").is_ok() {
        return Some("i3".into());
    }
    // XDG_CURRENT_DESKTOP can be colon-separated ("ubuntu:GNOME", "pop:GNOME").
    // Prefer a recognised desktop name over distro prefixes.
    const KNOWN: &[&str] = &[
        "gnome", "kde", "cinnamon", "xfce", "lxqt", "mate", "budgie", "deepin", "pantheon",
        "cosmic",
    ];
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    for seg in desktop.split(':') {
        let s = seg.trim().to_ascii_lowercase();
        if KNOWN.iter().any(|&k| k == s) {
            return Some(s);
        }
    }
    // Fallback: first non-empty segment.
    desktop.split(':').find_map(|s| {
        let t = s.trim();
        (!t.is_empty()).then(|| t.to_ascii_lowercase())
    })
}

/// Returns true for tiling WMs (i3, sway, Hyprland) where `set_position` on a
/// hidden/unmapped window is ignored - the WM applies its own placement on map.
pub fn is_tiling_wm() -> bool {
    std::env::var("I3SOCK").is_ok()
        || is_sway()
        || std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok()
}
