//! Screensaver / lock activation over the session bus. Action id: `"screensaver"`.
//!
//! The macOS tile starts the resting-image ScreenSaverEngine. Linux has no single
//! screensaver process, but the freedesktop `org.freedesktop.ScreenSaver` service
//! (implemented by GNOME, KDE, Cinnamon, XFCE, ...) exposes `SetActive`, which
//! blanks and, per the desktop's own lock setting, locks the session. That is the
//! closest honest equivalent, so this drives it, falling back to `Lock` for
//! implementations that only expose locking. Button-only.
//!
//! wlroots compositors (sway, Hyprland) run no such service; they lock via the
//! user's own `swayidle` / `swaylock`, which we cannot assume or invoke. So the
//! tile reports `Unavailable` there (no D-Bus name owner) and renders inert
//! rather than erroring on every press, matching how System Settings is gated.

use crate::platform::linux::dbus;
use crate::qactions::{ActionIntent, ActionOutcome, ActionState, SystemControl};

const DBUS_DEST: &str = "org.freedesktop.DBus";
const DBUS_PATH: &str = "/org/freedesktop/DBus";
const SS_DEST: &str = "org.freedesktop.ScreenSaver";
const SS_PATH: &str = "/org/freedesktop/ScreenSaver";
const SS_IFACE: &str = "org.freedesktop.ScreenSaver";
const NO_SERVICE: &str = "No screensaver on this session";

/// Starts the screensaver / lock. Button-only, no readable state. Action id:
/// `"screensaver"`.
pub struct ScreensaverControl;

impl SystemControl for ScreensaverControl {
    /// A button carries no on/off value, but the launchpad only renders it wired
    /// when the read is not `Unavailable`. So gate on whether a screensaver
    /// service actually owns the bus name: an empty value where it does, else
    /// unavailable so the tile stays inert on WMs without one.
    fn state(&self) -> ActionState {
        if available() {
            ActionState::Value {
                value: String::new(),
            }
        } else {
            ActionState::Unavailable {
                reason: NO_SERVICE.to_string(),
            }
        }
    }

    fn apply(&self, intent: ActionIntent) -> ActionOutcome {
        if intent != ActionIntent::Run {
            return ActionOutcome::Failed {
                message: "Screensaver has no toggle".to_string(),
            };
        }
        if set_active() {
            ActionOutcome::Ok {
                banner: Some("Screensaver".to_string()),
            }
        } else {
            ActionOutcome::Failed {
                message: "Could not start screensaver".to_string(),
            }
        }
    }
}

/// Whether a freedesktop screensaver service owns the bus name. Side-effect
/// free, so it is safe to call from `state()` on every panel read.
fn available() -> bool {
    let Some(conn) = dbus::session() else {
        return false;
    };
    dbus::runtime().block_on(async {
        conn.call_method(
            Some(DBUS_DEST),
            DBUS_PATH,
            Some(DBUS_DEST),
            "NameHasOwner",
            &SS_DEST,
        )
        .await
        .and_then(|reply| reply.body().deserialize::<bool>())
        .unwrap_or(false)
    })
}

fn set_active() -> bool {
    let Some(conn) = dbus::session() else {
        return false;
    };
    dbus::runtime().block_on(async {
        let Ok(proxy) = zbus::Proxy::new(conn, SS_DEST, SS_PATH, SS_IFACE).await else {
            return false;
        };
        if proxy.call::<_, _, bool>("SetActive", &true).await.is_ok() {
            return true;
        }
        proxy.call::<_, _, ()>("Lock", &()).await.is_ok()
    })
}
