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
use look_tools::{Action, Launch, Resolved};
use std::os::raw::c_char;

use crate::state::{cstr_to_string, json_cstring_or_null};

/// Resolves `action` ("edit", "terminal", "reveal") against a row, or the JSON
/// literal `null` when the action is unknown or the path is empty.
pub(crate) fn look_tool_action_json_impl(
    action: *const c_char,
    path: *const c_char,
    is_dir: bool,
) -> *mut c_char {
    json_cstring_or_null(
        resolve(action, path, is_dir)
            .map(Resolved::from)
            .as_ref()
            .and_then(json),
    )
}

/// Resolves `action` and, when it composes to shell text, runs it detached
/// through `look_sources::run`, which already owns process groups and login
/// shell selection.
///
/// An `application` result comes back untouched: finding and starting a bundle
/// is the native side's job (`specs/preferred-tools.md` §8).
pub(crate) fn look_perform_tool_action_json_impl(
    action: *const c_char,
    path: *const c_char,
    is_dir: bool,
) -> *mut c_char {
    let resolved = resolve(action, path, is_dir).map(|outcome| match outcome {
        Ok(Launch::Shell { tool, command }) => performed(&tool, command),
        other => Resolved::from(other),
    });

    json_cstring_or_null(resolved.as_ref().and_then(json))
}

fn resolve(
    action: *const c_char,
    path: *const c_char,
    is_dir: bool,
) -> Option<Result<Launch, look_tools::Unavailable>> {
    look_tools::resolve(
        &cstr_to_string(action),
        &cstr_to_string(path),
        is_dir,
        &RuntimeConfig::tools_cached(),
    )
}

fn json(resolved: &Resolved) -> Option<String> {
    serde_json::to_string(resolved).ok()
}

fn performed(tool: &str, command: String) -> Resolved {
    let error = look_sources::perform(&[command], None)
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
