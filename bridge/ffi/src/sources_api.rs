//! C-ABI wrappers over `look_sources`, so the Swift shell can read a
//! user-declared block, list what a row can go on to, and perform it.
//!
//! A row carries only its candidate id, so every entry point starts by
//! resolving `src:<block>:<row>` back to the block that declared it. Reading the
//! directory again on demand (rather than caching it here) means a file the user
//! just edited takes effect without a reindex.

use std::path::PathBuf;

use look_indexing::CandidateIdKind;
use look_sources::{Block, Producer, RowContext, load_dir, sources_dir};
use serde::Serialize;
use std::os::raw::c_char;

use crate::state::{cstr_to_string, json_cstring_or_null};

/// Somewhere a row can go from here. `performs` is what the target's own
/// producer decides: steps to run, or rows to descend into.
#[derive(Serialize)]
struct ThenTarget {
    id: String,
    name: String,
    icon: Option<String>,
    performs: bool,
}

/// What the panel shows for a block row: its name, the exact steps Enter will
/// perform, and where a row can go next.
#[derive(Serialize)]
struct BlockDetail {
    id: String,
    name: String,
    steps: Vec<String>,
    /// The file this block was declared in, so the panel can show it and
    /// reveal-in-Finder has something to point at.
    file: Option<String>,
    then: Vec<ThenTarget>,
}

/// One row of the block index the shell caches: enough to render a row without
/// re-reading the directory for every one of them.
#[derive(Serialize)]
struct BlockSummary {
    id: String,
    name: String,
    icon: Option<String>,
}

#[derive(Serialize)]
struct PerformOutcome {
    performed: usize,
    errors: Vec<String>,
}

/// `{id, name, steps, file, then}` for the block a candidate id belongs to, or
/// the JSON literal `null` when it is not a block row or no longer exists.
pub(crate) fn look_source_block_json_impl(candidate_id: *const c_char) -> *mut c_char {
    let candidate_id = cstr_to_string(candidate_id);
    let Some(block_id) = CandidateIdKind::source_id_of(&candidate_id) else {
        return json_cstring_or_null(None);
    };
    let Some(home) = home_dir() else {
        return json_cstring_or_null(None);
    };

    let blocks = load_dir(&sources_dir(&home)).blocks;
    let Some(block) = blocks.iter().find(|block| block.id == block_id) else {
        return json_cstring_or_null(None);
    };

    let then = block
        .then
        .iter()
        .filter_map(|target| blocks.iter().find(|block| &block.id == target))
        .map(|target| ThenTarget {
            id: target.id.clone(),
            name: target.name.clone(),
            icon: target.icon.clone(),
            performs: target.is_bundle(),
        })
        .collect();

    json_cstring_or_null(
        serde_json::to_string(&BlockDetail {
            id: block.id.clone(),
            name: block.name.clone(),
            steps: steps_of(block),
            file: block.source_file.clone(),
            then,
        })
        .ok(),
    )
}

/// Every declared block as `{id, name, icon}`. The shell caches this once per
/// launcher open so a row can show its declared icon without a disk read each
/// time it renders.
pub(crate) fn look_source_blocks_json_impl() -> *mut c_char {
    let Some(home) = home_dir() else {
        return json_cstring_or_null(Some("[]".to_string()));
    };
    let summaries: Vec<BlockSummary> = load_dir(&sources_dir(&home))
        .blocks
        .into_iter()
        .map(|block| BlockSummary {
            id: block.id,
            name: block.name,
            icon: block.icon,
        })
        .collect();
    json_cstring_or_null(serde_json::to_string(&summaries).ok())
}

/// Performs `block_id`'s steps against the selected row, which is what its
/// placeholders expand to. Returns `{performed, errors}`; an empty `errors`
/// means every step was spawned.
pub(crate) fn look_perform_block_json_impl(
    block_id: *const c_char,
    row_id: *const c_char,
    row_title: *const c_char,
    row_path: *const c_char,
    query: *const c_char,
) -> *mut c_char {
    let block_id = cstr_to_string(block_id);
    let row = RowContext {
        id: cstr_to_string(row_id),
        title: cstr_to_string(row_title),
        path: cstr_to_string(row_path),
        query: cstr_to_string(query),
    };

    let Some(block) = find_block(&block_id) else {
        return outcome(0, vec!["that block no longer exists".into()]);
    };
    let Producer::Bundle { steps } = &block.producer else {
        return outcome(0, vec![format!("[{}] has no steps to perform", block.id)]);
    };

    let outcomes = look_sources::perform(steps, Some(&row));
    let errors: Vec<String> = outcomes
        .iter()
        .filter_map(|step| {
            step.error
                .as_ref()
                .map(|error| format!("{}: {error}", step.step))
        })
        .collect();
    outcome(outcomes.len() - errors.len(), errors)
}

fn steps_of(block: &Block) -> Vec<String> {
    match &block.producer {
        Producer::Bundle { steps } => steps.clone(),
        _ => Vec::new(),
    }
}

fn outcome(performed: usize, errors: Vec<String>) -> *mut c_char {
    json_cstring_or_null(serde_json::to_string(&PerformOutcome { performed, errors }).ok())
}

fn find_block(block_id: &str) -> Option<Block> {
    let home = home_dir()?;
    load_dir(&sources_dir(&home))
        .blocks
        .into_iter()
        .find(|block| block.id == block_id)
}

fn home_dir() -> Option<PathBuf> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
}
