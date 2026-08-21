//! One action resolved against one row, in the shape every shell decodes.
//!
//! The wire contract lives here rather than in each shell's transport layer so
//! there is exactly one definition of it: the C FFI hands macOS this JSON and
//! the Tauri commands hand linows the same JSON, and a new [`Launch`] variant
//! cannot reach one shell without reaching the other.

use serde::Serialize;

use crate::{Action, Launch, Target, Tools, Unavailable};

/// Composed shell text. Only a resolve-only call returns this; performing runs
/// it and reports [`KIND_PERFORMED`] or [`KIND_FAILED`].
pub const KIND_SHELL: &str = "shell";
/// The shell launches `tool` with `path`, since only native code can find and
/// start an application.
pub const KIND_APPLICATION: &str = "application";
/// Nothing declared, so the platform's own handler does it.
pub const KIND_SYSTEM_DEFAULT: &str = "system_default";
/// No tool declared, or one that cannot do this.
pub const KIND_UNAVAILABLE: &str = "unavailable";
/// Already run; the caller has nothing left to do.
pub const KIND_PERFORMED: &str = "performed";
/// Tried, and the spawn itself failed.
pub const KIND_FAILED: &str = "failed";

/// Flat rather than a tagged union so a shell can decode it with a plain
/// struct: `kind` says which of the other fields are filled.
#[derive(Debug, Default, Serialize)]
pub struct Resolved {
    pub kind: &'static str,
    /// The tool that will start, for the label and the installed check.
    pub tool: Option<String>,
    /// For `shell`: the line to run through the login shell.
    pub command: Option<String>,
    /// For `application` and `system_default`: the path to act on.
    pub path: Option<String>,
    /// For `unavailable` and `failed`: what to show the user.
    pub reason: Option<String>,
    /// For `unavailable`: the config key that would fix it, when one would.
    pub key: Option<String>,
}

impl Resolved {
    pub fn performed(tool: Option<String>) -> Self {
        Self {
            kind: KIND_PERFORMED,
            tool,
            ..Self::default()
        }
    }

    pub fn failed(tool: Option<String>, reason: String) -> Self {
        Self {
            kind: KIND_FAILED,
            tool,
            reason: Some(reason),
            ..Self::default()
        }
    }

    /// Whether the shell that performed this has an error to show.
    pub fn is_failure(&self) -> bool {
        self.kind == KIND_FAILED
    }
}

impl From<Result<Launch, Unavailable>> for Resolved {
    fn from(outcome: Result<Launch, Unavailable>) -> Self {
        let launch = match outcome {
            Ok(launch) => launch,
            Err(unavailable) => {
                return Self {
                    kind: KIND_UNAVAILABLE,
                    reason: Some(unavailable.message()),
                    key: unavailable.key().map(str::to_string),
                    ..Self::default()
                };
            }
        };

        // The tool is the same question for every launch, so it is asked once
        // rather than per variant.
        let tool = launch.tool().map(str::to_string);
        match launch {
            Launch::Shell { command, .. } => Self {
                kind: KIND_SHELL,
                tool,
                command: Some(command),
                ..Self::default()
            },
            Launch::Application { path, .. } => Self {
                kind: KIND_APPLICATION,
                tool,
                path: Some(path),
                ..Self::default()
            },
            Launch::SystemDefault { path } => Self {
                kind: KIND_SYSTEM_DEFAULT,
                path: Some(path),
                ..Self::default()
            },
        }
    }
}

