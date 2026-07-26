//! Battery info control (read-only) for Windows. Action id: `"battery"`.
//! Windows peer of `battery.rs`; reads `GetSystemPowerStatus` (as
//! `platform/windows/sysinfo.rs` does) and reports `Unavailable` with no battery.

use crate::qactions::{ActionIntent, ActionOutcome, ActionState, SystemControl};
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};

const NO_BATTERY: &str = "No battery";

/// Reports the system battery charge. Action id: `"battery"`.
pub struct BatteryControl;

impl SystemControl for BatteryControl {
    fn state(&self) -> ActionState {
        match read_percent() {
            Some(pct) => ActionState::Value {
                value: format!("{pct}%"),
            },
            None => ActionState::Unavailable {
                reason: NO_BATTERY.to_string(),
            },
        }
    }

    fn apply(&self, _intent: ActionIntent) -> ActionOutcome {
        ActionOutcome::Failed {
            message: "Battery is read-only".to_string(),
        }
    }
}

/// The battery charge percent, or `None` when there is no battery (desktop) or
/// the level is unknown. `BatteryLifePercent == 255` is the "unknown / no
/// battery" sentinel, matching the sysinfo collector.
fn read_percent() -> Option<u8> {
    let mut status = SYSTEM_POWER_STATUS::default();
    unsafe { GetSystemPowerStatus(&mut status) }.ok()?;
    (status.BatteryLifePercent != 255).then_some(status.BatteryLifePercent.min(100))
}
