//! User-declared blocks: what a row's block is, what Enter will run, and the
//! rows a `then` target produces.
//!
//! Every one of these is a hop into `look_engine::sources`, which owns the
//! whole answer and is shared with macOS, so the two shells cannot disagree
//! about what `confirm` expands to or how a drilled id is spelled. Four of them
//! spawn processes, so they are `async` commands: Tauri runs those off the
//! request thread, and a `run` block the user wrote may take seconds.

use look_engine::sources::{
    BlockDetail, BlockSummary, Level, PerformOutcome, PreviewOutcome, RefreshOutcome,
};

/// The row a command acts on, as the frontend holds it. One struct because the
/// four always travel together and a block's verbs expand against all of them.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RowArgs {
    pub candidate_id: String,
    #[serde(default)]
    pub row_title: String,
    #[serde(default)]
    pub row_path: String,
    #[serde(default)]
    pub query: String,
    /// The levels this row was reached through, nearest parent first, as
    /// `[{id, title, path}]`. Empty outside a level.
    #[serde(default)]
    pub ancestors: String,
}

impl RowArgs {
    pub fn context(&self) -> look_sources::RowContext {
        look_engine::sources::row_context(
            // The candidate id: core strips the namespace, so a script asking
            // for a branch gets `main` rather than `src:branches:main`.
            &self.candidate_id,
            &self.row_title,
            &self.row_path,
            &self.query,
            look_engine::sources::parents_from_json(&self.ancestors),
        )
    }

    /// The same row without what was typed to reach it. A chord carries no
    /// query: `{query}` is the search text, and Ctrl+E is not that.
    pub fn context_without_query(&self) -> look_sources::RowContext {
        look_sources::RowContext {
            query: String::new(),
            ..self.context()
        }
    }
}

/// The block behind a row: its name, the exact steps Enter will perform, the
/// file that declared it, and where the row can go next, plus the actions a
/// user declared for rows LIKE it (`applies`). `null` when the row is not a
/// block row and nothing was declared for it.
#[tauri::command(async)]
pub fn source_block(row: RowArgs) -> Option<BlockDetail> {
    look_engine::sources::block_detail(&row.candidate_id, &row.context())
}

/// Every declared block as `{id, name, icon}`, for the frontend's row-icon
/// cache. One disk read per launcher open rather than one per row.
#[tauri::command(async)]
pub fn source_blocks() -> Vec<BlockSummary> {
    look_engine::sources::block_summaries()
}

/// Runs `block_id` against the row, or reports that it produces rows to descend
/// into instead.
///
/// `as_target` is the caller's intent: Enter on a row means "do what this
/// block's `open` says", while picking a `then` target means "go to this
/// block".
#[tauri::command(async)]
pub fn perform_block(block_id: String, row: RowArgs, as_target: bool) -> PerformOutcome {
    look_engine::sources::perform_block(&block_id, &row.context(), as_target)
}

/// The rows of `block_id` produced against the row the level opens from. Live
/// on every call: the run cache is keyed by block alone, and a drilled block
/// produces different rows per parent.
#[tauri::command(async)]
pub fn source_rows(block_id: String, parent: RowArgs) -> Level {
    look_engine::sources::level(
        &block_id,
        &parent.candidate_id,
        &parent.row_title,
        &parent.row_path,
        &parent.query,
        look_engine::sources::parents_from_json(&parent.ancestors),
    )
}

/// A block's declared `preview`, run against the selected row. `null` when the
/// block declares none; a failure comes back as its reason rather than empty.
#[tauri::command(async)]
pub fn source_preview(row: RowArgs) -> Option<PreviewOutcome> {
    look_engine::sources::preview(&row.candidate_id, &row.context())
}

/// Re-runs every top-level `run` block and stores its rows for the next index
/// pass. A block that fails keeps the rows it had.
#[tauri::command(async)]
pub fn refresh_run_blocks() -> RefreshOutcome {
    look_engine::sources::refresh_run_blocks()
}
