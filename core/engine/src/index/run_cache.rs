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

use std::fs;
use std::path::PathBuf;

use look_sources::{SourceRow, parse_lines};

/// Where a block's rows are kept between refreshes.
const CACHE_DIR_NAME: &str = ".look/cache/rows";

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
    let home = crate::config::user_home_dir()?;
    Some(PathBuf::from(home).join(CACHE_DIR_NAME).join(block_id))
}

/// The rows `block_id` last produced, or empty when it has never run.
pub(super) fn read(block_id: &str) -> Vec<SourceRow> {
    let Some(path) = cache_path(block_id) else {
        return Vec::new();
    };
    let Ok(contents) = fs::read(&path) else {
        return Vec::new();
    };
    let (rows, _) = parse_lines(&String::from_utf8_lossy(&contents), MAX_CACHED_ROWS);
    rows
}

/// Replaces `block_id`'s rows with `output` (a command's raw stdout).
///
/// Empty output is refused: a command that returns nothing is far more often
/// broken (network down, tool missing, wrong directory) than genuinely empty,
/// and keeping the last good rows costs nothing while clearing them loses the
/// user's ranking.
pub fn write(block_id: &str, output: &str) -> Result<usize, String> {
    let (rows, _) = parse_lines(output, MAX_CACHED_ROWS);
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
        assert!(write("probe", "").is_err());
        assert!(write("probe", "   \n\n").is_err());
    }
}
