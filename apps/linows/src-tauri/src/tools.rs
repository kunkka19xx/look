//! Preferred tools: what one action on one row resolves to, and running the
//! half of it core deliberately does not.
//!
//! Composition lives in `look_tools` and is shared with macOS, so both shells
//! drive the same terminals the same way and word an unavailable action
//! identically. What is left here is the part only a native shell can do: find
//! and start a named application, and reveal a path.
//!
//! The tools come from the cached config, so an edited `~/.look.config` is
//! picked up by the same reload every other setting goes through.

use look_engine::config::RuntimeConfig;
use look_tools::{Action, Launch, Target, Unavailable};
use serde::Serialize;

#[cfg(target_os = "linux")]
use crate::platform::linux::tools as platform_tools;
#[cfg(target_os = "windows")]
use crate::platform::windows::tools as platform_tools;

const KIND_SHELL: &str = "shell";
const KIND_APPLICATION: &str = "application";
/// Nothing declared, so the platform's own handler does it.
const KIND_SYSTEM_DEFAULT: &str = "system_default";
const KIND_UNAVAILABLE: &str = "unavailable";
/// Already run; the frontend has nothing left to do but dismiss.
const KIND_PERFORMED: &str = "performed";
/// Tried, and the spawn itself failed.
const KIND_FAILED: &str = "failed";

/// Said when a tool is declared but nothing by that name can be started.
const LAUNCH_FAILED: &str = "Could not start";

/// One action resolved against one row.
///
/// Flat rather than a tagged union, and field-for-field the shape the C FFI
/// hands macOS: `kind` says which of the other fields are filled.
#[derive(Serialize)]
pub struct ResolvedAction {
    kind: &'static str,
    /// The tool that will start, for the menu label.
    tool: Option<String>,
    /// For `shell`: the line to run through the login shell.
    command: Option<String>,
    /// For `application` and `system_default`: the path to act on.
    path: Option<String>,
    /// For `unavailable` and `failed`: what to show the user.
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

    fn performed(tool: Option<String>) -> Self {
        Self {
            kind: KIND_PERFORMED,
            tool,
            command: None,
            path: None,
            reason: None,
            key: None,
        }
    }

    fn failed(tool: Option<String>, reason: String) -> Self {
        Self {
            kind: KIND_FAILED,
            tool,
            command: None,
            path: None,
            reason: Some(reason),
            key: None,
        }
    }
}

/// Resolve `action` against a row without acting, which is what names the menu
/// entry ("Edit in Zed") and what explains an action that cannot run.
#[tauri::command]
pub fn tool_action(action: String, path: String, is_dir: bool) -> Option<ResolvedAction> {
    let target = target(&action, &path, is_dir)?;
    Some(ResolvedAction::of(
        Action::from_id(&action)?.resolve(&RuntimeConfig::load_cached().tools, &target),
    ))
}

/// Resolve `action` and carry it out.
///
/// Blocking: every branch spawns a process, and the shell branch waits on the
/// login shell it starts.
#[tauri::command]
pub async fn perform_tool_action(
    window: tauri::WebviewWindow,
    action: String,
    path: String,
    is_dir: bool,
) -> Option<ResolvedAction> {
    let target = target(&action, &path, is_dir)?;
    let action = Action::from_id(&action)?;
    let launch = match action.resolve(&RuntimeConfig::load_cached().tools, &target) {
        Ok(launch) => launch,
        // Nothing to run, so the window stays up behind the banner saying why.
        Err(unavailable) => return Some(ResolvedAction::of(Err(unavailable))),
    };

    crate::commands::hide_armed(&window);
    let outcome = tauri::async_runtime::spawn_blocking(move || perform(launch))
        .await
        .ok()?;

    // A tool that could not start leaves the user looking at their desktop with
    // no explanation, so bring the launcher back to carry the banner.
    if outcome.kind == KIND_FAILED {
        crate::commands::show_launcher(&window);
        let _ = window.set_focus();
    }
    Some(outcome)
}

fn perform(launch: Launch) -> ResolvedAction {
    match launch {
        Launch::Shell { tool, command } => {
            // Through core's runner, which already owns login-shell selection
            // and detaching, so a terminal outlives the launcher that started
            // it. The window it makes is a new one, and every desktop focuses
            // those itself.
            match look_sources::perform(&[command], None)
                .into_iter()
                .find_map(|step| step.error)
            {
                None => ResolvedAction::performed(Some(tool)),
                Some(reason) => ResolvedAction::failed(Some(tool), reason),
            }
        }
        Launch::Application { tool, path } => match platform_tools::launch(&tool, &path) {
            Ok(()) => {
                platform_tools::activate(&tool);
                ResolvedAction::performed(Some(tool))
            }
            Err(detail) => {
                // The detail names the mechanism that failed, which is for the
                // log; the banner names the tool, which is what the user set.
                eprintln!("[tools] launching {tool:?} failed: {detail}");
                ResolvedAction::failed(Some(tool.clone()), format!("{LAUNCH_FAILED} {tool}"))
            }
        },
        Launch::SystemDefault { path } => match platform_tools::reveal(&path) {
            Ok(()) => ResolvedAction::performed(None),
            Err(reason) => ResolvedAction::failed(None, reason),
        },
    }
}

/// The row as core wants it, or `None` for a row with no path behind it.
///
/// `is_dir` comes from the row's kind rather than the filesystem: the frontend
/// already knows, and core answers `Target::dir()` lexically for the same
/// reason - resolving a menu label must not cost a `stat`.
fn target(action: &str, path: &str, is_dir: bool) -> Option<Target> {
    if action.trim().is_empty() || path.trim().is_empty() {
        return None;
    }
    Some(if is_dir {
        Target::Folder(path.to_string())
    } else {
        Target::File(path.to_string())
    })
}
