//! Quick Actions - the linows half of the shared framework (see
//! docs/writing-controls.md and the macOS peer in
//! apps/macos/.../Support/QuickActions/).
//!
//! Descriptors (which actions exist, their labels, control kind, info fields)
//! come from the shared `look-qactions` catalog; this module supplies the
//! native side: the `SystemControl` adapter contract, the registry resolving
//! an `action_id` to its adapter, and the Tauri commands the frontend calls.
//! Adapters block (D-Bus, CLIs), so state/apply run on the blocking pool,
//! mirroring `answers.rs`.

// Adapters exist for Linux and Windows (see `controls`); on any other target
// nothing constructs the success-path states/outcomes/values of the shared
// types below - they exist only to serialize back to the frontend. Silence the
// resulting dead_code lint there; a future adapter would use them and this
// lifts on its own.
#![cfg_attr(not(any(target_os = "linux", target_os = "windows")), allow(dead_code))]

pub mod controls;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::async_runtime;

/// Current state of a control's value, read for display in the panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum ActionState {
    On,
    Off,
    /// A non-boolean value shown as-is. Button controls (Screensaver, Restart,
    /// Shut Down) return it empty: they have no on/off value, but a resolved
    /// state marks them present so the launchpad renders them wired. Battery
    /// returns the charge percent here.
    // Constructed on Linux and Windows; dead only on a target with no adapters.
    #[cfg_attr(not(any(target_os = "linux", target_os = "windows")), allow(dead_code))]
    Value {
        value: String,
    },
    /// The control cannot act here: no hardware, no service, unsupported OS.
    Unavailable {
        reason: String,
    },
}

/// What the user asked a control to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionIntent {
    /// Flip a boolean control against its live state (on <-> off).
    Toggle,
    /// Drive a boolean control to an explicit target. The panel resolves a
    /// toggle press to this against the state it is showing, so a press does
    /// what the user sees even when the panel is stale (the system changed
    /// while the window was hidden); a blind `Toggle` would flip the live state
    /// and do the opposite. Wire form: `{ "set_on": true }`.
    SetOn(bool),
    /// Trigger a non-toggle action (a plain button).
    Run,
}

/// Result of applying an intent, surfaced to the user as a banner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ActionOutcome {
    /// Success. `banner` overrides the default confirmation text.
    Ok {
        banner: Option<String>,
    },
    Failed {
        message: String,
    },
    /// Part of the shared adapter contract; no linows control needs an OS
    /// permission yet.
    #[allow(dead_code)]
    NeedsPermission {
        message: String,
    },
}

/// `info()` key for the Battery adapters' charging flag. Battery is a
/// presentational tile with no shared `core/qactions` descriptor, so this key
/// is linows-only; defined once here rather than in each per-OS adapter file
/// so the Linux and Windows adapters can't drift out of sync on the spelling.
pub const BATTERY_CHARGING_INFO_KEY: &str = "charging";

/// A resolved info-field value. The shared descriptor declares `label` +
/// `value_key`; the adapter resolves the key to what to display.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum InfoValue {
    Text {
        text: String,
    },
    /// A set of items the panel renders one-per-row (e.g. each paired Bluetooth
    /// device), instead of squeezing them into a single value.
    List {
        items: Vec<ListItem>,
    },
    Unavailable {
        reason: String,
    },
}

/// One entry in an [`InfoValue::List`]. An `id` makes the row actionable via
/// [`SystemControl::apply_item`]; `on` drives an on/off marker (e.g. whether a
/// device is currently connected). Both are optional so a list can be plain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ListItem {
    pub id: Option<String>,
    pub label: String,
    pub on: Option<bool>,
}

/// The adapter a contributor implements per control - the one linows file you
/// write when adding one (see docs/writing-controls.md). Methods may block;
/// they always run off the UI thread. Best-effort: never panic, surface
/// problems as `Unavailable` / `Failed` / `NeedsPermission`.
pub trait SystemControl: Send + Sync {
    /// Read the current state for display.
    fn state(&self) -> ActionState;

