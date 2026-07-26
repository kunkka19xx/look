//! Windows Wi-Fi radio toggle. Action id: `"wifi"`. Windows peer of `wifi.rs`;
//! the WinRT radio plumbing is shared with Bluetooth in `radio_windows`.

use super::radio_windows::{self as radio, RadioError};
use crate::qactions::{ActionIntent, ActionOutcome, ActionState, SystemControl};
use windows::Devices::Radios::RadioKind;

const NO_ADAPTER: &str = "No Wi-Fi hardware";
const ACCESS_DENIED: &str = "Wi-Fi access is blocked by Windows";

pub struct WifiControl;

impl SystemControl for WifiControl {
    fn state(&self) -> ActionState {
        match radio::of_kind(RadioKind::WiFi) {
            Some(r) if radio::is_on(&r) => ActionState::On,
            Some(_) => ActionState::Off,
            None => ActionState::Unavailable {
                reason: NO_ADAPTER.to_string(),
            },
        }
    }

    fn apply(&self, intent: ActionIntent) -> ActionOutcome {
        let Some(r) = radio::of_kind(RadioKind::WiFi) else {
            return ActionOutcome::Failed {
                message: NO_ADAPTER.to_string(),
            };
        };
        let target = match intent {
            ActionIntent::Toggle => !radio::is_on(&r),
            ActionIntent::SetOn(on) => on,
            ActionIntent::Run => {
                return ActionOutcome::Failed {
                    message: "Wi-Fi has no run action".to_string(),
                };
            }
        };
        let failed = || format!("Could not turn Wi-Fi {}", radio::on_off(target));
        match radio::set_powered(&r, target) {
            Ok(()) => {}
            Err(RadioError::Denied) => {
                return ActionOutcome::Failed {
                    message: ACCESS_DENIED.to_string(),
                };
            }
            Err(RadioError::Failed) => return ActionOutcome::Failed { message: failed() },
        }
        if !radio::wait_for(&r, target) {
            return ActionOutcome::Failed { message: failed() };
        }
        ActionOutcome::Ok {
            banner: Some(format!("Wi-Fi {}", radio::on_off(target))),
        }
    }
}
