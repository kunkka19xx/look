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
    pub const KEEP_AWAKE: &str = "keepawake";
    pub const SCREENSAVER: &str = "screensaver";
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
        id::KEEP_AWAKE => Some(toggle(id::KEEP_AWAKE, "Keep Awake", "On", "Off")),
        id::SCREENSAVER => Some(button(id::SCREENSAVER, "Screensaver")),
        id::MIC => Some(toggle(id::MIC, "Mic", "On", "Muted")),
        id::RESTART => Some(button(id::RESTART, "Restart")),
        id::SHUTDOWN => Some(button(id::SHUTDOWN, "Shut Down")),
        _ => None,
    }
}

/// How a launchpad tile is DRESSED: which font sizes, paddings and glyph
/// treatment it gets. Not how wide it is - that is `col_span`/`row_span`, which
/// the core resolves. The two used to be the same thing, which is why an S tile
/// standing two rows tall (Weather) needed an override to say so.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TileSize {
    L,
    M,
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
    /// A tile the user declared in `~/.look/super-actions.toml`. No descriptor and
    /// no native adapter, so `title` has to arrive already filled in.
    Custom,
}

/// Who owns a plain Cmd/Ctrl + letter chord before the launchpad ever sees it.
///
/// The start of one answer to "is this chord taken, and by whom". Today that
/// question has three answers in three places: menu `keyboardShortcut`s in the
/// macOS shell, ~20 positional handlers in its key monitor, and this catalog.
/// A menu shortcut is dispatched by the OS before Look's own monitor runs, so a
/// tile holding one would silently never fire - the failure a name here turns
/// into a warning.
///
/// Only chords that win EVERYWHERE belong here. The result-oriented ones
/// (Cmd+F reveal, Cmd+P pick) are deliberately absent: they sit below the
/// launchpad mnemonic in the monitor, so on the empty screen they are free.
pub fn chord_owner(key: char) -> Option<&'static str> {
    match key.to_ascii_uppercase() {
        'Q' => Some("quitting Look"),
        _ => None,
    }
}

/// The smallest rectangle a role can be drawn in and still say what it means.
/// Keyed off the role, not the id: the presentation is what needs the room.
/// `launchpad::resolve` drops anything drawn under this.
pub fn min_span(role: TileRole) -> (u8, u8) {
    match role {
        TileRole::Slot => (2, 2),
        TileRole::Weather => (1, 2),
        TileRole::Media => (2, 1),
        // The user drew it, so the size they drew is the size they meant.
        TileRole::Toggle | TileRole::Info | TileRole::Action | TileRole::Custom => (1, 1),
    }
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
    /// Where the tile sits, already solved here: the zero-based column and row
    /// of its top-left cell, and how many cells it covers.
    ///
    /// Absolute rather than a hint. The spans these replaced were relative -
    /// each shell turned them into placement itself, which meant the
    /// arrangement was written once here and again in every shell. A shell now
    /// offsets by `col`/`row` and sizes by the spans, and nothing re-derives an
    /// arrangement from the order of this list.
    pub col: u8,
    pub row: u8,
    pub col_span: u8,
    pub row_span: u8,
    /// On/off captions for toggle tiles (e.g. Theme's Dark/Light), from the
    /// shared descriptor. `None` for non-toggle tiles.
    pub on_label: Option<String>,
    pub off_label: Option<String>,
    /// Whether pressing it does anything. Battery and Weather are readouts, and
    /// so is a user tile that declared no `press` - none of them should offer a
    /// button's affordances for something that will not happen.
    pub pressable: bool,
    /// Whether it has anything to display, as opposed to only acting. Without
    /// it a shell cannot tell "nothing to show" from "has not run yet", and
    /// would leave a button on a placeholder forever.
    pub has_value: bool,
    /// Asked before the press runs, in the words the user wrote.
    pub confirm: Option<String>,
    /// The symbol a user tile asked for, when it did. A built-in's symbol stays
    /// with the shell that draws it: these are platform names, and the two
    /// shells do not spell them the same way.
    pub icon: Option<String>,
}

