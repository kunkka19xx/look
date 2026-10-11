//! Fonts fontconfig knows and gpui's text system does not.
//!
//! cosmic-text reads fontconfig's config files itself and never consults
//! `XDG_DATA_DIRS`, which is how fontconfig reaches a Nix profile's fonts (and
//! Flatpak's). So a family the user set in the config may be invisible to the
//! renderer while `fc-match` finds it fine. Asking `fc-list` for that family's
//! files and feeding them in closes the gap for exactly the fonts asked for.

#[cfg(target_os = "linux")]
use std::collections::HashSet;
#[cfg(target_os = "linux")]
use std::sync::Mutex;

use gpui::App;

#[cfg(target_os = "linux")]
static LOADED: Mutex<Option<HashSet<String>>> = Mutex::new(None);

/// Make `family` renderable if fontconfig has it. A no-op for the platform
/// default, which the text system already found, and after the first call per
/// family.
#[cfg(target_os = "linux")]
pub fn ensure_family(cx: &App, family: &str) {
    use linows_backend::platform::linux::host_command;

    if family == crate::theme::PLATFORM_FONT {
        return;
    }
    let mut loaded = LOADED.lock().unwrap_or_else(|p| p.into_inner());
    if !loaded
        .get_or_insert_with(HashSet::new)
        .insert(family.to_string())
    {
        return;
    }
    let Ok(output) = host_command("fc-list").args([family, "file"]).output() else {
        return;
    };
    let fonts: Vec<_> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| std::fs::read(line.trim().trim_end_matches(':')).ok())
        .map(std::borrow::Cow::Owned)
        .collect();
    if fonts.is_empty() {
        return;
    }
    if let Err(err) = cx.text_system().add_fonts(fonts) {
        eprintln!("[fonts] loading {family:?}: {err}");
    }
}

#[cfg(not(target_os = "linux"))]
pub fn ensure_family(_cx: &App, _family: &str) {}
