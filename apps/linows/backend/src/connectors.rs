//! Starting and stopping the processes behind `connect` blocks.
//!
//! The host itself is `look_engine::connectors`, shared with macOS. What is
//! here is when the shell starts them, and which state a push refreshes.

use std::sync::OnceLock;

use look_engine::connectors::OnRows;

use crate::state::AppState;

/// The state a push refreshes. Set once, by the first `start`: a restart
/// reads it rather than being handed it again.
static STATE: OnceLock<&'static AppState> = OnceLock::new();

/// Starts every declared connector, and stops any whose block is gone or
/// turned off. Called at launch and on reload: a reload is how a newly
/// installed manifest is noticed, and how a block turned off from Settings
/// gets its process ended.
pub fn start(state: &'static AppState) {
    let _ = STATE.set(state);
    let Some(home) = look_engine::config_path::home() else {
        return;
    };
    look_engine::connectors::start(std::path::Path::new(&home), on_rows(state));
}

/// Respawns any connector that died. Called when the launcher opens, which
/// is what "restarted on next use" means in practice. Reads no directories.
pub fn restart_dead() {
    if let Some(state) = STATE.get() {
        look_engine::connectors::restart_dead(on_rows(state));
    }
}

/// Ends every connector. Their rows stay cached, so the next launch shows
/// what they last sent while they start again.
pub fn stop() {
    look_engine::connectors::stop();
}

/// Only the declared sources are re-read. Marking the whole index dirty would
/// make the next launcher open walk the home directory again.
fn on_rows(state: &'static AppState) -> OnRows {
    look_engine::connectors::coalesced(|| state.refresh_sources_now())
}