/// One entry of the layout table below. `title` is `None` when the catalog
/// descriptor supplies it, and `Some` for the presentational tiles that have no
/// descriptor to take it from (L slot, Battery, Weather, Now Playing).
type Placed = (
    &'static str,
    Option<&'static str>,
    TileSize,
    TileRole,
    Option<char>,
    u8,
    u8,
    u8,
    u8,
);

fn tile(placed: Placed) -> LaunchpadTile {
    let (action_id, title, size, role, mnemonic, col, row, col_span, row_span) = placed;
    let descriptor = descriptor(action_id);
    LaunchpadTile {
        action_id: action_id.to_string(),
        title: title.map(str::to_string).unwrap_or_else(|| {
            descriptor
                .as_ref()
                .map(|d| d.title.clone())
                .unwrap_or_default()
        }),
        size,
        role,
        mnemonic,
        col,
        row,
        col_span,
        row_span,
        on_label: descriptor.as_ref().and_then(|d| d.on_label.clone()),
        off_label: descriptor.and_then(|d| d.off_label),
        pressable: matches!(role, TileRole::Toggle | TileRole::Action | TileRole::Media),
        has_value: !matches!(role, TileRole::Action),
        // What a tile asks before it acts, wherever it came from. The shells
        // gate on this field alone rather than on a list of ids.
        confirm: match action_id {
            crate::action_id::RESTART => Some("Restart?".to_string()),
            crate::action_id::SHUTDOWN => Some("Shut down?".to_string()),
            _ => None,
        },
        // A built-in's symbol belongs to the shell drawing it, which already
        // knows this id.
        icon: None,
    }
}

