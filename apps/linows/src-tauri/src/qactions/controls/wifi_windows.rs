//! Windows Wi-Fi radio toggle. Action id: `"wifi"`.
//!
//! Windows peer of `wifi.rs`. Uses `Windows.Devices.Radios.Radio` (kind WiFi),
//! the same WinRT surface `bluetooth_windows.rs` drives for Bluetooth, so the
//! settle-poll shape is identical. Machines with no Wi-Fi radio report
//! `Unavailable`; a radio blocked by Windows privacy settings surfaces on apply.

use crate::qactions::{ActionIntent, ActionOutcome, ActionState, SystemControl};
use std::sync::Once;
use std::thread;
use std::time::{Duration, Instant};
use windows::Devices::Radios::{Radio, RadioAccessStatus, RadioKind, RadioState};
use windows::Win32::System::Com::CoIncrementMTAUsage;

const NO_ADAPTER: &str = "No Wi-Fi hardware";
const ACCESS_DENIED: &str = "Wi-Fi access is blocked by Windows";

const SETTLE_TIMEOUT: Duration = Duration::from_millis(1500);
const POLL_INTERVAL: Duration = Duration::from_millis(80);

/// Toggles and reports Windows Wi-Fi radio power. Action id: `"wifi"`.
pub struct WifiControl;

impl SystemControl for WifiControl {
    fn state(&self) -> ActionState {
        match wifi_radio() {
            Ok(radio) if radio_is_on(&radio) => ActionState::On,
            Ok(_) => ActionState::Off,
            Err(reason) => ActionState::Unavailable { reason },
        }
    }

    fn apply(&self, intent: ActionIntent) -> ActionOutcome {
        let radio = match wifi_radio() {
            Ok(radio) => radio,
            Err(reason) => return ActionOutcome::Failed { message: reason },
        };
        let target = match intent {
            ActionIntent::Toggle => !radio_is_on(&radio),
            ActionIntent::SetOn(on) => on,
            ActionIntent::Run => {
                return ActionOutcome::Failed {
                    message: "Wi-Fi has no run action".to_string(),
                };
            }
        };

        if let Err(message) = set_powered(&radio, target) {
            return ActionOutcome::Failed { message };
        }
        if !wait_for_power_state(&radio, target) {
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

/// Keep the process in an MTA so WinRT calls on pooled blocking threads work.
fn ensure_mta() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let _ = unsafe { CoIncrementMTAUsage() };
    });
}

fn wifi_radio() -> Result<Radio, String> {
    ensure_mta();
    let radios = Radio::GetRadiosAsync()
        .and_then(|op| op.get())
        .map_err(|_| NO_ADAPTER.to_string())?;
    for radio in radios {
        if radio.Kind() == Ok(RadioKind::WiFi) {
            return Ok(radio);
        }
    }
    Err(NO_ADAPTER.to_string())
}

fn radio_is_on(radio: &Radio) -> bool {
    radio.State() == Ok(RadioState::On)
}

fn set_powered(radio: &Radio, on: bool) -> Result<(), String> {
    let access = Radio::RequestAccessAsync()
        .and_then(|op| op.get())
        .map_err(|_| ACCESS_DENIED.to_string())?;
    if access != RadioAccessStatus::Allowed {
        return Err(ACCESS_DENIED.to_string());
    }
    let target = if on { RadioState::On } else { RadioState::Off };
    match radio.SetStateAsync(target).and_then(|op| op.get()) {
        Ok(RadioAccessStatus::Allowed) => Ok(()),
        Ok(_) => Err(ACCESS_DENIED.to_string()),
        Err(_) => Err(format!("Could not turn Wi-Fi {}", on_off(on))),
    }
}

/// Poll the radio until it reports `target` or the settle timeout elapses.
fn wait_for_power_state(radio: &Radio, target: bool) -> bool {
    let deadline = Instant::now() + SETTLE_TIMEOUT;
    loop {
        if radio_is_on(radio) == target {
            return true;
        }
        if Instant::now() >= deadline {
            return radio_is_on(radio) == target;
        }
        thread::sleep(POLL_INTERVAL);
    }
}
