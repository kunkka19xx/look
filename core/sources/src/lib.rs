//! User-defined blocks: lists and bundles the user declares, which join the
//! launcher's own index rather than living behind a picker of their own.
//!
//! This crate is the platform-neutral half. It reads the sources directory,
//! validates declarations, and produces rows for the producers that need
//! nothing but the filesystem. It deliberately does NOT know about candidates,
//! ids, ranking, or process execution: mapping rows into the index is the
//! engine's job, and running a user's command is the shell's.
//!
//! See `specs/user-sources.md`.

mod collect;
mod def;
mod load;
mod rows;
mod run;

pub use collect::{CollectError, Collected, MAX_ROWS_PER_SOURCE, collect, expand_home};
pub use def::{
    Block, DEFAULT_FOLDER_DEPTH, KEY_DIR, KEY_DO, KEY_FILE, KEY_RUN, Only, ParsedFile, Producer,
    Refresh, RowFormat, Verbs, inferred, parse_file,
};
pub use load::{Loaded, Problem, SOURCES_DIR_ENV, load_dir, sources_dir};
pub use rows::{SourceRow, parse_line, parse_lines};
pub use run::{ENV_ID, ENV_PATH, ENV_TITLE, RowContext, StepOutcome, capture, expand, perform};