/// The default empty-state launchpad layout, every tile already placed.
///
/// The table below IS the grid, read as six columns by three rows:
///
/// ```text
/// lslot  lslot    bluetooth  wifi        battery      weather
/// lslot  lslot    theme      keepawake   screensaver  weather
/// mic    restart  shutdown   nowplaying  nowplaying   nowplaying
/// ```
///
/// Order carries no meaning any more. It used to: the list was in placement
/// order and each shell re-encoded that same arrangement by hand, so the layout
/// lived in three places and had to agree. Now a shell offsets each tile by its
/// own `col`/`row` and never reconstructs anything.
///
/// `size` stays, but as presentation only - which font and padding a tile gets,
/// not how wide it is. Width is `col_span`.
pub fn launchpad_layout() -> Vec<LaunchpadTile> {
    use crate::action_id as id;
    use TileRole as role;
    use TileSize as size;

    // id, title override, size, role, mnemonic, col, row, col_span, row_span
    let placed: [Placed; 12] = [
        (id::L_SLOT, Some(""), size::L, role::Slot, None, 0, 0, 2, 2),
        (
            id::BLUETOOTH,
            None,
            size::S,
            role::Toggle,
            Some('B'),
            2,
            0,
            1,
            1,
        ),
        (id::WIFI, None, size::S, role::Toggle, Some('W'), 3, 0, 1, 1),
        (
            id::BATTERY,
            Some("Battery"),
            size::S,
            role::Info,
            None,
            4,
            0,
            1,
            1,
        ),
        // Two rows tall in the column the single-width Battery and Screensaver
        // tiles leave free on the right of the middle block.
        (
            id::WEATHER,
            Some("Weather"),
            size::S,
            role::Weather,
            None,
            5,
            0,
            1,
            2,
        ),
        (
            id::THEME,
            None,
            size::S,
            role::Toggle,
            Some('T'),
            2,
            1,
            1,
            1,
        ),
        (
            id::KEEP_AWAKE,
            None,
            size::S,
            role::Toggle,
            Some('K'),
            3,
            1,
            1,
            1,
        ),
        (
            id::SCREENSAVER,
            None,
            size::S,
            role::Action,
            Some('S'),
            4,
            1,
            1,
            1,
        ),
        (id::MIC, None, size::S, role::Action, Some('M'), 0, 2, 1, 1),
        (
            id::RESTART,
            None,
            size::S,
            role::Action,
            Some('R'),
            1,
            2,
            1,
            1,
        ),
        (
            id::SHUTDOWN,
            None,
            size::S,
            role::Action,
            Some('D'),
            2,
            2,
            1,
            1,
        ),
        (
            id::NOW_PLAYING,
            Some("Now Playing"),
            size::M,
            role::Media,
            Some('P'),
            3,
            2,
            3,
            1,
        ),
    ];

    placed.into_iter().map(tile).collect()
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
    fn the_default_layout_tiles_the_grid_with_no_gap_and_no_overlap() {
        // This replaces an assertion on the exact ORDER of the twelve tiles.
        // Order was the contract when the shells re-derived placement from it;
        // now that each tile carries its own cell, order says nothing and a
        // test on it would pass while the grid was wrong.
        //
        // What matters instead is the property the old fixed order existed to
        // guarantee: the tiles cover the 6x3 grid exactly once. This is the
        // regression felt on every launch, and the one a user-declared layout
        // is allowed to break (a hole is their choice) but the DEFAULT is not.
        const COLUMNS: u8 = 6;
        const ROWS: u8 = 3;

        let mut owner = std::collections::HashMap::new();
        for tile in launchpad_layout() {
            assert!(
                tile.col_span >= 1 && tile.row_span >= 1,
                "{} covers no cell",
                tile.action_id
            );
            for row in tile.row..tile.row + tile.row_span {
                for col in tile.col..tile.col + tile.col_span {
                    assert!(
                        col < COLUMNS && row < ROWS,
                        "{} runs outside the grid at ({col},{row})",
                        tile.action_id
                    );
                    if let Some(other) = owner.insert((col, row), tile.action_id.clone()) {
                        panic!("{} overlaps {other} at ({col},{row})", tile.action_id);
                    }
                }
            }
        }

        assert_eq!(
            owner.len(),
            usize::from(COLUMNS) * usize::from(ROWS),
            "the default layout leaves a hole"
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

    fn placed(action_id: &str) -> LaunchpadTile {
        launchpad_layout()
            .into_iter()
            .find(|t| t.action_id == action_id)
            .unwrap_or_else(|| panic!("{action_id} is in the layout"))
    }

    #[test]
    fn now_playing_fills_the_rest_of_the_bottom_row() {
        let now_playing = placed(action_id::NOW_PLAYING);
        assert_eq!(
            (
                now_playing.col,
                now_playing.row,
                now_playing.col_span,
                now_playing.row_span
            ),
            (3, 2, 3, 1)
        );
        assert_eq!(now_playing.role, TileRole::Media);
    }

    #[test]
    fn weather_stands_two_rows_tall_in_the_last_column() {
        let weather = placed(action_id::WEATHER);
        assert_eq!(
            (weather.col, weather.row, weather.col_span, weather.row_span),
            (5, 0, 1, 2)
        );
        // `size` is presentation now, not width: an S tile that covers one
        // column and two rows is exactly the case that separates the two.
        assert_eq!(weather.size, TileSize::S);
        assert_eq!(weather.role, TileRole::Weather);
    }

    #[test]
    fn every_reserved_chord_names_who_holds_it() {
        // The point of the registry: a refusal can say why. A reserved chord
        // with no owner would produce "it has no key" and no reason.
        assert_eq!(chord_owner('Q'), Some("quitting Look"));
        assert_eq!(chord_owner('q'), chord_owner('Q'), "case-insensitive");
        assert_eq!(chord_owner('F'), None, "free on the empty screen");
    }

    #[test]
    fn no_built_in_tile_wants_a_reserved_chord() {
        // A built-in claiming one would never fire either, and nothing would
        // report it - the warning path only covers user tiles.
        for tile in launchpad_layout() {
            if let Some(key) = tile.mnemonic {
                assert_eq!(
                    chord_owner(key),
                    None,
                    "{} wants Cmd+{key}, which is reserved",
                    tile.action_id
                );
            }
        }
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