    /// Resolve the descriptor's info `value_key`s to display values. Controls
    /// without info fields keep the default.
    fn info(&self, _keys: &[String]) -> HashMap<String, InfoValue> {
        HashMap::new()
    }

    /// Perform `intent` and report the outcome.
    fn apply(&self, intent: ActionIntent) -> ActionOutcome;

    /// Act on one item of a list-valued info field (e.g. connect/disconnect a
    /// specific device). Defaults to unsupported: most controls have no
    /// per-item actions.
    fn apply_item(&self, _item_id: &str, _intent: ActionIntent) -> ActionOutcome {
        ActionOutcome::Failed {
            message: "No per-item action".to_string(),
        }
    }
}

/// Resolves an action id to its native adapter - the one-line-per-control
/// registry. An id with a shared descriptor but no adapter here renders as
/// unavailable (declaration is shared across OSes; execution is not).
#[cfg(target_os = "linux")]
fn adapter(action_id: &str) -> Option<&'static dyn SystemControl> {
    use look_qactions::action_id as id;
    match action_id {
        id::BLUETOOTH => Some(&controls::bluetooth::BluetoothControl),
        id::WIFI => Some(&controls::wifi::WifiControl),
        id::THEME => Some(&controls::theme::ThemeControl),
        id::KEEP_AWAKE => Some(&controls::keepawake::KeepAwakeControl),
        id::MIC => Some(&controls::mic::MicControl),
        id::SCREENSAVER => Some(&controls::screensaver::ScreensaverControl),
        id::RESTART => Some(&controls::power::RestartControl),
        id::SHUTDOWN => Some(&controls::power::ShutdownControl),
        id::BATTERY => Some(&controls::battery::BatteryControl),
        _ => None,
    }
}

#[cfg(target_os = "windows")]
fn adapter(action_id: &str) -> Option<&'static dyn SystemControl> {
    use look_qactions::action_id as id;
    match action_id {
        id::BLUETOOTH => Some(&controls::bluetooth_windows::BluetoothControl),
        id::WIFI => Some(&controls::wifi_windows::WifiControl),
        id::THEME => Some(&controls::theme_windows::ThemeControl),
        id::KEEP_AWAKE => Some(&controls::keepawake_windows::KeepAwakeControl),
        id::MIC => Some(&controls::mic_windows::MicControl),
        id::SCREENSAVER => Some(&controls::screensaver_windows::ScreensaverControl),
        id::RESTART => Some(&controls::power_windows::RestartControl),
        id::SHUTDOWN => Some(&controls::power_windows::ShutdownControl),
        id::BATTERY => Some(&controls::battery_windows::BatteryControl),
        _ => None,
    }
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn adapter(_action_id: &str) -> Option<&'static dyn SystemControl> {
    None
}

const UNAVAILABLE_ON_OS: &str = "Not supported on this system";

/// One action's live state plus its resolved info values, fetched together so
/// a selection costs a single IPC round trip.
#[derive(Serialize)]
pub struct QuickActionStatus {
    pub state: ActionState,
    pub info: HashMap<String, InfoValue>,
}

fn unavailable_status() -> QuickActionStatus {
    QuickActionStatus {
        state: ActionState::Unavailable {
            reason: UNAVAILABLE_ON_OS.to_string(),
        },
        info: HashMap::new(),
    }
}

/// Quick Action descriptors for a selected result, from the shared catalog.
/// Empty for results with no actions (the common case).
#[tauri::command]
pub fn quick_actions(result_id: String, kind: String) -> Vec<look_qactions::ActionDescriptor> {
    look_qactions::descriptors_for(&result_id, &kind)
}

/// A tile icon that names a file rather than one of the shell's glyphs, with
/// `~` expanded.
///
/// Done here rather than in the window: the core passes the name through
/// untouched (macOS reads it as an SF Symbol), and the frontend has no home
/// directory to expand against, so this is the one place that knows both.
fn resolve_icon(icon: Option<String>) -> Option<String> {
    resolve_icon_in(icon, crate::files::get_home_dir())
}

/// How big a tile icon may be: a glyph at 16px, not an illustration. Read on
/// every layout fetch, so this is the ceiling on what that costs.
const MAX_ICON_BYTES: u64 = 256 * 1024;

