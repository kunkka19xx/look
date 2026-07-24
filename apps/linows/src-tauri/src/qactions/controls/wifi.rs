//! Wi-Fi radio toggle via NetworkManager over the system bus. Action id: `"wifi"`.
//!
//! Mirrors the macOS WiFiControl (CoreWLAN power on/off). NetworkManager owns the
//! Linux radio switch: its `WirelessEnabled` property is the soft toggle the
//! desktop's own Wi-Fi switch drives, so writing it here matches what the OS UI
//! does. Sessions without NetworkManager report `Unavailable`, as do machines
//! whose Wi-Fi is hard-blocked (rfkill) or absent.
//!
//! Talks D-Bus directly rather than shelling out to `nmcli`: the CLI is just a
//! client of the same API, and an in-process call avoids the AppImage's
//! LD_LIBRARY_PATH scrubbing (see `bluetooth.rs`).

use crate::platform::linux::dbus;
use crate::qactions::{ActionIntent, ActionOutcome, ActionState, SystemControl};
use std::time::{Duration, Instant};

const NM_DEST: &str = "org.freedesktop.NetworkManager";
const NM_PATH: &str = "/org/freedesktop/NetworkManager";
const NM_IFACE: &str = "org.freedesktop.NetworkManager";

const NO_SERVICE: &str = "NetworkManager not running";
const NO_HARDWARE: &str = "No Wi-Fi hardware";

/// Mirrors `bluetooth.rs`: NetworkManager applies the radio change
/// asynchronously, so poll until the property reflects the target before the
/// panel re-reads it.
const SETTLE_TIMEOUT: Duration = Duration::from_millis(1500);
const POLL_INTERVAL: Duration = Duration::from_millis(80);

/// Toggles and reports Linux Wi-Fi radio power. Action id: `"wifi"`.
pub struct WifiControl;

impl SystemControl for WifiControl {
    fn state(&self) -> ActionState {
        match read_enabled() {
            Ok((_, false)) => ActionState::Unavailable {
                reason: NO_HARDWARE.to_string(),
            },
            Ok((true, true)) => ActionState::On,
            Ok((false, true)) => ActionState::Off,
            Err(reason) => ActionState::Unavailable { reason },
        }
    }

    fn apply(&self, intent: ActionIntent) -> ActionOutcome {
        // Re-read right before acting so a change made elsewhere between show and
        // press is not clobbered.
        let (enabled, hardware) = match read_enabled() {
            Ok(v) => v,
            Err(message) => return ActionOutcome::Failed { message },
        };
        if !hardware {
            return ActionOutcome::Failed {
                message: NO_HARDWARE.to_string(),
            };
        }
        let target = match intent {
            ActionIntent::Toggle => !enabled,
            ActionIntent::SetOn(on) => on,
            ActionIntent::Run => {
                return ActionOutcome::Failed {
                    message: "Wi-Fi has no run action".to_string(),
                };
            }
        };

        if let Err(message) = set_enabled(target) {
            return ActionOutcome::Failed { message };
        }
        if !wait_for_enabled(target) {
            return ActionOutcome::Failed {
                message: format!("Could not turn Wi-Fi {}", on_off(target)),
            };
        }
        ActionOutcome::Ok {
            banner: Some(format!("Wi-Fi {}", on_off(target))),
        }
    }
}

fn on_off(on: bool) -> &'static str {
    if on { "on" } else { "off" }
}

/// `(WirelessEnabled, WirelessHardwareEnabled)`, or a human reason when
/// NetworkManager is unreachable. The hardware flag is false when the radio is
/// hard-blocked (rfkill) or the machine has no Wi-Fi at all.
fn read_enabled() -> Result<(bool, bool), String> {
    let conn = dbus::system().ok_or_else(|| NO_SERVICE.to_string())?;
    dbus::runtime().block_on(async {
        let proxy = zbus::Proxy::new(conn, NM_DEST, NM_PATH, NM_IFACE)
            .await
            .map_err(|_| NO_SERVICE.to_string())?;
        let enabled: bool = proxy
            .get_property("WirelessEnabled")
            .await
            .map_err(|_| NO_SERVICE.to_string())?;
        let hardware: bool = proxy
            .get_property("WirelessHardwareEnabled")
            .await
            .unwrap_or(true);
        Ok((enabled, hardware))
    })
}

fn set_enabled(on: bool) -> Result<(), String> {
    let conn = dbus::system().ok_or_else(|| NO_SERVICE.to_string())?;
    dbus::runtime()
        .block_on(async {
            zbus::Proxy::new(conn, NM_DEST, NM_PATH, NM_IFACE)
                .await?
                .set_property("WirelessEnabled", on)
                .await
        })
        .map_err(|_| format!("Could not turn Wi-Fi {}", on_off(on)))
}

/// Polls `WirelessEnabled` until it reaches `target` or the settle timeout. Runs
/// on the blocking pool, so plain sleeps are fine.
fn wait_for_enabled(target: bool) -> bool {
    let deadline = Instant::now() + SETTLE_TIMEOUT;
    loop {
        if matches!(read_enabled(), Ok((on, _)) if on == target) {
            return true;
        }
        if Instant::now() >= deadline {
            return matches!(read_enabled(), Ok((on, _)) if on == target);
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}
