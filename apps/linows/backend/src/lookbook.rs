//! What a lookbook entry would do on this machine, and installing it. One
//! hop each into `look_engine::lookbook`, shared with macOS.

use std::path::PathBuf;

use look_engine::lookbook::{
    InstallOutcome, InstallRequest, NamedFile, Present, PresentRequest, Preview,
};

use crate::state::AppState;

const NO_HOME: &str = "no home directory";

/// What `toml` declares and what it would need here, or why it is not a file
/// of blocks at all.
pub fn preview(toml: &str) -> Result<Preview, String> {
    look_engine::lookbook::preview(toml, &home()?)
}

/// The same with the answers filled in, so the sheet shows what Install
/// would write.
pub fn preview_filled(file: &NamedFile) -> Result<Preview, String> {
    look_engine::lookbook::preview_filled(file, &home()?)
}

/// Which of a catalog entry's `requires` are not installed here.
pub fn missing_programs(programs: &[String]) -> Vec<String> {
    look_engine::lookbook::missing_programs(programs)
}

/// Which of an entry's files are already on disk, so the sheet can say what
/// it would replace.
pub fn present(request: &PresentRequest) -> Result<Present, String> {
    Ok(look_engine::lookbook::present(request, &home()?))
}

/// Writes an entry and re-reads the sources, which no watcher covers. Queues
/// a full refresh when one is already running.
pub fn install(request: &InstallRequest, state: &AppState) -> Result<InstallOutcome, String> {
    let outcome = look_engine::lookbook::install(request, &home()?);
    if !outcome.written.is_empty() && !state.refresh_sources_now() {
        state.request_index_refresh();
    }
    Ok(outcome)
}

fn home() -> Result<PathBuf, String> {
    crate::sources::home().ok_or_else(|| NO_HOME.to_string())
}
