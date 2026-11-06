//! When the shell starts the processes behind `connect` blocks, and what a
//! push refreshes. The host itself is `look_engine::connectors`.

use std::sync::{Mutex, OnceLock};

use look_engine::connectors::OnRows;

use crate::state::AppState;

/// The state a push refreshes, set by the first `start`.
static STATE: OnceLock<&'static AppState> = OnceLock::new();

/// Whether `stop` has run. Held across every start and stop, so a start still
/// on its thread when the shell quits launches nothing.
static STOPPED: Mutex<bool> = Mutex::new(false);

fn lifecycle() -> std::sync::MutexGuard<'static, bool> {
    STOPPED.lock().unwrap_or_else(|err| err.into_inner())
}

/// Starts every declared connector and stops the rest. Called at launch and
/// on reload, which is how a new manifest or a block turned off is noticed.
pub fn start(state: &'static AppState) {
    let stopped = lifecycle();
    if *stopped {
        return;
    }
    let _ = STATE.set(state);
    let Some(home) = look_engine::config_path::home() else {
        return;
    };
    look_engine::connectors::start(std::path::Path::new(&home), on_rows(state));
}

/// Respawns connectors that died. Called on every open; reads no directories.
pub fn restart_dead() {
    let stopped = lifecycle();
    if *stopped {
        return;
    }
    if let Some(state) = STATE.get() {
        look_engine::connectors::restart_dead(on_rows(state));
    }
}

/// Ends every connector. Their rows stay cached.
pub fn stop() {
    let mut stopped = lifecycle();
    *stopped = true;
    look_engine::connectors::stop();
}

/// Re-reads the declared sources only, so the next open walks nothing else.
fn on_rows(state: &'static AppState) -> OnRows {
    look_engine::connectors::coalesced(|| state.refresh_sources_now())
}
