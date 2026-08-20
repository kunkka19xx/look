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
use std::time::Duration;

use crate::state::{cstr_to_string, json_cstring_or_null};

/// Somewhere a row can go from here. `performs` is what the target's own
/// producer decides: steps to run, or rows to descend into.
#[derive(Serialize)]
struct ThenTarget {
    id: String,
    name: String,
    icon: Option<String>,
    performs: bool,
    /// The question to ask before running it, already expanded against the row,
    /// or null when it needs no confirmation.
    confirm: Option<String>,
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
pub(crate) fn look_source_block_json_impl(
    candidate_id: *const c_char,
    row_id: *const c_char,
    row_title: *const c_char,
    row_path: *const c_char,
) -> *mut c_char {
    let candidate_id = cstr_to_string(candidate_id);
    let row = row_context(row_id, row_title, row_path, "");
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
            // Expanded here so the question names the row the user is looking
            // at ("Delete main?"), not the template.
            confirm: target
                .confirm
                .as_deref()
                .map(|question| look_sources::expand(question, &row)),
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
    let row = row_context(row_id, row_title, row_path, &cstr_to_string(query));

    let Some(block) = find_block(&block_id) else {
        return outcome(0, vec!["that block no longer exists".into()]);
    };

    // A bundle IS its steps. Any other producer makes rows, so what Enter does
    // to one of them is the block's `open` verb.
    let steps: Vec<String> = match &block.producer {
        Producer::Bundle { steps } => steps.clone(),
        _ => match block.verbs.open.as_deref() {
            Some(command) => vec![command.to_string()],
            None => {
                return outcome(
                    0,
                    vec![format!("[{}] declares no `open` for its rows", block.id)],
                );
            }
        },
    };

    let outcomes = look_sources::perform(&steps, Some(&row));
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

/// Fallback limits for a captured command, when the block names none. A source
/// refresh happens while the user waits, so the ceiling is low on purpose.
const DEFAULT_CAPTURE_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_CAPTURE_BYTES: usize = 256 * 1024;

/// Re-runs every enabled `run` block and stores its rows, so the next index pass
/// picks them up. Returns `{refreshed, errors}`.
///
/// A block that fails keeps the rows it had: losing them would also drop the
/// usage history keyed to their ids, which is a worse outcome than stale rows.
pub(crate) fn look_refresh_run_blocks_json_impl() -> *mut c_char {
    let Some(home) = home_dir() else {
        return json_cstring_or_null(Some("{\"refreshed\":0,\"errors\":[]}".into()));
    };

    let mut refreshed = 0usize;
    let mut errors: Vec<String> = Vec::new();
    for block in load_dir(&sources_dir(&home)).blocks {
        let Producer::Run {
            command,
            cwd,
            timeout,
            ..
        } = &block.producer
        else {
            continue;
        };
        if !block.enabled {
            look_engine::index::clear_run_rows(&block.id);
            continue;
        }

        let outcome = look_sources::capture(
            command,
            cwd.as_deref(),
            timeout.unwrap_or(DEFAULT_CAPTURE_TIMEOUT),
            MAX_CAPTURE_BYTES,
        )
        .and_then(|output| look_engine::index::store_run_rows(&block.id, &output));

        match outcome {
            Ok(rows) => refreshed += rows,
            Err(message) => errors.push(format!("[{}] {message}", block.id)),
        }
    }

    json_cstring_or_null(
        serde_json::to_string(&serde_json::json!({
            "refreshed": refreshed,
            "errors": errors,
        }))
        .ok(),
    )
}

/// Runs a block's declared `preview` against the selected row and returns its
/// output as `{rows, error}`-shaped text, or `null` when it declares none.
pub(crate) fn look_source_preview_json_impl(
    candidate_id: *const c_char,
    row_id: *const c_char,
    row_title: *const c_char,
    row_path: *const c_char,
) -> *mut c_char {
    let candidate_id = cstr_to_string(candidate_id);
    let Some(block_id) = CandidateIdKind::source_id_of(&candidate_id) else {
        return json_cstring_or_null(None);
    };
    let Some(block) = find_block(block_id) else {
        return json_cstring_or_null(None);
    };
    let Some(preview) = block.preview.as_deref() else {
        return json_cstring_or_null(None);
    };

    let row = row_context(row_id, row_title, row_path, "");
    let command = look_sources::expand(preview, &row);
    let text = look_sources::capture(
        &command,
        Some(&row.path),
        DEFAULT_CAPTURE_TIMEOUT,
        MAX_CAPTURE_BYTES,
    );

    json_cstring_or_null(
        serde_json::to_string(&match text {
            Ok(text) => serde_json::json!({ "text": text, "error": serde_json::Value::Null }),
            Err(message) => serde_json::json!({ "text": "", "error": message }),
        })
        .ok(),
    )
}

/// What Enter will run: a bundle's steps, or the `open` verb that acts on a
/// row the block produced. Either way the panel shows the real commands.
/// The selected row as a user's command sees it.
///
/// `{id}` is the row's OWN id, never the namespaced candidate id: a script
/// asking for a branch expects `main`, and handing it `src:branches:main` makes
/// git read the whole thing as `rev:path`.
fn row_context(
    row_id: *const c_char,
    row_title: *const c_char,
    row_path: *const c_char,
    query: &str,
) -> RowContext {
    let candidate_id = cstr_to_string(row_id);
    RowContext {
        id: CandidateIdKind::source_row_id_of(&candidate_id).to_string(),
        title: cstr_to_string(row_title),
        path: cstr_to_string(row_path),
        query: query.to_string(),
    }
}

fn steps_of(block: &Block) -> Vec<String> {
    match &block.producer {
        Producer::Bundle { steps } => steps.clone(),
        _ => block.verbs.open.iter().cloned().collect(),
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
