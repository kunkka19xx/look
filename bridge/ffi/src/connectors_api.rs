//! Starting and stopping the processes behind `connect` blocks.

use crate::state::{home_dir, refresh_sources_now};

/// Starts every declared connector, and restarts any that died. Called at
/// launch and on reload, since a reload is how a newly installed manifest is
/// noticed.
pub(crate) fn start() {
    let Some(home) = home_dir() else {
        return;
    };
    look_engine::connectors::start(&home, on_rows());
}

/// Respawns any connector that died. Called when the launcher opens, which is
/// what "restarted on next use" means in practice.
pub(crate) fn restart_dead() {
    look_engine::connectors::restart_dead(on_rows());
}

pub(crate) fn stop() {
    look_engine::connectors::stop();
}

/// Only the declared sources are re-read. Marking the whole index dirty would
/// make the next launcher open walk the home directory again.
fn on_rows() -> look_engine::connectors::OnRows {
    look_engine::connectors::coalesced(refresh_sources_now)
}