/// What `action` does to the row at `path`, or `None` when the action is
/// unknown or the row has no path behind it.
///
/// `is_dir` comes from the row's kind rather than the filesystem: the shell
/// already knows, and [`Target::dir`] answers lexically for the same reason -
/// naming a menu entry must not cost a `stat`.
pub fn resolve(
    action: &str,
    path: &str,
    is_dir: bool,
    tools: &Tools,
) -> Option<Result<Launch, Unavailable>> {
    let action = Action::from_id(action)?;
    if path.trim().is_empty() {
        return None;
    }
    let target = if is_dir {
        Target::Folder(path.to_string())
    } else {
        Target::File(path.to_string())
    };
    Some(action.resolve(tools, &target))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key;

    fn json(outcome: Result<Launch, Unavailable>) -> serde_json::Value {
        serde_json::to_value(Resolved::from(outcome)).expect("serialize")
    }

    #[test]
    fn a_shell_launch_carries_its_command() {
        let value = json(Ok(Launch::Shell {
            tool: "ghostty".into(),
            command: "'ghostty' -e true".into(),
        }));

        assert_eq!(value["kind"], KIND_SHELL);
        assert_eq!(value["tool"], "ghostty");
        assert_eq!(value["command"], "'ghostty' -e true");
        assert!(value["path"].is_null());
        assert!(value["reason"].is_null());
    }

    #[test]
    fn an_application_launch_carries_its_path_instead() {
        let value = json(Ok(Launch::Application {
            tool: "zed".into(),
            path: "/tmp/look".into(),
        }));

        assert_eq!(value["kind"], KIND_APPLICATION);
        assert_eq!(value["tool"], "zed");
        assert_eq!(value["path"], "/tmp/look");
        assert!(value["command"].is_null());
    }

    #[test]
    fn a_system_default_carries_only_the_path() {
        let value = json(Ok(Launch::SystemDefault {
            path: "/tmp/look".into(),
        }));

        assert_eq!(value["kind"], KIND_SYSTEM_DEFAULT);
        assert_eq!(value["path"], "/tmp/look");
        assert!(value["tool"].is_null());
    }

    #[test]
    fn an_unavailable_action_carries_a_reason_and_the_key_to_set() {
        let value = json(Err(Unavailable::NotDeclared {
            key: key::TEXT_EDITOR,
        }));

        assert_eq!(value["kind"], KIND_UNAVAILABLE);
        assert_eq!(value["key"], key::TEXT_EDITOR);
        assert!(
            value["reason"]
                .as_str()
                .is_some_and(|reason| reason.contains(key::TEXT_EDITOR))
        );
    }

    #[test]
    fn a_reason_with_no_fix_still_explains_itself() {
        let value = json(Err(Unavailable::CannotRunCommand {
            tool: "warp".into(),
        }));

        assert_eq!(value["kind"], KIND_UNAVAILABLE);
        assert!(value["key"].is_null());
        assert!(
            value["reason"]
                .as_str()
                .is_some_and(|reason| reason.contains("warp"))
        );
    }

    #[test]
    fn only_a_failure_reads_as_one() {
        assert!(Resolved::failed(None, "boom".into()).is_failure());
        assert!(!Resolved::performed(Some("zed".into())).is_failure());
        assert!(!Resolved::from(Ok(Launch::SystemDefault { path: "/".into() })).is_failure());
    }

    /// The guards every shell would otherwise write for itself.
    #[test]
    fn an_unusable_request_resolves_to_nothing() {
        let tools = Tools::default();
        assert!(resolve("frobnicate", "/tmp/a.txt", false, &tools).is_none());
        assert!(resolve("edit", "   ", false, &tools).is_none());
        assert!(resolve("reveal", "/tmp/a.txt", false, &tools).is_some());
    }

    /// A folder row and a file row of the same path resolve differently, which
    /// is the whole reason `is_dir` is passed rather than looked up.
    #[test]
    fn the_row_kind_picks_the_target() {
        let mut tools = Tools::default();
        tools.set(key::TEXT_EDITOR, "zed");
        tools.set(key::CODE_EDITOR, "cursor");

        let file = Resolved::from(resolve("edit", "/tmp/look", false, &tools).unwrap());
        let folder = Resolved::from(resolve("edit", "/tmp/look", true, &tools).unwrap());

        assert_eq!(file.tool.as_deref(), Some("zed"));
        assert_eq!(folder.tool.as_deref(), Some("cursor"));
    }
}
