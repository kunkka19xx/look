//! Starting and stopping the processes behind `connect` blocks.

use std::sync::Arc;

use crate::state::mark_index_dirty;

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
        // Rows arriving are an index change like any other: the shell decides
        // when to act on it, and the next search picks them up.
        Arc::new(|_block: &str| mark_index_dirty()),
    );
}

pub(crate) fn stop() {
    look_engine::connectors::stop();
}
