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
use look_tools::{Launch, Resolved};

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
pub fn tool_actions(actions: Vec<String>, path: String, is_dir: bool) -> Vec<Option<Resolved>> {
    let tools = RuntimeConfig::tools_cached();
    actions
        .iter()
        .map(|action| look_tools::resolve(action, &path, is_dir, &tools).map(Resolved::from))
        .collect()
}

/// Resolve `action` and carry it out.
#[tauri::command]
pub async fn perform_tool_action(
    window: tauri::WebviewWindow,
    action: String,
    path: String,
    is_dir: bool,
) -> Option<Resolved> {
    let launch = match look_tools::resolve(&action, &path, is_dir, &RuntimeConfig::tools_cached())?
    {
        Ok(launch) => launch,
        // Nothing to run, so the window stays up behind the banner saying why.
        Err(unavailable) => return Some(Err(unavailable).into()),
    };

    crate::commands::hide_armed(&window);
    // Spawning a process is blocking work, so it stays off the async runtime.
    let outcome = tauri::async_runtime::spawn_blocking(move || perform(launch))
        .await
        .ok()?;

    // A tool that could not start leaves the user looking at their desktop with
    // no explanation, so bring the launcher back to carry the banner.
    if outcome.is_failure() {
        crate::commands::show_launcher(&window);
        let _ = window.set_focus();
    }
    Some(outcome)
}

fn perform(launch: Launch) -> Resolved {
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
                None => Resolved::performed(Some(tool)),
                Some(reason) => Resolved::failed(Some(tool), reason),
            }
        }
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
