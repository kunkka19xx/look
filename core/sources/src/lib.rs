//! User-defined sources: rows the user declares, which join the launcher's own
//! index rather than living behind a picker of their own.
//!
//! This crate is the platform-neutral half. It reads the sources directory,
//! validates declarations, and produces rows for the kinds that need nothing but
//! the filesystem (folder and list). It deliberately does NOT know about
//! candidates, ids, ranking, or process execution: mapping rows into the index
//! is the engine's job, and running a user's command is the shell's, since only
//! the shells own a process seam and a terminal.
//!
//! See `specs/user-sources.md`.

mod collect;
mod def;
mod load;
mod rows;

pub use collect::{CollectError, Collected, MAX_ROWS_PER_SOURCE, collect, expand_home};
pub use def::{
    Action, ActionMode, DEFAULT_ACTION_KEY, DEFAULT_FOLDER_DEPTH, Only, Refresh, RowFormat, Scope,
    SourceDef, SourceSpec, inferred, parse,
};
pub use load::{Loaded, Problem, SOURCES_DIR_ENV, load_dir, sources_dir};
pub use rows::{SourceRow, parse_line, parse_lines};
