//! Starting and stopping the processes behind `connect` blocks.

use crate::state::{home_dir, refresh_sources_now};
use std::ffi::{CStr, c_char};

/// Starts every declared connector, and restarts any that died. Called at
/// launch and on reload, since a reload is how a newly installed manifest is
/// noticed.
pub(crate) fn start(app_version: *const c_char) {
    let Some(home) = home_dir() else {
        return;
    };
    let app_version = if app_version.is_null() {
        ""
    } else {
        unsafe { CStr::from_ptr(app_version) }
            .to_str()
            .unwrap_or("")
    };
    look_engine::connectors::start(&home, app_version, on_rows());
}

/// Respawns any connector that died. Called when the launcher opens, which is
/// what "restarted on next use" means in practice.
pub(crate) fn restart_dead() {
    look_engine::connectors::restart_dead(on_rows());
}

/// Ends every connector. Their rows stay cached.
pub(crate) fn stop() {
    look_engine::connectors::stop();
}

/// Only the declared sources are re-read. Marking the whole index dirty would
/// make the next launcher open walk the home directory again.
fn on_rows() -> look_engine::connectors::OnRows {
    look_engine::connectors::coalesced(refresh_sources_now)
}
