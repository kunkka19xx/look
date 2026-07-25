//! Keep-awake power inhibitor via systemd-logind over the system bus. Action id:
//! `"keepawake"`.
//!
//! Mirrors the macOS KeepAwakeControl (a held `ProcessInfo` activity). logind's
//! `Inhibit` hands back a file descriptor and the lock holds only while that fd
//! stays open, so this parks it in a process-wide slot for as long as keep-awake
//! is on and drops it (closing the fd) to release. State is in-memory: it resets
//! to off if Look restarts, matching macOS.
//!
//! `what = "idle"` blocks only idle-triggered screen blank and auto-suspend, not
//! a manual or lid suspend, which is the "don't doze while I'm working" behaviour
//! the tile promises.

use crate::platform::linux::dbus;
use crate::qactions::{ActionIntent, ActionOutcome, ActionState, SystemControl};
use std::sync::Mutex;
use zbus::zvariant::OwnedFd;

const LOGIN1_DEST: &str = "org.freedesktop.login1";
const LOGIN1_PATH: &str = "/org/freedesktop/login1";
const MANAGER_IFACE: &str = "org.freedesktop.login1.Manager";

/// The held inhibitor fd, or `None` when keep-awake is off. Holding the fd is
/// what keeps the lock active; dropping it releases it.
static LOCK: Mutex<Option<OwnedFd>> = Mutex::new(None);

/// Toggles an idle inhibitor so the display and system do not sleep while on.
/// Action id: `"keepawake"`.
pub struct KeepAwakeControl;

impl SystemControl for KeepAwakeControl {
    fn state(&self) -> ActionState {
        if held() {
            ActionState::On
        } else {
            ActionState::Off
        }
    }

    fn apply(&self, intent: ActionIntent) -> ActionOutcome {
        // Hold one guard across check-and-act so a rapid double-press can't both
        // read "off" and both acquire, silently dropping the first inhibitor fd.
        let mut slot = match LOCK.lock() {
            Ok(guard) => guard,
            Err(_) => {
                return ActionOutcome::Failed {
                    message: "Keep Awake lock poisoned".to_string(),
                };
            }
        };
        let held = slot.is_some();
        let target = match intent {
            ActionIntent::Toggle => !held,
            ActionIntent::SetOn(on) => on,
            ActionIntent::Run => {
                return ActionOutcome::Failed {
                    message: "Keep Awake has no run action".to_string(),
                };
            }
        };
        if target == held {
            return ActionOutcome::Ok {
                banner: Some(banner(target)),
            };
        }
        if target {
            match acquire() {
                Ok(fd) => {
                    *slot = Some(fd);
                    ActionOutcome::Ok {
                        banner: Some(banner(true)),
                    }
                }
                Err(message) => ActionOutcome::Failed { message },
            }
        } else {
            // Dropping the fd closes it, which releases the logind lock.
            slot.take();
            ActionOutcome::Ok {
                banner: Some(banner(false)),
            }
        }
    }
}

fn held() -> bool {
    LOCK.lock().map(|slot| slot.is_some()).unwrap_or(false)
}

fn banner(on: bool) -> String {
    format!("Keep Awake {}", if on { "on" } else { "off" })
}

/// Take an `idle` inhibitor lock from logind. The returned fd must stay open for
/// the lock to hold.
fn acquire() -> Result<OwnedFd, String> {
    let conn = dbus::system().ok_or_else(|| "Requires systemd-logind".to_string())?;
    dbus::runtime()
        .block_on(async {
            zbus::Proxy::new(conn, LOGIN1_DEST, LOGIN1_PATH, MANAGER_IFACE)
                .await?
                .call("Inhibit", &("idle", "Look", "Keep Awake", "block"))
                .await
        })
        .map_err(|_: zbus::Error| "Could not hold a keep-awake lock".to_string())
}
