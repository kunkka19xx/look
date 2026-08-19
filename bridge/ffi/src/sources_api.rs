//! C-ABI wrappers over `look_sources`, so the Swift shell can read a
//! user-declared block and perform it.
//!
//! A row carries only its candidate id, so both entry points start by resolving
//! `src:<block>:<row>` back to the block that declared it. Reading the
//! directory again on demand (rather than caching it here) means a file the user
//! just edited takes effect without a reindex.

use std::path::PathBuf;

use look_indexing::CandidateIdKind;
use look_sources::{Block, Producer, load_dir, sources_dir};
use serde::Serialize;
use std::os::raw::c_char;

use crate::state::{cstr_to_string, json_cstring_or_null};

/// What the panel shows for a block row: its name and the exact steps Enter
/// will perform, so the user reads what will happen before it happens.
#[derive(Serialize)]
struct BlockDetail {
    id: String,
    name: String,
    steps: Vec<String>,
    /// The file this block was declared in, so the panel can show it and
    /// reveal-in-Finder has something to point at.
    file: Option<String>,
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

/// `{id, name, steps}` for the block a candidate id belongs to, or the JSON
/// literal `null` when it is not a block row or no longer exists.
pub(crate) fn look_source_block_json_impl(candidate_id: *const c_char) -> *mut c_char {
    let candidate_id = cstr_to_string(candidate_id);
    json_cstring_or_null(
        find_block(&candidate_id)
            .map(|block| {
                let steps = match &block.producer {
                    Producer::Bundle { steps } => steps.clone(),
                    _ => Vec::new(),
                };
                BlockDetail {
                    id: block.id.clone(),
                    name: block.name.clone(),
                    steps,
                    file: block.source_file.clone(),
                }
            })
            .and_then(|detail| serde_json::to_string(&detail).ok()),
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

/// Performs every step of the block a candidate id belongs to. Returns
/// `{performed, errors}`; an empty `errors` means every step was spawned.
pub(crate) fn look_perform_block_json_impl(candidate_id: *const c_char) -> *mut c_char {
    let candidate_id = cstr_to_string(candidate_id);
    let Some(block) = find_block(&candidate_id) else {
        return json_cstring_or_null(
            serde_json::to_string(&PerformOutcome {
                performed: 0,
                errors: vec!["that block no longer exists".into()],
            })
            .ok(),
        );
    };

    let Producer::Bundle { steps } = &block.producer else {
        return json_cstring_or_null(
            serde_json::to_string(&PerformOutcome {
                performed: 0,
                errors: vec![format!("[{}] has no steps to perform", block.id)],
            })
            .ok(),
        );
    };

    let outcomes = look_sources::perform(steps, None);
    let errors: Vec<String> = outcomes
        .iter()
        .filter_map(|outcome| {
            outcome
                .error
                .as_ref()
                .map(|error| format!("{}: {error}", outcome.step))
        })
        .collect();

    json_cstring_or_null(
        serde_json::to_string(&PerformOutcome {
            performed: outcomes.len() - errors.len(),
            errors,
        })
        .ok(),
    )
}

fn find_block(candidate_id: &str) -> Option<Block> {
    let block_id = CandidateIdKind::source_id_of(candidate_id)?;
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
