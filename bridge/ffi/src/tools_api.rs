//! C-ABI wrapper over `look_tools`, so a shell can ask what one action on one
//! row resolves to without knowing anything about terminals or editors.
//!
//! Transport only: the resolved shape, its `kind` strings, and the guards on an
//! unusable request all live in `look_tools::Resolved`, so this and the Tauri
//! commands hand their shells the same JSON by construction.
//!
//! The tools are read from the cached config, so a reload picks up an edited
//! `~/.look/config` with no extra call from the shell.

use look_engine::config::RuntimeConfig;
use look_indexing::CandidateIdKind;
use look_sources::{Block, RowContext};
use look_tools::{Action, Launch, Resolved};
use std::os::raw::c_char;

use crate::sources_api::{find_block, parents_from};
use crate::state::{cstr_to_string, json_cstring_or_null};

/// Resolves `action` ("edit", "terminal", "reveal") against a row, or the JSON
/// literal `null` when the action is unknown or the path is empty.
pub(crate) fn look_tool_action_json_impl(
    action: *const c_char,
    candidate_id: *const c_char,
    row_title: *const c_char,
    path: *const c_char,
    is_dir: bool,
    ancestors_json: *const c_char,
) -> *mut c_char {
    let request = Request::read(action, candidate_id, row_title, path, is_dir, ancestors_json);

    json_cstring_or_null(
        request
            .resolve()
            .map(|outcome| request.mark(Resolved::from(outcome)))
            .as_ref()
            .and_then(json),
    )
}

/// Resolves `action` and, when it composes to shell text, runs it detached
/// through `look_sources::run`, which already owns process groups and login
/// shell selection.
///
/// An `application` result comes back untouched: finding and starting a bundle
/// is the native side's job.
pub(crate) fn look_perform_tool_action_json_impl(
    action: *const c_char,
    candidate_id: *const c_char,
    row_title: *const c_char,
    path: *const c_char,
    is_dir: bool,
    ancestors_json: *const c_char,
) -> *mut c_char {
    let request = Request::read(action, candidate_id, row_title, path, is_dir, ancestors_json);

    let resolved = request.resolve().map(|outcome| {
        let resolved = match outcome {
            Ok(Launch::Shell { tool, command }) => performed(&tool, command, &request.row),
            other => Resolved::from(other),
        };
        request.mark(resolved)
    });

    json_cstring_or_null(resolved.as_ref().and_then(json))
}

/// One action against one row, read once and shared by resolve and perform so
/// they cannot answer differently for the same press.
struct Request {
    action: String,
    /// The block that produced the row, when one did. Its `edit` / `terminal` /
    /// `reveal` wins for its own rows.
    block: Option<Block>,
    row: RowContext,
    path: String,
    is_dir: bool,
}

impl Request {
    fn read(
        action: *const c_char,
        candidate_id: *const c_char,
        row_title: *const c_char,
        path: *const c_char,
        is_dir: bool,
        ancestors_json: *const c_char,
    ) -> Self {
        let candidate_id = cstr_to_string(candidate_id);
        let path = cstr_to_string(path);
        // An ordinary file never pays for reading the sources directory.
        let block = CandidateIdKind::source_id_of(&candidate_id).and_then(find_block);

        Self {
            action: cstr_to_string(action),
            block,
            row: RowContext {
                id: CandidateIdKind::source_row_id_of(&candidate_id).to_string(),
                title: cstr_to_string(row_title),
                path: path.clone(),
                // A chord carries no query: `{query}` is what the user typed to
                // reach a row, and Cmd+E is not that.
                query: String::new(),
                parents: parents_from(ancestors_json),
            },
            path,
            is_dir,
        }
    }

    fn resolve(&self) -> Option<Result<Launch, look_tools::Unavailable>> {
        look_sources::resolve_for_row(
            &self.action,
            &self.path,
            self.is_dir,
            &RuntimeConfig::tools_cached(),
            self.block.as_ref(),
            &self.row,
        )
    }

    /// Says whether the block took this chord, which is what the label needs.
    fn mark(&self, mut resolved: Resolved) -> Resolved {
        resolved.from_block = look_sources::block_declares(self.block.as_ref(), &self.action);
        resolved
    }
}

fn json(resolved: &Resolved) -> Option<String> {
    serde_json::to_string(resolved).ok()
}

/// With the row, like every other command a block declares: same working
/// directory, same `LOOK_*` environment.
fn performed(tool: &str, command: String, row: &RowContext) -> Resolved {
    let error = look_sources::perform(&[command], Some(row))
        .into_iter()
        .find_map(|outcome| outcome.error);

    match error {
        None => Resolved::performed(Some(tool.to_string())),
        Some(reason) => Resolved::failed(Some(tool.to_string()), reason),
    }
}

/// Every action id the shells may pass, so a menu can be built without
/// hardcoding the list twice.
pub(crate) fn look_tool_actions_json_impl() -> *mut c_char {
    let ids: Vec<&str> = Action::ALL.iter().map(|action| action.id()).collect();
    json_cstring_or_null(serde_json::to_string(&ids).ok())
}
