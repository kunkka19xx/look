//! Preferred tools: what the actions on one row resolve to, and running the
//! half of it core deliberately does not.
//!
//! Composition and the resolved shape live in `look_tools` and are shared with
//! macOS, so both shells drive the same terminals the same way and word an
//! unavailable action identically. What is left here is the part only a native
//! shell can do: find and start a named application, and reveal a path.
//!
//! The tools come from the cached config, so an edited `~/.look/config` is
//! picked up by the same reload every other setting goes through.

use look_engine::config::RuntimeConfig;
use look_sources::RowContext;
use look_tools::{Launch, Resolved};

use crate::sources::RowArgs;

#[cfg(target_os = "linux")]
use crate::platform::linux::tools as platform_tools;
#[cfg(target_os = "windows")]
use crate::platform::windows::tools as platform_tools;

/// Said when a tool is declared but nothing by that name can be started.
const LAUNCH_FAILED: &str = "Could not start";

/// Resolve several actions against one row without acting, which is what names
/// the menu entries ("Edit in Zed") and what explains one that cannot run.
///
/// Batched because the menu asks about every action at once: one hop and one
/// config read instead of one per entry. An action that does not apply comes
/// back as `null` in its own slot rather than shifting the rest.
#[tauri::command(async)]
pub fn tool_actions(
    actions: Vec<String>,
    row: RowArgs,
    is_dir: Option<bool>,
) -> Vec<Option<Resolved>> {
    let request = Request::read(row, is_dir);
    actions
        .iter()
        .map(|action| {
            request
                .resolve(action)
                .map(|outcome| request.mark(action, Resolved::from(outcome)))
        })
        .collect()
}

/// Resolve `action` and carry it out.
#[tauri::command]
pub async fn perform_tool_action(
    window: tauri::WebviewWindow,
    action: String,
    row: RowArgs,
    is_dir: Option<bool>,
) -> Option<Resolved> {
    let request = Request::read(row, is_dir);
    let launch = match request.resolve(&action)? {
        Ok(launch) => launch,
        // Nothing to run, so the window stays up behind the banner saying why.
        Err(unavailable) => return Some(request.mark(&action, Err(unavailable).into())),
    };

    crate::commands::hide_armed(&window);
    // Spawning a process is blocking work, so it stays off the async runtime.
    let row = request.row.clone();
    let outcome = tauri::async_runtime::spawn_blocking(move || perform(launch, &row))
        .await
        .ok()?;

    // A tool that could not start leaves the user looking at their desktop with
    // no explanation, so bring the launcher back to carry the banner.
    if outcome.is_failure() {
        crate::commands::show_launcher(&window);
        crate::commands::focus_launcher(&window);
    }
    Some(request.mark(&action, outcome))
}

/// One row and the tools in play, read once so resolving an action and
/// performing it cannot answer differently for the same press.
///
/// The block a row came from is what makes this more than a `look_tools` call:
/// its own `edit` / `terminal` / `reveal` wins for its own rows, expanded like
/// every other command it declares.
struct Request {
    block: Option<look_sources::Block>,
    row: RowContext,
    path: String,
    is_dir: bool,
}

impl Request {
    fn read(row: RowArgs, is_dir: Option<bool>) -> Self {
        // An ordinary file never pays for reading the sources directory.
        let block = look_engine::sources::block_for_candidate(&row.candidate_id);
        let path = row.row_path.clone();
        // A file row and a folder row say which they are; a block's row does
        // not, so the filesystem is what answers. Editing resolves to a
        // different tool for each and a terminal opens in a different place,
        // so guessing is not an option.
        let is_dir = is_dir.unwrap_or_else(|| std::path::Path::new(&path).is_dir());
        Self {
            block,
            // A chord carries no query: `{query}` is what the user typed to
            // reach a row, and Ctrl+E is not that.
            row: row.context_without_query(),
            path,
            is_dir,
        }
    }

    fn resolve(&self, action: &str) -> Option<Result<Launch, look_tools::Unavailable>> {
        look_sources::resolve_for_row(
            action,
            &self.path,
            self.is_dir,
            &RuntimeConfig::tools_cached(),
            self.block.as_ref(),
            &self.row,
        )
    }

    /// Says whether the block took this chord, which is what the label needs.
    fn mark(&self, action: &str, mut resolved: Resolved) -> Resolved {
        resolved.from_block = look_sources::block_declares(self.block.as_ref(), action);
        resolved
    }
}

/// With the row, like every other command a block declares: same working
/// directory, same `LOOK_*` environment.
fn perform(launch: Launch, row: &RowContext) -> Resolved {
    match launch {
        Launch::Shell { tool, command } => {
            // Through core's runner, which already owns login-shell selection
            // and detaching, so a terminal outlives the launcher that started
            // it. The window it makes is a new one, and every desktop focuses
            // those itself.
            match look_sources::perform(&[command], Some(row))
                .into_iter()
                .find_map(|step| step.error)
            {
                None => Resolved::performed(Some(tool)),
                Some(reason) => Resolved::failed(Some(tool), reason),
            }
        }
        // No `activate`: a terminal makes a new window, which the desktop
        // focuses itself. Only an editor reusing an existing window needs it.
        Launch::Argv { tool, args, cwd } => match platform_tools::launch_argv(&tool, &args, &cwd) {
            Ok(()) => Resolved::performed(Some(tool)),
            Err(detail) => {
                eprintln!("[tools] launching {tool:?} failed: {detail}");
                Resolved::failed(Some(tool.clone()), format!("{LAUNCH_FAILED} {tool}"))
            }
        },
        Launch::Application { tool, path } => match platform_tools::launch(&tool, &path) {
            Ok(()) => {
                platform_tools::activate(&tool);
                Resolved::performed(Some(tool))
            }
            Err(detail) => {
                // The detail names the mechanism that failed, which is for the
                // log; the banner names the tool, which is what the user set.
                eprintln!("[tools] launching {tool:?} failed: {detail}");
                Resolved::failed(Some(tool.clone()), format!("{LAUNCH_FAILED} {tool}"))
            }
        },
        Launch::SystemDefault { path } => match platform_tools::reveal(&path) {
            Ok(()) => Resolved::performed(None),
            Err(reason) => Resolved::failed(None, reason),
        },
    }
}
