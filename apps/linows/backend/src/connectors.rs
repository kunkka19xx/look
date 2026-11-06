//! When the shell starts the processes behind `connect` blocks, and what a
//! push refreshes. The host itself is `look_engine::connectors`.

use std::sync::OnceLock;

use look_engine::connectors::OnRows;

use crate::state::AppState;

/// The state a push refreshes, set by the first `start`.
static STATE: OnceLock<&'static AppState> = OnceLock::new();

/// Starts every declared connector and stops the rest. Called at launch and
/// on reload, which is how a new manifest or a block turned off is noticed.
pub fn start(state: &'static AppState) {
    let _ = STATE.set(state);
    let Some(home) = look_engine::config_path::home() else {
        return;
    };
    look_engine::connectors::start(std::path::Path::new(&home), on_rows(state));
}

/// Respawns connectors that died. Called on every open; reads no directories.
pub fn restart_dead() {
    if let Some(state) = STATE.get() {
        look_engine::connectors::restart_dead(on_rows(state));
    }
}

/// Ends every connector. Their rows stay cached.
pub fn stop() {
    look_engine::connectors::stop();
}

/// Re-reads the declared sources only, so the next open walks nothing else.
fn on_rows(state: &'static AppState) -> OnRows {
    look_engine::connectors::coalesced(|| state.refresh_sources_now())
}
