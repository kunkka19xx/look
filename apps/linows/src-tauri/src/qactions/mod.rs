//! Quick Actions - the linows half of the shared framework (see
//! docs/writing-controls.md and the macOS peer in
//! apps/macos/.../Support/QuickActions/).
//!
//! Descriptors (which actions exist, their labels, control kind, info fields)
//! come from the shared `look-qactions` catalog; this module supplies the
//! native side: the `SystemControl` adapter contract, the registry resolving
//! an `action_id` to its adapter, and the Tauri commands the frontend calls.
//! Adapters block (D-Bus, CLIs), so state/apply run on the blocking pool,
//! mirroring `answers.rs`.

// Adapters exist for Linux and Windows (see `controls`); on any other target
// nothing constructs the success-path states/outcomes/values of the shared
// types below - they exist only to serialize back to the frontend. Silence the
// resulting dead_code lint there; a future adapter would use them and this
// lifts on its own.
#![cfg_attr(not(any(target_os = "linux", target_os = "windows")), allow(dead_code))]

pub mod controls;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::async_runtime;

/// Current state of a control's value, read for display in the panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum ActionState {
    On,
    Off,
    /// A non-boolean value shown as-is. Button controls (Screensaver, Restart,
    /// Shut Down) return it empty: they have no on/off value, but a resolved
    /// state marks them present so the launchpad renders them wired. Battery
    /// returns the charge percent here.
    // Constructed on Linux and Windows; dead only on a target with no adapters.
    #[cfg_attr(not(any(target_os = "linux", target_os = "windows")), allow(dead_code))]
    Value {
        value: String,
    },
    /// The control cannot act here: no hardware, no service, unsupported OS.
    Unavailable {
        reason: String,
    },
}

/// What the user asked a control to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionIntent {
    /// Flip a boolean control against its live state (on <-> off).
    Toggle,
    /// Drive a boolean control to an explicit target. The panel resolves a
    /// toggle press to this against the state it is showing, so a press does
    /// what the user sees even when the panel is stale (the system changed
    /// while the window was hidden); a blind `Toggle` would flip the live state
    /// and do the opposite. Wire form: `{ "set_on": true }`.
    SetOn(bool),
    /// Trigger a non-toggle action (a plain button).
    Run,
}

/// Result of applying an intent, surfaced to the user as a banner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ActionOutcome {
    /// Success. `banner` overrides the default confirmation text.
    Ok {
        banner: Option<String>,
    },
    Failed {
        message: String,
    },
    /// Part of the shared adapter contract; no linows control needs an OS
    /// permission yet.
    #[allow(dead_code)]
    NeedsPermission {
        message: String,
    },
}

/// `info()` key for the Battery adapters' charging flag. Battery is a
/// presentational tile with no shared `core/qactions` descriptor, so this key
/// is linows-only; defined once here rather than in each per-OS adapter file
/// so the Linux and Windows adapters can't drift out of sync on the spelling.
pub const BATTERY_CHARGING_INFO_KEY: &str = "charging";

/// A resolved info-field value. The shared descriptor declares `label` +
/// `value_key`; the adapter resolves the key to what to display.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum InfoValue {
    Text {
        text: String,
    },
    /// A set of items the panel renders one-per-row (e.g. each paired Bluetooth
    /// device), instead of squeezing them into a single value.
    List {
        items: Vec<ListItem>,
    },
    Unavailable {
        reason: String,
    },
}

/// One entry in an [`InfoValue::List`]. An `id` makes the row actionable via
/// [`SystemControl::apply_item`]; `on` drives an on/off marker (e.g. whether a
/// device is currently connected). Both are optional so a list can be plain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ListItem {
    pub id: Option<String>,
    pub label: String,
    pub on: Option<bool>,
}

/// The adapter a contributor implements per control - the one linows file you
/// write when adding one (see docs/writing-controls.md). Methods may block;
/// they always run off the UI thread. Best-effort: never panic, surface
/// problems as `Unavailable` / `Failed` / `NeedsPermission`.
pub trait SystemControl: Send + Sync {
    /// Read the current state for display.
    fn state(&self) -> ActionState;

    /// Resolve the descriptor's info `value_key`s to display values. Controls
    /// without info fields keep the default.
    fn info(&self, _keys: &[String]) -> HashMap<String, InfoValue> {
        HashMap::new()
    }

    /// Perform `intent` and report the outcome.
    fn apply(&self, intent: ActionIntent) -> ActionOutcome;

    /// Act on one item of a list-valued info field (e.g. connect/disconnect a
    /// specific device). Defaults to unsupported: most controls have no
    /// per-item actions.
    fn apply_item(&self, _item_id: &str, _intent: ActionIntent) -> ActionOutcome {
        ActionOutcome::Failed {
            message: "No per-item action".to_string(),
        }
    }
}

