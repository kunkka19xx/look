//! System appearance toggle (dark <-> light). Action id: `"theme"`.
//!
//! Mirrors the macOS Theme tile, which flips the OS appearance rather than the
//! app's own preset. On Linux that is the freedesktop `color-scheme` setting,
//! written through `gsettings` (the same host tool the GNOME integration
//! already drives). Non-GNOME sessions without `gsettings` report `Unavailable`.
//!
//! `On` == dark, matching the shared descriptor's Dark/Light labels.

use crate::qactions::{ActionIntent, ActionOutcome, ActionState, SystemControl};

const SCHEMA: &str = "org.gnome.desktop.interface";
const KEY: &str = "color-scheme";
const DARK: &str = "prefer-dark";
const LIGHT: &str = "default";
const UNAVAILABLE: &str = "Requires GNOME (gsettings)";

/// Toggles the desktop light/dark appearance. Action id: `"theme"`.
pub struct ThemeControl;

impl SystemControl for ThemeControl {
    fn state(&self) -> ActionState {
        match read_scheme() {
            Some(scheme) if scheme.contains("dark") => ActionState::On,
            Some(_) => ActionState::Off,
            None => ActionState::Unavailable {
                reason: UNAVAILABLE.to_string(),
            },
        }
    }

    fn apply(&self, intent: ActionIntent) -> ActionOutcome {
        let dark = match intent {
            ActionIntent::Toggle => !matches!(read_scheme(), Some(s) if s.contains("dark")),
            ActionIntent::SetOn(on) => on,
            ActionIntent::Run => {
                return ActionOutcome::Failed {
                    message: "Theme has no run action".to_string(),
                };
            }
        };
        if write_scheme(if dark { DARK } else { LIGHT }) {
            ActionOutcome::Ok {
                banner: Some(if dark { "Dark" } else { "Light" }.to_string()),
            }
        } else {
            ActionOutcome::Failed {
                message: UNAVAILABLE.to_string(),
            }
        }
    }
}

/// The current `color-scheme` value (e.g. `prefer-dark`), or `None` when
/// `gsettings` is missing or the read fails.
fn read_scheme() -> Option<String> {
    let output = crate::platform::linux::host_command("gsettings")
        .args(["get", SCHEMA, KEY])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Write `value` to the `color-scheme` key; returns whether it succeeded.
fn write_scheme(value: &str) -> bool {
    crate::platform::linux::host_command("gsettings")
        .args(["set", SCHEMA, KEY, value])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
