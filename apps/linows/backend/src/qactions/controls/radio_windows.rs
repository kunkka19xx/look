//! Shared WinRT `Windows.Devices.Radios.Radio` plumbing for the Wi-Fi and
//! Bluetooth toggles: lookup, access request, and the settle-poll after a set.

use crate::platform::windows::ensure_mta;
use std::thread;
use std::time::{Duration, Instant};
use windows::Devices::Radios::{Radio, RadioAccessStatus, RadioKind, RadioState};

const SETTLE_TIMEOUT: Duration = Duration::from_millis(1500);
const POLL_INTERVAL: Duration = Duration::from_millis(80);

/// Why a power change didn't take: access blocked, or the set itself failed.
pub enum RadioError {
    Denied,
    Failed,
}

/// The first radio of `kind`, or `None` when the machine has none.
pub fn of_kind(kind: RadioKind) -> Option<Radio> {
    ensure_mta();
    let radios = Radio::GetRadiosAsync().and_then(|op| op.get()).ok()?;
    radios.into_iter().find(|r| r.Kind() == Ok(kind))
}

pub fn is_on(radio: &Radio) -> bool {
    radio.State() == Ok(RadioState::On)
}

/// Request access, then drive the radio to `on`.
pub fn set_powered(radio: &Radio, on: bool) -> Result<(), RadioError> {
    let access = Radio::RequestAccessAsync()
        .and_then(|op| op.get())
        .map_err(|_| RadioError::Denied)?;
    if access != RadioAccessStatus::Allowed {
        return Err(RadioError::Denied);
    }
    let target = if on { RadioState::On } else { RadioState::Off };
    match radio.SetStateAsync(target).and_then(|op| op.get()) {
        Ok(RadioAccessStatus::Allowed) => Ok(()),
        Ok(_) => Err(RadioError::Denied),
        Err(_) => Err(RadioError::Failed),
    }
}

/// Poll until the radio reports `target` or the settle timeout elapses.
pub fn wait_for(radio: &Radio, target: bool) -> bool {
    let deadline = Instant::now() + SETTLE_TIMEOUT;
    loop {
        if is_on(radio) == target {
            return true;
        }
        if Instant::now() >= deadline {
            return is_on(radio) == target;
        }
        thread::sleep(POLL_INTERVAL);
    }
}

pub fn on_off(on: bool) -> &'static str {
    if on { "on" } else { "off" }
}
