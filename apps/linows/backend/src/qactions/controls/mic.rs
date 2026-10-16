//! Microphone mute toggle via the audio-server CLIs. Action id: `"mic"`.
//!
//! Linux has a real capture mute (unlike macOS, which zeroes input gain), so this
//! flips the default source's mute flag. Prefers PipeWire's `wpctl`, falls back to
//! PulseAudio's `pactl`. Reports `Unavailable` when neither tool nor a default
//! source answers. "On" == mic live (not muted), matching the On/Muted labels.

use crate::platform::linux::host_command;
use crate::qactions::{ActionIntent, ActionOutcome, ActionState, SystemControl};
use std::process::Command;

const WP_SOURCE: &str = "@DEFAULT_AUDIO_SOURCE@";
const PA_SOURCE: &str = "@DEFAULT_SOURCE@";
const NO_MIC: &str = "No microphone";
/// The line `wpctl get-volume` prints when it resolved a source at all.
const WP_VOLUME: &str = "Volume:";
const MONITOR_SUFFIX: &str = ".monitor";

/// Mutes and reports the default capture source. Action id: `"mic"`.
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

/// Whether the default capture source is muted, via `wpctl` then `pactl`. `None`
/// when neither backend answers (no server, no source, tool missing).
fn read_muted() -> Option<bool> {
    // "Volume: 0.40 [MUTED]" when muted. Checked for rather than trusting the
    // exit code: with no default source wpctl still exits 0, printing its error
    // on stderr and nothing on stdout, which reads as an unmuted mic.
    if let Some(out) = output(host_command("wpctl").args(["get-volume", WP_SOURCE]))
        && out.contains(WP_VOLUME)
    {
        return Some(out.contains("[MUTED]"));
    }
    // A monitor is the loopback of an output, not a capture device: pulse falls
    // back to one when the machine has no microphone at all.
    if default_source_is_monitor() {
        return None;
    }
    // "Mute: yes" / "Mute: no".
    if let Some(out) = output(host_command("pactl").args(["get-source-mute", PA_SOURCE])) {
        return Some(out.contains("yes"));
    }
    None
}

/// Whether pulse's default source is an output monitor.
fn default_source_is_monitor() -> bool {
    output(host_command("pactl").arg("get-default-source"))
        .is_some_and(|name| name.trim().ends_with(MONITOR_SUFFIX))
}

/// Set the default source mute flag; true on success. Tries `wpctl`, then `pactl`.
fn set_muted(mute: bool) -> bool {
    let flag = if mute { "1" } else { "0" };
    succeeds(host_command("wpctl").args(["set-mute", WP_SOURCE, flag]))
        || succeeds(host_command("pactl").args(["set-source-mute", PA_SOURCE, flag]))
}

fn output(cmd: &mut Command) -> Option<String> {
    let out = cmd.output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

fn succeeds(cmd: &mut Command) -> bool {
    cmd.status().map(|s| s.success()).unwrap_or(false)
}
