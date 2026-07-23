//! Quick Actions - shared action catalog
//!
//! This crate is the platform-neutral half of the framework: it declares WHICH
//! actions exist and their presentation (labels, control kind, info fields). It
//! knows nothing about how to execute them - that is a native `SystemControl`
//! adapter, keyed by `action_id`, in each platform shell.
//!
//! A contributor adds a control by (1) declaring its descriptor here, (2) binding
//! the platform result id(s) that trigger it, and (3) implementing the native
//! adapter. Only steps 1-2 live in this crate.

use serde::Serialize;

/// How an action's control renders in the panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ControlKind {
    /// A boolean on/off switch.
    Toggle,
    /// A plain trigger button.
    Button,
}

/// A read-only field shown above the actions. The core declares the label and a
/// `value_key`; the native adapter resolves the key to a live value for display.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InfoFieldSpec {
    pub label: String,
    pub value_key: String,
}

/// A declared action. Serialized to the platform shells; `action_id` selects the
/// native adapter that runs it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ActionDescriptor {
    pub action_id: String,
    pub title: String,
    pub control: ControlKind,
    pub on_label: Option<String>,
    pub off_label: Option<String>,
    pub info: Vec<InfoFieldSpec>,
}

/// Shared `action_id` constants. Kept as named constants (not inline literals)
/// so the descriptor catalog, the launchpad layout, and their tests all agree on
/// one spelling.
pub mod action_id {
    pub const BLUETOOTH: &str = "bluetooth";
    pub const WIFI: &str = "wifi";
    pub const THEME: &str = "theme";
    pub const FOCUS: &str = "focus";
    pub const SAVER: &str = "saver";
    pub const MIC: &str = "mic";
    pub const RESTART: &str = "restart";
    pub const SHUTDOWN: &str = "shutdown";
    pub const BATTERY: &str = "battery";
    pub const NOW_PLAYING: &str = "nowplaying";
    /// A read-only weather tile. Presentational like Battery: no descriptor and
    /// no native adapter, its value is resolved by each shell (see follow-up).
    pub const WEATHER: &str = "weather";
    /// The launchpad's L slot: a presentational Todo/Pomo/Clock rotation, not a
    /// system control, so it has no descriptor and no native adapter.
    pub const L_SLOT: &str = "lslot";
}

const STATUS_INFO_KEY: &str = "status";

fn toggle(action_id: &str, title: &str, on_label: &str, off_label: &str) -> ActionDescriptor {
    ActionDescriptor {
        action_id: action_id.to_string(),
        title: title.to_string(),
        control: ControlKind::Toggle,
        on_label: Some(on_label.to_string()),
        off_label: Some(off_label.to_string()),
        info: vec![InfoFieldSpec {
            label: "Status".to_string(),
            value_key: STATUS_INFO_KEY.to_string(),
        }],
    }
}

fn button(action_id: &str, title: &str) -> ActionDescriptor {
    ActionDescriptor {
        action_id: action_id.to_string(),
        title: title.to_string(),
        control: ControlKind::Button,
        on_label: None,
        off_label: None,
        info: vec![],
    }
}

/// The action definition for `action_id`, or `None` if unknown. This is the
/// shared, platform-neutral catalog - add new controls here.
pub fn descriptor(action_id: &str) -> Option<ActionDescriptor> {
    use crate::action_id as id;
    match action_id {
        id::BLUETOOTH => Some(toggle(id::BLUETOOTH, "Bluetooth", "On", "Off")),
        id::WIFI => Some(toggle(id::WIFI, "Wi-Fi", "On", "Off")),
        id::THEME => Some(toggle(id::THEME, "Theme", "Dark", "Light")),
        id::FOCUS => Some(toggle(id::FOCUS, "Focus", "On", "Off")),
        id::SAVER => Some(toggle(id::SAVER, "Saver", "On", "Off")),
        id::MIC => Some(toggle(id::MIC, "Mic", "On", "Muted")),
        id::RESTART => Some(button(id::RESTART, "Restart")),
        id::SHUTDOWN => Some(button(id::SHUTDOWN, "Shut Down")),
        _ => None,
    }
}

/// A launchpad tile's footprint in the 6-column bento grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TileSize {
    /// Large, 2 columns x 2 rows.
    L,
    /// Medium, 2 columns x 1 row (unless overridden by `span`).
    M,
    /// Small, 1 column x 1 row.
    S,
}

/// How a launchpad tile is drawn. `ControlKind` only distinguishes toggle vs
/// button; the launchpad also has read-only info tiles, a media transport, and
/// the rotating L slot, so it needs its own presentation role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TileRole {
    /// Stateful on/off control (Bluetooth, Wi-Fi, Theme, Focus, Saver).
    Toggle,
    /// Read-only live value (Battery).
    Info,
    /// A fire-once system action rendered as a compact button (Mic, Restart,
    /// Shut Down).
    Action,
    /// Now Playing: track name plus a play/pause transport.
    Media,
    /// Read-only weather (condition icon + temperature), resolved natively.
    Weather,
    /// The rotating Todo / Pomo / Clock slot; rendered entirely by the shell.
    Slot,
}

