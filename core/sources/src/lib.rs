//! User-defined blocks: lists and bundles the user declares, which join the
//! launcher's own index rather than living behind a picker of their own.
//!
//! This crate is the platform-neutral half. It reads the sources directory,
//! validates declarations, and produces rows for the producers that need
//! nothing but the filesystem. It deliberately does NOT know about candidates,
//! ids, ranking, or process execution: mapping rows into the index is the
//! engine's job, and running a user's command is the shell's.
//!
mod collect;
mod connect;
mod def;
mod detail;
mod load;
mod rows;
mod run;
mod tools;

pub use collect::{
    CollectError, Collected, MAX_ROWS_PER_SOURCE, collect, collect_for_row, expand_home,
};
pub use connect::{Ask, Content, PROTOCOL_VERSION, Row as ConnectorRow, Say, read as read_frame};
pub use def::{
    Applies, AppliesOnly, Block, Only, ParsedFile, Producer, RowFormat, RowKind, Verbs, inferred,
    parse_duration, parse_file,
};
pub use detail::{DetailBlock, DetailField, DetailItem, parse as parse_detail};
pub use load::{
    CONNECTORS_DIR_ENV, Loaded, Problem, SOURCES_DIR_ENV, connectors_dir, load_connectors,
    load_dir, sources_dir,
};
pub use rows::{SourceRow, parse_rows};
pub use run::{
    ParentRow, RowContext, StepOutcome, capture, end_connector, expand, expand_path, perform,
    spawn_connector,
};
pub use tools::{block_declares, resolve_for_row};
