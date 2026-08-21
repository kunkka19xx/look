//! C-ABI wrapper over `look_tools`, so a shell can ask what one action on one
//! row resolves to without knowing anything about terminals or editors.
//!
//! The tools are read from the cached config, so a reload picks up an edited
//! `~/.look/config` with no extra call from the shell.

use look_engine::config::RuntimeConfig;
use look_tools::{Action, Launch, Target, Unavailable};
use serde::Serialize;
use std::os::raw::c_char;

use crate::state::{cstr_to_string, json_cstring_or_null};

const KIND_SHELL: &str = "shell";
const KIND_APPLICATION: &str = "application";
/// Nothing declared, so the platform's own handler does it.
const KIND_SYSTEM_DEFAULT: &str = "system_default";
const KIND_UNAVAILABLE: &str = "unavailable";
/// Core already ran it; the shell has nothing left to do.
const KIND_PERFORMED: &str = "performed";
/// Core tried and the spawn itself failed.
const KIND_FAILED: &str = "failed";

/// One action resolved against one row.
///
/// Flat rather than a tagged union so the shells can decode it with a plain
/// struct: `kind` says which of the other fields are filled.
#[derive(Serialize)]
struct ResolvedAction {
    kind: &'static str,
    /// The tool that will start, for the label and the installed check.
    tool: Option<String>,
    /// For `shell`: the line to run through the login shell.
    command: Option<String>,
    /// For `application`: the path to hand the platform launcher.
    path: Option<String>,
    /// For `unavailable`: what to show the user.
    reason: Option<String>,
    /// For `unavailable`: the config key that would fix it, when one would.
    key: Option<String>,
}

impl ResolvedAction {
    fn of(outcome: Result<Launch, Unavailable>) -> Self {
        match outcome {
            Ok(Launch::Shell { tool, command }) => Self {
                kind: KIND_SHELL,
                tool: Some(tool),
                command: Some(command),
                path: None,
                reason: None,
                key: None,
            },
            Ok(Launch::Application { tool, path }) => Self {
                kind: KIND_APPLICATION,
                tool: Some(tool),
                command: None,
                path: Some(path),
                reason: None,
                key: None,
            },
            Ok(Launch::SystemDefault { path }) => Self {
                kind: KIND_SYSTEM_DEFAULT,
                tool: None,
                command: None,
                path: Some(path),
                reason: None,
                key: None,
            },
            Err(unavailable) => Self {
                kind: KIND_UNAVAILABLE,
                tool: None,
                command: None,
                path: None,
                reason: Some(unavailable.message()),
                key: unavailable.key().map(str::to_string),
            },
        }
    }
}

/// Resolves `action` ("edit", "terminal") against a row, or the JSON literal
/// `null` when the action is unknown or the path is empty.
pub(crate) fn look_tool_action_json_impl(
    action: *const c_char,
    path: *const c_char,
    is_dir: bool,
) -> *mut c_char {
    let path = cstr_to_string(path);
    let Some(action) = Action::from_id(&cstr_to_string(action)) else {
        return json_cstring_or_null(None);
    };
    if path.trim().is_empty() {
        return json_cstring_or_null(None);
    }

    let target = if is_dir {
        Target::Folder(path)
    } else {
        Target::File(path)
    };
    let resolved = ResolvedAction::of(action.resolve(&RuntimeConfig::load_cached().tools, &target));

    json_cstring_or_null(serde_json::to_string(&resolved).ok())
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
    let path = cstr_to_string(path);
    let Some(action) = Action::from_id(&cstr_to_string(action)) else {
        return json_cstring_or_null(None);
    };
    if path.trim().is_empty() {
        return json_cstring_or_null(None);
    }

    let target = if is_dir {
        Target::Folder(path)
    } else {
        Target::File(path)
    };

    let resolved = match action.resolve(&RuntimeConfig::load_cached().tools, &target) {
        Ok(Launch::Shell { tool, command }) => performed(&tool, command),
        other => ResolvedAction::of(other),
    };

    json_cstring_or_null(serde_json::to_string(&resolved).ok())
}

fn performed(tool: &str, command: String) -> ResolvedAction {
    let error = look_sources::perform(&[command], None)
        .into_iter()
        .find_map(|outcome| outcome.error);

    match error {
        None => ResolvedAction {
            kind: KIND_PERFORMED,
            tool: Some(tool.to_string()),
            command: None,
            path: None,
            reason: None,
            key: None,
        },
        Some(reason) => ResolvedAction {
            kind: KIND_FAILED,
            tool: Some(tool.to_string()),
            command: None,
            path: None,
            reason: Some(reason),
            key: None,
        },
    }
}

/// Every action id the shells may pass, so a menu can be built without
/// hardcoding the list twice.
pub(crate) fn look_tool_actions_json_impl() -> *mut c_char {
    let ids: Vec<&str> = Action::ALL.iter().map(|action| action.id()).collect();
    json_cstring_or_null(serde_json::to_string(&ids).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use look_tools::key;

    fn json_of(outcome: Result<Launch, Unavailable>) -> serde_json::Value {
        serde_json::to_value(ResolvedAction::of(outcome)).expect("serialize")
    }

    #[test]
    fn a_shell_launch_carries_its_command() {
        let json = json_of(Ok(Launch::Shell {
            tool: "ghostty".into(),
            command: "'ghostty' -e true".into(),
        }));

        assert_eq!(json["kind"], KIND_SHELL);
        assert_eq!(json["tool"], "ghostty");
        assert_eq!(json["command"], "'ghostty' -e true");
        assert!(json["path"].is_null());
        assert!(json["reason"].is_null());
    }

    #[test]
    fn an_application_launch_carries_its_path_instead() {
        let json = json_of(Ok(Launch::Application {
            tool: "zed".into(),
            path: "/tmp/look".into(),
        }));

        assert_eq!(json["kind"], KIND_APPLICATION);
        assert_eq!(json["tool"], "zed");
        assert_eq!(json["path"], "/tmp/look");
        assert!(json["command"].is_null());
    }

    #[test]
    fn a_system_default_carries_only_the_path() {
        let json = json_of(Ok(Launch::SystemDefault {
            path: "/tmp/look".into(),
        }));

        assert_eq!(json["kind"], KIND_SYSTEM_DEFAULT);
        assert_eq!(json["path"], "/tmp/look");
        assert!(json["tool"].is_null());
        assert!(json["reason"].is_null());
    }

    #[test]
    fn an_unavailable_action_carries_a_reason_and_the_key_to_set() {
        let json = json_of(Err(Unavailable::NotDeclared {
            key: key::TEXT_EDITOR,
        }));

        assert_eq!(json["kind"], KIND_UNAVAILABLE);
        assert_eq!(json["key"], key::TEXT_EDITOR);
        assert!(
            json["reason"]
                .as_str()
                .is_some_and(|reason| reason.contains(key::TEXT_EDITOR))
        );
    }

    #[test]
    fn a_reason_with_no_fix_still_explains_itself() {
        let json = json_of(Err(Unavailable::CannotRunCommand {
            tool: "warp".into(),
        }));

        assert_eq!(json["kind"], KIND_UNAVAILABLE);
        assert!(json["key"].is_null());
        assert!(
            json["reason"]
                .as_str()
                .is_some_and(|reason| reason.contains("warp"))
        );
    }
}