/// One tile in the empty-state launchpad. Carries everything the platform shell
/// needs to place and label it; live state (toggle value, battery %, track) is
/// resolved natively.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LaunchpadTile {
    pub action_id: String,
    pub title: String,
    pub size: TileSize,
    pub role: TileRole,
    /// The keyboard-mnemonic character, triggered with the platform command
    /// modifier. `None` for non-actionable tiles (L slot, Battery).
    pub mnemonic: Option<char>,
    /// Column span override, used by the Now Playing tile (spans 3). `None`
    /// falls back to the natural width of `size`.
    pub span: Option<u8>,
    /// Row span override, used by the Weather tile (stands 2 rows tall in a
    /// single column). `None` falls back to the natural height of `size`.
    pub row_span: Option<u8>,
    /// On/off captions for toggle tiles (e.g. Theme's Dark/Light), from the
    /// shared descriptor. `None` for non-toggle tiles.
    pub on_label: Option<String>,
    pub off_label: Option<String>,
}

fn tile(action_id: &str, size: TileSize, role: TileRole, mnemonic: Option<char>) -> LaunchpadTile {
    let descriptor = descriptor(action_id);
    LaunchpadTile {
        action_id: action_id.to_string(),
        title: descriptor
            .as_ref()
            .map(|d| d.title.clone())
            .unwrap_or_default(),
        size,
        role,
        mnemonic,
        span: None,
        row_span: None,
        on_label: descriptor.as_ref().and_then(|d| d.on_label.clone()),
        off_label: descriptor.and_then(|d| d.off_label),
    }
}

/// The fixed empty-state launchpad layout, in placement order. This exact order
/// and these sizes tile the 6-column grid with no gaps in every state:
///
/// L slot -> BT -> Wi-Fi -> Battery -> Theme -> Focus -> Saver -> Weather ->
/// Mic -> Restart -> Shut Down -> Now Playing (M, span 3).
///
/// Battery and Theme are single-column (S); the freed column on the right of the
/// middle block is filled by the Weather tile, which stands two rows tall.
///
/// The order and mnemonics are shared so every platform shell renders the same
/// control strip; only the native state reads differ.
pub fn launchpad_layout() -> Vec<LaunchpadTile> {
    use crate::action_id as id;
    use TileRole as role;
    use TileSize as size;

    let now_playing = LaunchpadTile {
        action_id: id::NOW_PLAYING.to_string(),
        title: "Now Playing".to_string(),
        size: size::M,
        role: role::Media,
        mnemonic: Some('P'),
        span: Some(3),
        row_span: None,
        on_label: None,
        off_label: None,
    };

    let weather = LaunchpadTile {
        action_id: id::WEATHER.to_string(),
        title: "Weather".to_string(),
        size: size::S,
        role: role::Weather,
        mnemonic: None,
        span: None,
        row_span: Some(2),
        on_label: None,
        off_label: None,
    };

    vec![
        LaunchpadTile {
            action_id: id::L_SLOT.to_string(),
            title: String::new(),
            size: size::L,
            role: role::Slot,
            mnemonic: None,
            span: None,
            row_span: None,
            on_label: None,
            off_label: None,
        },
        tile(id::BLUETOOTH, size::S, role::Toggle, Some('B')),
        tile(id::WIFI, size::S, role::Toggle, Some('W')),
        LaunchpadTile {
            action_id: id::BATTERY.to_string(),
            title: "Battery".to_string(),
            size: size::S,
            role: role::Info,
            mnemonic: None,
            span: None,
            row_span: None,
            on_label: None,
            off_label: None,
        },
        tile(id::THEME, size::S, role::Toggle, Some('T')),
        tile(id::FOCUS, size::S, role::Toggle, Some('F')),
        tile(id::SAVER, size::S, role::Toggle, Some('S')),
        weather,
        tile(id::MIC, size::S, role::Action, Some('M')),
        tile(id::RESTART, size::S, role::Action, Some('R')),
        tile(id::SHUTDOWN, size::S, role::Action, Some('D')),
        now_playing,
    ]
}

/// Descriptors that apply to a search result. Resolves the (platform-specific)
/// result id to a shared `action_id` via [`binding_for`], then looks up the
/// definition. Empty when the result has no actions.
pub fn descriptors_for(result_id: &str, kind: &str) -> Vec<ActionDescriptor> {
    binding_for(result_id, kind)
        .and_then(descriptor)
        .into_iter()
        .collect()
}

/// Maps a platform result to a shared `action_id`. The action DEFINITIONS above
/// are shared; only which platform result triggers them is per-OS, so this is
/// `cfg`-gated. A contributor adding an OS binds its result id here.
#[cfg(target_os = "macos")]
fn binding_for(result_id: &str, _kind: &str) -> Option<&'static str> {
    match result_id {
        "setting:com.apple.bluetoothsettings" => Some("bluetooth"),
        _ => None,
    }
}

