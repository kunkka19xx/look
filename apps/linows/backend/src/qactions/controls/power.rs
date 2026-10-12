//! System power actions (restart, shut down) via systemd-logind over the system
//! bus. Action ids: `"restart"`, `"shutdown"`.
//!
//! Mirror the macOS RestartControl / ShutdownControl. logind's `Reboot` /
//! `PowerOff` are the standard entry points; called non-interactively they defer
//! to polkit, which grants an active local session the action without a password
//! on a typical desktop. The launchpad gates both behind an inline confirm before
//! the intent reaches here. Button-only.

use crate::platform::linux::dbus;
use crate::qactions::{ActionIntent, ActionOutcome, ActionState, SystemControl};

const LOGIN1_DEST: &str = "org.freedesktop.login1";
const LOGIN1_PATH: &str = "/org/freedesktop/login1";
const MANAGER_IFACE: &str = "org.freedesktop.login1.Manager";

/// Restarts the machine. Button-only. Action id: `"restart"`.
pub struct RestartControl;

/// Shuts the machine down. Button-only. Action id: `"shutdown"`.
pub struct ShutdownControl;

impl SystemControl for RestartControl {
    fn state(&self) -> ActionState {
        present()
    }

    fn apply(&self, intent: ActionIntent) -> ActionOutcome {
        power(
            intent,
            "Reboot",
            "Restart",
            "Restarting…",
            "Could not restart",
        )
    }
}

impl SystemControl for ShutdownControl {
    fn state(&self) -> ActionState {
        present()
    }

    fn apply(&self, intent: ActionIntent) -> ActionOutcome {
        power(
            intent,
            "PowerOff",
            "Shut Down",
            "Shutting down…",
            "Could not shut down",
        )
    }
}

/// Buttons carry no on/off value; an empty value marks them present so the
/// launchpad renders them wired.
fn present() -> ActionState {
    ActionState::Value {
        value: String::new(),
    }
}

/// Call a logind Manager power method non-interactively, mapping the outcome to
/// the tile's banner. Rejects non-`Run` intents (these are buttons).
fn power(intent: ActionIntent, method: &str, name: &str, ok: &str, fail: &str) -> ActionOutcome {
    if intent != ActionIntent::Run {
        return ActionOutcome::Failed {
            message: format!("{name} has no toggle"),
        };
    }
    if call(method) {
        ActionOutcome::Ok {
            banner: Some(ok.to_string()),
        }
    } else {
        ActionOutcome::Failed {
            message: fail.to_string(),
        }
    }
}

fn call(method: &str) -> bool {
    let Some(conn) = dbus::system() else {
        return false;
    };
    dbus::block_on(async {
        let Ok(proxy) = zbus::Proxy::new(conn, LOGIN1_DEST, LOGIN1_PATH, MANAGER_IFACE).await
        else {
            return false;
        };
        // interactive = false: let polkit decide from the active session rather
        // than pop an agent dialog behind our window.
        proxy.call::<_, _, ()>(method, &false).await.is_ok()
    })
}