/// The resolution itself, against a given home, so it is testable without
/// touching the process environment.
///
/// Inlined as a `data:` URL rather than served over the asset protocol: the
/// window draws a file icon as a CSS mask, and a custom URI scheme there is the
/// one place WebKitGTK is unreliable about them. Inlining also frees the file
/// from the protocol's `$HOME` scope, which is a rule a user has no way to see.
/// An icon is a few hundred bytes, so the copy costs nothing.
fn resolve_icon_in(icon: Option<String>, home: Option<String>) -> Option<String> {
    let icon = icon?;
    let path = match icon.strip_prefix("~/") {
        Some(rest) => format!("{}/{rest}", home?.trim_end_matches('/')),
        // Not a path at all: one of the window's own glyph names, which it
        // resolves for itself.
        None if !icon.starts_with('/') => return Some(icon),
        None => icon,
    };

    let size = match std::fs::metadata(&path) {
        Ok(meta) => meta.len(),
        Err(err) => {
            eprintln!("look: tile icon \"{path}\" cannot be read ({err})");
            return None;
        }
    };
    if size > MAX_ICON_BYTES {
        eprintln!("look: tile icon \"{path}\" is {size} bytes, over the {MAX_ICON_BYTES} limit");
        return None;
    }

    // Same reader the app-icon pipeline uses, so a tile accepts the formats an
    // icon theme does and refuses the ones a window cannot draw.
    let inlined = crate::platform::shared::read_icon_file(&path);
    if inlined.is_none() {
        eprintln!("look: tile icon \"{path}\" is not an image the window can draw");
    }
    inlined
}

/// What each user tile currently shows. Reads the cache; runs nothing.
#[tauri::command]
pub fn launchpad_tile_values()
-> std::collections::HashMap<String, look_engine::launchpad::TileValue> {
    let mut values = look_engine::launchpad_values::cached();
    // A reading may carry an icon of its own, so it needs the same expansion
    // the layout's does.
    for value in values.values_mut() {
        value.icon = resolve_icon(value.icon.take());
    }
    values
}

/// Re-runs stale tile commands. Spawns, so `async`.
///
/// Returns `refreshed` as well as the errors: the frontend re-reads the values
/// only when something actually ran, which is the uncommon case.
#[tauri::command(async)]
pub fn refresh_launchpad_tiles() -> (usize, Vec<String>) {
    let outcome = look_engine::launchpad_values::refresh();
    (outcome.refreshed, outcome.errors)
}

/// Runs a user tile's press, named by the tile.
#[tauri::command(async)]
pub fn press_launchpad_tile(name: String) -> Option<String> {
    look_engine::launchpad_values::press(&name).err()
}

/// The empty-state launchpad's tile layout: the user's `~/.look/super-actions.toml`
/// when they have one, else the shared catalog's default.
///
/// One source of truth across shells, and resolved entirely in the core - the
/// frontend receives tiles that already know their cell, and never parses the
/// drawing or works out a span for itself. A file that cannot be trusted falls
/// back to the default rather than rendering an empty strip.
#[tauri::command]
pub fn launchpad_layout() -> look_engine::launchpad::LayoutPayload {
    let mut payload = look_engine::launchpad::layout_payload();
    for tile in &mut payload.tiles {
        tile.icon = resolve_icon(tile.icon.take());
    }
    payload
}

/// Anything wrong with `~/.look/super-actions.toml`, or empty when it is fine.
///
/// Its own command rather than a field beside the tiles - the same split
/// `qactions_api` makes for the FFI shell.
/// Resolves silently: `launchpad_layout` has already printed these to stderr,
/// and this puts them where a user who did not launch from a terminal can see.
#[tauri::command]
pub fn launchpad_warnings() -> Vec<String> {
    look_engine::launchpad::layout().warnings
}

/// Live state + info values for an action. `info_keys` are the descriptor's
/// `value_key`s the frontend wants resolved.
#[tauri::command]
pub async fn quick_action_state(action_id: String, info_keys: Vec<String>) -> QuickActionStatus {
    async_runtime::spawn_blocking(move || match adapter(&action_id) {
        Some(control) => QuickActionStatus {
            state: control.state(),
            info: control.info(&info_keys),
        },
        None => unavailable_status(),
    })
    .await
    .unwrap_or_else(|_| unavailable_status())
}

