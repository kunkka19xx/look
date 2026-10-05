//! A launch query (`lookapp <mode>`), parked for the UI to pull.
//!
//! Parked on every path rather than pushed: cold start cannot push at all (the
//! UI is still booting when the arguments are read), and on a warm launch a
//! pushed event races the show, whose reset clears the query. The UI pulls
//! after that reset, so the show always runs first.

use std::sync::Mutex;

use look_engine::modes;

static PENDING_LAUNCH: Mutex<Option<String>> = Mutex::new(None);

pub fn take_launch_query() -> Option<String> {
    PENDING_LAUNCH
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take()
}

/// Park before showing: the pull hangs off the show.
pub fn park_launch(launch: &modes::Launch) {
    if let modes::Launch::Query { text } = launch {
        *PENDING_LAUNCH
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(text.clone());
    }
}
