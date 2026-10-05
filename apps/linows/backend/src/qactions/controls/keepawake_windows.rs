//! Keep-awake power request for Windows. Action id: `"keepawake"`.
//!
//! Windows peer of `keepawake.rs`. `SetThreadExecutionState(ES_CONTINUOUS |
//! ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED)` holds the machine awake; passing
//! `ES_CONTINUOUS` alone releases it. State is in-memory: it resets to off if
//! Look restarts, matching the macOS and Linux peers.
//!
//! The request is thread-associated and cleared when its thread exits, so a
//! pooled blocking thread (tokio reaps idle ones) can't own it. A single
//! long-lived worker holds the request instead; the tile just signals it.

use crate::qactions::{ActionIntent, ActionOutcome, ActionState, SystemControl};
use std::sync::{Condvar, Mutex, Once, OnceLock};
use windows::Win32::System::Power::{
    ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED, SetThreadExecutionState,
};

/// Desired on/off plus a condvar the tile pulses to wake the worker.
struct Shared {
    on: Mutex<bool>,
    changed: Condvar,
}

fn shared() -> &'static Shared {
    static SHARED: OnceLock<Shared> = OnceLock::new();
    SHARED.get_or_init(|| Shared {
        on: Mutex::new(false),
        changed: Condvar::new(),
    })
}

/// Toggles a system + display power request so the machine does not sleep while
/// on. Action id: `"keepawake"`.
pub struct KeepAwakeControl;

impl SystemControl for KeepAwakeControl {
    fn state(&self) -> ActionState {
        if *shared().on.lock().unwrap() {
            ActionState::On
        } else {
            ActionState::Off
        }
    }

    fn apply(&self, intent: ActionIntent) -> ActionOutcome {
        ensure_worker();
        let shared = shared();
        let mut on = shared.on.lock().unwrap();
        let target = match intent {
            ActionIntent::Toggle => !*on,
            ActionIntent::SetOn(set) => set,
            ActionIntent::Run => {
                return ActionOutcome::Failed {
                    message: "Keep Awake has no run action".to_string(),
                };
            }
        };
        *on = target;
        // Wake the worker to apply the new state on its own (persistent) thread.
        shared.changed.notify_all();
        ActionOutcome::Ok {
            banner: Some(format!("Keep Awake {}", if target { "on" } else { "off" })),
        }
    }
}

/// Spawn the worker once. It owns the execution-state request for the process
/// life so the request outlives any single tile press.
fn ensure_worker() {
    static START: Once = Once::new();
    START.call_once(|| {
        std::thread::spawn(|| {
            let shared = shared();
            let mut applied = false;
            let mut on = shared.on.lock().unwrap();
            loop {
                // Wait until the desired state differs from what we last applied.
                on = shared.changed.wait_while(on, |v| *v == applied).unwrap();
                applied = *on;
                let flags = if applied {
                    ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED
                } else {
                    ES_CONTINUOUS
                };
                unsafe { SetThreadExecutionState(flags) };
            }
        });
    });
}