#[cfg(target_os = "linux")]
fn binding_for(result_id: &str, _kind: &str) -> Option<&'static str> {
    // Ids come from core/engine/src/platform/linux/settings_catalog.rs.
    match result_id {
        "setting:bluetooth" => Some("bluetooth"),
        _ => None,
    }
}

#[cfg(target_os = "windows")]
fn binding_for(result_id: &str, _kind: &str) -> Option<&'static str> {
    // Ids come from core/engine/src/platform/windows/settings_catalog.rs
    // (candidate_id_suffix, prefixed with `setting:`).
    match result_id {
        "setting:windows.devices.bluetooth" => Some("bluetooth"),
        _ => None,
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn binding_for(_result_id: &str, _kind: &str) -> Option<&'static str> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bluetooth_descriptor_is_a_toggle_with_labels() {
        let d = descriptor("bluetooth").expect("bluetooth defined");
        assert_eq!(d.control, ControlKind::Toggle);
        assert_eq!(d.on_label.as_deref(), Some("On"));
        assert_eq!(d.off_label.as_deref(), Some("Off"));
        assert_eq!(d.info.len(), 1);
    }

    #[test]
    fn unknown_action_is_none() {
        assert!(descriptor("nope").is_none());
    }

    #[test]
    fn launchpad_layout_matches_the_fixed_spec_order() {
        let layout = launchpad_layout();
        let ids: Vec<&str> = layout.iter().map(|t| t.action_id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                action_id::L_SLOT,
                action_id::BLUETOOTH,
                action_id::WIFI,
                action_id::BATTERY,
                action_id::THEME,
                action_id::FOCUS,
                action_id::SAVER,
                action_id::WEATHER,
                action_id::MIC,
                action_id::RESTART,
                action_id::SHUTDOWN,
                action_id::NOW_PLAYING,
            ]
        );
    }

    #[test]
    fn launchpad_control_tiles_resolve_to_a_descriptor() {
        // Every tile backed by a system control must have a catalog descriptor so
        // its title/labels stay in sync; the presentational tiles (L slot,
        // Battery, Weather, Now Playing) intentionally have none.
        let presentational = [
            action_id::L_SLOT,
            action_id::BATTERY,
            action_id::WEATHER,
            action_id::NOW_PLAYING,
        ];
        for tile in launchpad_layout() {
            if presentational.contains(&tile.action_id.as_str()) {
                continue;
            }
            let d = descriptor(&tile.action_id)
                .unwrap_or_else(|| panic!("no descriptor for {}", tile.action_id));
            assert_eq!(d.title, tile.title);
        }
    }

    #[test]
    fn now_playing_spans_three_columns() {
        let layout = launchpad_layout();
        let now_playing = layout
            .iter()
            .find(|t| t.action_id == action_id::NOW_PLAYING)
            .expect("now playing tile present");
        assert_eq!(now_playing.span, Some(3));
        assert_eq!(now_playing.role, TileRole::Media);
    }

    #[test]
    fn weather_tile_stands_two_rows_tall() {
        let layout = launchpad_layout();
        let weather = layout
            .iter()
            .find(|t| t.action_id == action_id::WEATHER)
            .expect("weather tile present");
        assert_eq!(weather.size, TileSize::S);
        assert_eq!(weather.row_span, Some(2));
        assert_eq!(weather.role, TileRole::Weather);
    }

    #[test]
    fn actionable_tiles_carry_unique_mnemonics() {
        let mut seen = std::collections::HashSet::new();
        for tile in launchpad_layout() {
            if let Some(ch) = tile.mnemonic {
                // Shells match mnemonics case-insensitively, so `b` and `B`
                // would collide at runtime and leave one tile unreachable.
                assert!(
                    seen.insert(ch.to_ascii_lowercase()),
                    "duplicate mnemonic {ch} on {}",
                    tile.action_id
                );
            }
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_bluetooth_setting_resolves_to_the_toggle() {
        let found = descriptors_for("setting:bluetooth", "app");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].action_id, "bluetooth");
        // A non-actionable result yields nothing.
        assert!(descriptors_for("setting:sound", "app").is_empty());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_bluetooth_setting_resolves_to_the_toggle() {
        let found = descriptors_for("setting:windows.devices.bluetooth", "app");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].action_id, "bluetooth");
        // A non-actionable result yields nothing.
        assert!(descriptors_for("setting:windows.devices.wifi", "app").is_empty());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_bluetooth_setting_resolves_to_the_toggle() {
        let found = descriptors_for("setting:com.apple.bluetoothsettings", "app");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].action_id, "bluetooth");
        // A non-actionable result yields nothing.
        assert!(descriptors_for("setting:com.apple.wifi-settings-extension", "app").is_empty());
    }
}
