//! Linux window-manager / compositor detection.

pub(crate) fn detect_compositor() -> Option<String> {
    if std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok() {
        return Some("hyprland".into());
    }
    if std::env::var("SWAYSOCK").is_ok() {
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
/// hidden/unmapped window is ignored — the WM applies its own placement on map.
pub fn is_tiling_wm() -> bool {
    std::env::var("I3SOCK").is_ok()
        || std::env::var("SWAYSOCK").is_ok()
        || std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok()
}