/// Resolves an action id to its native adapter - the one-line-per-control
/// registry. An id with a shared descriptor but no adapter here renders as
/// unavailable (declaration is shared across OSes; execution is not).
#[cfg(target_os = "linux")]
fn adapter(action_id: &str) -> Option<&'static dyn SystemControl> {
    use look_qactions::action_id as id;
    match action_id {
        id::BLUETOOTH => Some(&controls::bluetooth::BluetoothControl),
        id::WIFI => Some(&controls::wifi::WifiControl),
        id::THEME => Some(&controls::theme::ThemeControl),
        id::KEEP_AWAKE => Some(&controls::keepawake::KeepAwakeControl),
        id::MIC => Some(&controls::mic::MicControl),
        id::SCREENSAVER => Some(&controls::screensaver::ScreensaverControl),
        id::RESTART => Some(&controls::power::RestartControl),
        id::SHUTDOWN => Some(&controls::power::ShutdownControl),
        id::BATTERY => Some(&controls::battery::BatteryControl),
        _ => None,
    }
}

#[cfg(target_os = "windows")]
fn adapter(action_id: &str) -> Option<&'static dyn SystemControl> {
    use look_qactions::action_id as id;
    match action_id {
        id::BLUETOOTH => Some(&controls::bluetooth_windows::BluetoothControl),
        id::WIFI => Some(&controls::wifi_windows::WifiControl),
        id::THEME => Some(&controls::theme_windows::ThemeControl),
        id::KEEP_AWAKE => Some(&controls::keepawake_windows::KeepAwakeControl),
        id::MIC => Some(&controls::mic_windows::MicControl),
        id::SCREENSAVER => Some(&controls::screensaver_windows::ScreensaverControl),
        id::RESTART => Some(&controls::power_windows::RestartControl),
        id::SHUTDOWN => Some(&controls::power_windows::ShutdownControl),
        id::BATTERY => Some(&controls::battery_windows::BatteryControl),
        _ => None,
    }
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn adapter(_action_id: &str) -> Option<&'static dyn SystemControl> {
    None
}

const UNAVAILABLE_ON_OS: &str = "Not supported on this system";

/// One action's live state plus its resolved info values, fetched together so
/// a selection costs a single IPC round trip.
#[derive(Serialize)]
pub struct QuickActionStatus {
    pub state: ActionState,
    pub info: HashMap<String, InfoValue>,
}

fn unavailable_status() -> QuickActionStatus {
    QuickActionStatus {
        state: ActionState::Unavailable {
            reason: UNAVAILABLE_ON_OS.to_string(),
        },
        info: HashMap::new(),
    }
}

/// Quick Action descriptors for a selected result, from the shared catalog.
/// Empty for results with no actions (the common case).
#[tauri::command]
pub fn quick_actions(result_id: String, kind: String) -> Vec<look_qactions::ActionDescriptor> {
    look_qactions::descriptors_for(&result_id, &kind)
}

/// The empty-state launchpad's tile layout, from the shared catalog. One source
/// of truth across shells: the frontend renders these tiles (order, size, role,
/// mnemonic, labels) instead of hardcoding its own copy.
#[tauri::command]
pub fn launchpad_layout() -> Vec<look_qactions::LaunchpadTile> {
    look_qactions::launchpad_layout()
}

/// Live state + info values for an action. `info_keys` are the descriptor's
/// `value_key`s the frontend wants resolved.
#[tauri::command]
pub async fn quick_action_state(action_id: String, info_keys: Vec<String>) -> QuickActionStatus {
    async_runtime::spawn_blocking(move || match adapter(&action_id) {
        Some(control) => QuickActionStatus {
            state: control.state(),
            info: control.info(&info_keys),
        },
        None => unavailable_status(),
    })
    .await
    .unwrap_or_else(|_| unavailable_status())
}

/// Run an action's intent; the outcome feeds the banner.
#[tauri::command]
pub async fn quick_action_apply(action_id: String, intent: ActionIntent) -> ActionOutcome {
    async_runtime::spawn_blocking(move || match adapter(&action_id) {
        Some(control) => control.apply(intent),
        None => ActionOutcome::Failed {
            message: UNAVAILABLE_ON_OS.to_string(),
        },
    })
    .await
    .unwrap_or_else(|_| ActionOutcome::Failed {
        message: "Action failed".to_string(),
    })
}

/// Run an intent against one list item of an action (e.g. toggle a device's
/// connection). Like `quick_action_apply`, but targets an item by id.
#[tauri::command]
pub async fn quick_action_apply_item(
    action_id: String,
    item_id: String,
    intent: ActionIntent,
) -> ActionOutcome {
    async_runtime::spawn_blocking(move || match adapter(&action_id) {
        Some(control) => control.apply_item(&item_id, intent),
        None => ActionOutcome::Failed {
            message: UNAVAILABLE_ON_OS.to_string(),
        },
    })
    .await
    .unwrap_or_else(|_| ActionOutcome::Failed {
        message: "Action failed".to_string(),
    })
}
