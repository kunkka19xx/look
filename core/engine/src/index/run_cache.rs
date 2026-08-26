//! Rows a `run` block last produced.
//!
//! Running a user's command is the shell's job: it owns a process seam, and
//! spawning arbitrary commands on the indexing thread would let one slow script
//! stall the whole walk. So the shell runs the block, hands the rows here, and
//! the next index pass reads them back.
//!
//! The cache is what makes a failed refresh harmless. A command that errors,
//! times out, or returns nothing leaves the last good rows in place, because
//! losing them would also delete the usage history keyed to their ids.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use look_sources::{RowFormat, SourceRow, parse_rows};

/// Where a block's rows are kept between refreshes.
const CACHE_DIR_NAME: &str = ".look/cache/rows";

/// Overrides that directory. Mirrors `LOOK_SOURCES_DIR`, and is what lets a
/// test sweep a cache of its own rather than the one belonging to whoever is
/// running it.
pub const CACHE_DIR_ENV: &str = "LOOK_ROWS_CACHE_DIR";

fn cache_dir() -> Option<PathBuf> {
    if let Ok(custom) = std::env::var(CACHE_DIR_ENV) {
        let trimmed = custom.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed));
        }
    }
    Some(PathBuf::from(crate::config::user_home_dir()?).join(CACHE_DIR_NAME))
}

/// Same ceiling the collectors use, applied again on read: a cache file edited
/// or corrupted outside Look must not be able to flood the index.
const MAX_CACHED_ROWS: usize = look_sources::MAX_ROWS_PER_SOURCE;

fn cache_path(block_id: &str) -> Option<PathBuf> {
    // A block id is a TOML table header, so it can contain path separators. Any
    // id that could escape the cache directory is refused rather than sanitized,
    // since a silently renamed file would lose the rows it was meant to keep.
    if block_id.is_empty() || block_id.contains(['/', '\\']) || block_id.contains("..") {
        return None;
    }
    Some(cache_dir()?.join(block_id))
}

/// The rows `block_id` last produced, or empty when it has never run.
///
/// The cache holds the command's raw stdout, so reading it back needs the same
/// format the block declared when it was written.
pub(super) fn read(block_id: &str, format: RowFormat) -> Vec<SourceRow> {
    let Some(path) = cache_path(block_id) else {
        return Vec::new();
    };
    let Ok(contents) = fs::read(&path) else {
        return Vec::new();
    };
    let (rows, _) = parse_rows(&String::from_utf8_lossy(&contents), MAX_CACHED_ROWS, format)
        .unwrap_or_default();
    rows
}

/// Replaces `block_id`'s rows with `output` (a command's raw stdout).
///
/// Empty output is refused: a command that returns nothing is far more often
/// broken (network down, tool missing, wrong directory) than genuinely empty,
/// and keeping the last good rows costs nothing while clearing them loses the
/// user's ranking.
pub fn write(block_id: &str, output: &str, format: RowFormat) -> Result<usize, String> {
    let (rows, _) = parse_rows(output, MAX_CACHED_ROWS, format)?;
    if rows.is_empty() {
        return Err("produced no rows; keeping the previous ones".into());
    }
    let Some(path) = cache_path(block_id) else {
        return Err(format!("[{block_id}] is not a usable cache name"));
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    fs::write(&path, output).map_err(|err| err.to_string())?;
    Ok(rows.len())
}

/// Drops a block's rows, for a block the user deleted or disabled.
pub fn clear(block_id: &str) {
    if let Some(path) = cache_path(block_id) {
        let _ = fs::remove_file(path);
    }
}

/// Removes cached rows belonging to no declared block, and returns how many
/// went. `keep` is the set of blocks a refresh would write or preserve.
///
/// A block deleted from a file is never seen by a refresh again, so nothing
/// else can clear what it left: without this, its rows sit in the cache for
/// good, and re-adding a block under the same id would serve them.
///
/// Not called when the sources directory cannot be read. Zero declared blocks
/// then means "the directory is missing", not "the user deleted everything",
/// and sweeping on that reading would throw away every block's rows the first
/// time a home directory is slow to mount.
pub fn sweep(keep: &BTreeSet<String>) -> usize {
    let Some(dir) = cache_dir() else {
        return 0;
    };
    let Ok(entries) = fs::read_dir(&dir) else {
        return 0;
    };

    let mut removed = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if keep.contains(name) {
            continue;
        }
        if fs::remove_file(&path).is_ok() {
            removed += 1;
        }
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_id_that_could_escape_the_cache_directory_is_refused() {
        assert!(cache_path("../../etc/passwd").is_none());
        assert!(cache_path("a/b").is_none());
        assert!(cache_path("").is_none());
    }

    #[test]
    fn empty_output_keeps_the_previous_rows() {
        // A command returning nothing is usually broken, not empty, and the ids
        // it produced carry the user's usage history.
        assert!(write("probe", "", RowFormat::Lines).is_err());
        assert!(write("probe", "   \n\n", RowFormat::Lines).is_err());
    }

    #[test]
    fn output_that_is_not_the_declared_format_keeps_the_previous_rows() {
        // Same reasoning: a script whose JSON broke must not take the rows, and
        // the ranking they carry, down with it.
        assert!(write("probe", "look\tLook", RowFormat::Json).is_err());
    }
}
