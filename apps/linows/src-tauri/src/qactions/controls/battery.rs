//! Battery info control (read-only). Action id: `"battery"`.
//!
//! The launchpad Battery tile is an info tile: it shows a live charge percent
//! and never toggles. Reads sysfs directly (`/sys/class/power_supply`) rather
//! than shelling out, matching the in-process policy of the reference adapter.
//! Desktops without a battery report `Unavailable`, which the shell renders as
//! a dimmed placeholder.

use crate::qactions::{ActionIntent, ActionOutcome, ActionState, InfoValue, SystemControl};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

const POWER_SUPPLY_DIR: &str = "/sys/class/power_supply";
const NO_BATTERY: &str = "No battery";
const CHARGING_INFO_KEY: &str = "charging";

/// Reports the system battery charge. Action id: `"battery"`.
pub struct BatteryControl;

impl SystemControl for BatteryControl {
    fn state(&self) -> ActionState {
        match read_capacity() {
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
                text: if is_charging() {
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

/// The first `type == "Battery"` power supply's `capacity`, clamped to 0..=100.
/// `None` when there is no battery (desktop) or sysfs is unreadable.
fn read_capacity() -> Option<u8> {
    let entries = fs::read_dir(POWER_SUPPLY_DIR).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !is_battery(&path) {
            continue;
        }
        if let Some(pct) = read_trimmed(&path.join("capacity")).and_then(|s| s.parse::<u32>().ok())
        {
            return Some(pct.min(100) as u8);
        }
    }
    None
}

/// Whether the battery is actively charging, per its sysfs `status`.
fn is_charging() -> bool {
    let Ok(entries) = fs::read_dir(POWER_SUPPLY_DIR) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if is_battery(&path) && read_trimmed(&path.join("status")).as_deref() == Some("Charging") {
            return true;
        }
    }
    false
}

/// Whether a power-supply node is a battery (as opposed to an AC adapter).
fn is_battery(dir: &Path) -> bool {
    read_trimmed(&dir.join("type")).as_deref() == Some("Battery")
}

fn read_trimmed(path: &Path) -> Option<String> {
    Some(fs::read_to_string(path).ok()?.trim().to_string())
}
