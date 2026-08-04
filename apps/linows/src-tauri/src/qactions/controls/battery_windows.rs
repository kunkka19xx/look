//! Battery info control (read-only) for Windows. Action id: `"battery"`.
//! Windows peer of `battery.rs`; reads `GetSystemPowerStatus` (as
//! `platform/windows/sysinfo.rs` does) and reports `Unavailable` with no battery.

use crate::qactions::{ActionIntent, ActionOutcome, ActionState, InfoValue, SystemControl};
use std::collections::HashMap;
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};

const NO_BATTERY: &str = "No battery";
const CHARGING_INFO_KEY: &str = "charging";

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

    fn info(&self, keys: &[String]) -> HashMap<String, InfoValue> {
        if !keys.iter().any(|k| k == CHARGING_INFO_KEY) {
            return HashMap::new();
        }
        HashMap::from([(
            CHARGING_INFO_KEY.to_string(),
            InfoValue::Text {
                text: if charging_flag() {
                    "charging".to_string()
                } else {
                    "idle".to_string()
                },
            },
        )])
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

/// Whether the battery is charging: the `BatteryFlag` AC/charging bit (0x8).
fn charging_flag() -> bool {
    let mut status = SYSTEM_POWER_STATUS::default();
    if unsafe { GetSystemPowerStatus(&mut status) }.is_err() {
        return false;
    }
    if status.BatteryFlag == 255 || status.BatteryFlag & 128 == 128 {
        return false;
    }
    status.BatteryFlag & 8 == 8
}
