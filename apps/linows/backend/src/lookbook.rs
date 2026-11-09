//! What a lookbook entry would do on this machine, and installing it. Every
//! call is one hop into `look_engine::lookbook`, shared with macOS, so the
//! two shells say the same thing about the same file.

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

/// The same for a declaration whose answers are already chosen, so the sheet
/// shows what Install would land rather than what the author shipped.
pub fn preview_filled(file: &NamedFile) -> Result<Preview, String> {
    look_engine::lookbook::preview_filled(file, &home()?)
}

/// Which of `programs` are not installed here. For a catalog entry's
/// `requires`, which names a program with no block behind it.
pub fn missing_programs(programs: &[String]) -> Vec<String> {
    look_engine::lookbook::missing_programs(programs)
}

/// Which of an entry's files are already on disk, so the sheet can say what
/// it would replace before it writes.
pub fn present(request: &PresentRequest) -> Result<Present, String> {
    Ok(look_engine::lookbook::present(request, &home()?))
}

/// Writes an entry into the user's directories and re-reads the sources,
/// since no watcher covers them. Blocks on that read, or queues a full one
/// behind whatever refresh is already running.
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
