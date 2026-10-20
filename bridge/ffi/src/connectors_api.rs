//! Starting and stopping the processes behind `connect` blocks.

use std::sync::Arc;

use crate::state::request_sources_refresh;

/// Starts every declared connector, and restarts any that died. Called at
/// launch and on reload, since a reload is how a newly installed manifest is
/// noticed.
pub(crate) fn start() {
    let Some(home) = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(std::path::PathBuf::from)
    else {
        return;
    };
    look_engine::connectors::start(
        &home,
        // Only the declared sources are re-read. Marking the whole index
        // dirty would make the next launcher open walk the home directory
        // again, which a connector pushing on a timer would do all day.
        Arc::new(|_block: &str| {
            request_sources_refresh();
        }),
    );
}

/// Respawns any connector that died. Called when the launcher opens, which is
/// what "restarted on next use" means in practice.
pub(crate) fn restart_dead() {
    look_engine::connectors::restart_dead(Arc::new(|_block: &str| {
        request_sources_refresh();
    }));
}

pub(crate) fn stop() {
    look_engine::connectors::stop();
}