/// Run an action's intent; the outcome feeds the banner.
#[tauri::command]
pub async fn quick_action_apply(action_id: String, intent: ActionIntent) -> ActionOutcome {
    async_runtime::spawn_blocking(move || match adapter(&action_id) {
        Some(control) => control.apply(intent),
        None => ActionOutcome::Failed {
            message: UNAVAILABLE_ON_OS.to_string(),
        },
    })
    .await
    .unwrap_or_else(|_| ActionOutcome::Failed {
        message: "Action failed".to_string(),
    })
}

/// Run an intent against one list item of an action (e.g. toggle a device's
/// connection). Like `quick_action_apply`, but targets an item by id.
#[tauri::command]
pub async fn quick_action_apply_item(
    action_id: String,
    item_id: String,
    intent: ActionIntent,
) -> ActionOutcome {
    async_runtime::spawn_blocking(move || match adapter(&action_id) {
        Some(control) => control.apply_item(&item_id, intent),
        None => ActionOutcome::Failed {
            message: UNAVAILABLE_ON_OS.to_string(),
        },
    })
    .await
    .unwrap_or_else(|_| ActionOutcome::Failed {
        message: "Action failed".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::resolve_icon_in;

    /// A directory of this test's own, so two tests never share a file.
    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("look-icon-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a scratch directory");
        dir
    }

    fn write(dir: &std::path::Path, name: &str, bytes: &[u8]) {
        std::fs::write(dir.join(name), bytes).expect("a scratch file");
    }

    const SVG: &[u8] = b"<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>";

    #[test]
    fn a_glyph_name_is_left_alone() {
        // Only a path is resolved here; "battery" is the window's own glyph and
        // must reach it spelled exactly as the user wrote it.
        assert_eq!(
            resolve_icon_in(Some("battery".into()), None),
            Some("battery".into())
        );
        assert_eq!(resolve_icon_in(None, None), None);
    }

    #[test]
    fn a_home_relative_icon_is_read_and_inlined() {
        // The window has no home directory of its own and no way to read a
        // file, so both have to happen here.
        let dir = scratch("home");
        write(&dir, "disk.svg", SVG);

        let resolved = resolve_icon_in(
            Some("~/disk.svg".into()),
            Some(dir.to_string_lossy().into_owned()),
        )
        .expect("an inlined icon");
        assert!(
            resolved.starts_with("data:image/svg+xml;base64,"),
            "{resolved}"
        );
    }

    #[test]
    fn a_trailing_slash_on_home_does_not_double_up() {
        let dir = scratch("slash");
        write(&dir, "disk.svg", SVG);

        assert!(
            resolve_icon_in(
                Some("~/disk.svg".into()),
                Some(format!("{}/", dir.to_string_lossy())),
            )
            .is_some()
        );
    }

    #[test]
    fn an_absolute_icon_needs_no_home() {
        let dir = scratch("absolute");
        write(&dir, "disk.svg", SVG);

        assert!(
            resolve_icon_in(
                Some(dir.join("disk.svg").to_string_lossy().into_owned()),
                None
            )
            .is_some()
        );
    }

    #[test]
    fn a_missing_file_draws_nothing_rather_than_a_path() {
        // The window would treat a leftover path as a glyph name, miss, and
        // draw nothing anyway - but silently, with no line saying why.
        assert_eq!(resolve_icon_in(Some("/nope/disk.svg".into()), None), None);
    }

    #[test]
    fn an_icon_over_the_size_limit_is_refused() {
        // A tile icon is drawn at 16px. Anything this big is a photograph, and
        // it would be re-encoded on every layout fetch.
        let dir = scratch("huge");
        write(
            &dir,
            "huge.png",
            &vec![0u8; super::MAX_ICON_BYTES as usize + 1],
        );

        assert_eq!(
            resolve_icon_in(
                Some(dir.join("huge.png").to_string_lossy().into_owned()),
                None
            ),
            None
        );
    }
}
