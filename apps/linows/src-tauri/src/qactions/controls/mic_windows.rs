//! Microphone mute toggle for Windows. Action id: `"mic"`.
//!
//! Windows peer of `mic.rs`. Windows has a real capture mute, so this flips the
//! default capture endpoint's mute flag through Core Audio's
//! `IAudioEndpointVolume`, the same switch the volume flyout's mic button drives.
//! Reports `Unavailable` when there is no default capture device. "On" == mic
//! live (not muted), matching the On/Muted labels.

use crate::qactions::{ActionIntent, ActionOutcome, ActionState, SystemControl};
use std::sync::Once;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{IMMDeviceEnumerator, MMDeviceEnumerator, eCapture, eConsole};
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance, CoIncrementMTAUsage};

const NO_MIC: &str = "No microphone";

/// Mutes and reports the default capture endpoint. Action id: `"mic"`.
pub struct MicControl;

impl SystemControl for MicControl {
    fn state(&self) -> ActionState {
        match read_muted() {
            Some(true) => ActionState::Off,
            Some(false) => ActionState::On,
            None => ActionState::Unavailable {
                reason: NO_MIC.to_string(),
            },
        }
    }

    fn apply(&self, intent: ActionIntent) -> ActionOutcome {
        let Some(muted) = read_muted() else {
            return ActionOutcome::Failed {
                message: NO_MIC.to_string(),
            };
        };
        let mute = match intent {
            ActionIntent::Toggle => !muted,
            // "on" = mic live = unmuted.
            ActionIntent::SetOn(on) => !on,
            ActionIntent::Run => {
                return ActionOutcome::Failed {
                    message: "Mic has no run action".to_string(),
                };
            }
        };
        if set_muted(mute) {
            ActionOutcome::Ok {
                banner: Some(if mute { "Mic muted" } else { "Mic on" }.to_string()),
            }
        } else {
            ActionOutcome::Failed {
                message: format!("Could not {} mic", if mute { "mute" } else { "unmute" }),
            }
        }
    }
}

/// Keep the process in an MTA so Core Audio COM calls on pooled blocking threads
/// work (a thread that never called `CoInitializeEx` joins the implicit MTA).
fn ensure_mta() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let _ = unsafe { CoIncrementMTAUsage() };
    });
}

/// The default capture endpoint's volume interface, or `None` when there is no
/// default capture device (unplugged, disabled).
fn endpoint_volume() -> Option<IAudioEndpointVolume> {
    ensure_mta();
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
        let device = enumerator
            .GetDefaultAudioEndpoint(eCapture, eConsole)
            .ok()?;
        device
            .Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None)
            .ok()
    }
}

fn read_muted() -> Option<bool> {
    let volume = endpoint_volume()?;
    unsafe { volume.GetMute() }.ok().map(|m| m.as_bool())
}

fn set_muted(mute: bool) -> bool {
    let Some(volume) = endpoint_volume() else {
        return false;
    };
    // Null event context: no specific caller to exclude from the change notice.
    unsafe { volume.SetMute(mute, std::ptr::null()) }.is_ok()
}
