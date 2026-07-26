//! Lock screen activation for Windows. Action id: `"screensaver"`.
//! Windows peer of `screensaver.rs`; `LockWorkStation` is the honest equivalent
//! of the macOS screensaver tile. Always available, so the tile renders wired.
//! Button-only.

use crate::qactions::{ActionIntent, ActionOutcome, ActionState, SystemControl};
use windows::Win32::System::Shutdown::LockWorkStation;

/// Locks the workstation. Button-only, no readable state. Action id:
/// `"screensaver"`.
pub struct ScreensaverControl;

impl SystemControl for ScreensaverControl {
    /// A button carries no on/off value; an empty value marks it present so the
    /// launchpad renders it wired. `LockWorkStation` is always available.
    fn state(&self) -> ActionState {
        ActionState::Value {
            value: String::new(),
        }
    }

    fn apply(&self, intent: ActionIntent) -> ActionOutcome {
        if intent != ActionIntent::Run {
            return ActionOutcome::Failed {
                message: "Screensaver has no toggle".to_string(),
            };
        }
        if unsafe { LockWorkStation() }.is_ok() {
            ActionOutcome::Ok {
                banner: Some("Locked".to_string()),
            }
        } else {
            ActionOutcome::Failed {
                message: "Could not lock the screen".to_string(),
            }
        }
    }
}
