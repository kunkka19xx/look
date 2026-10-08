//! The one list of keys: what the help screen shows by topic, what
//! Settings > Shortcuts shows flat, and where every footer hint takes its
//! key spelling from, so the three cannot drift. The macOS
//! `ShortcutCatalog`, with the webview's wording for Linux and Windows.

use std::sync::LazyLock;

use gpui::prelude::*;
use gpui::{Div, FontWeight, SharedString, div, px};

use crate::modes::{self, COMMAND_ENTRIES};
use crate::theme::Theme;

/// One documented shortcut. `id` is the stable handle: the keys may
/// change, the id must not, since the hints bind to it.
pub struct Entry {
    pub id: &'static str,
    pub keys: String,
    pub action: String,
}

impl Entry {
    fn new(id: &'static str, keys: impl Into<String>, action: impl Into<String>) -> Self {
        Self {
            id,
            keys: keys.into(),
            action: action.into(),
        }
    }
}

/// The help screen's filter capsules; Settings shows every topic in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Topic {
    Main,
    Prefixes,
    Command,
}

impl Topic {
    pub const ALL: [Topic; 3] = [Topic::Main, Topic::Prefixes, Topic::Command];

    pub fn label(self) -> &'static str {
        match self {
            Topic::Main => "Main",
            Topic::Prefixes => "Prefixes",
            Topic::Command => "Command",
        }
    }
}

/// One titled block. The title is the identity: two groups never share one.
pub struct Group {
    pub title: &'static str,
    pub topic: Topic,
    pub entries: Vec<Entry>,
}

pub const TOGGLE_LAUNCHER: &str = "global.toggleLauncher";
pub const OPEN: &str = "main.open";
pub const OPEN_PICKED: &str = "main.openPicked";
pub const COPY: &str = "main.copy";
pub const PASTE: &str = "main.paste";
pub const TRASH: &str = "main.trash";
pub const PICK: &str = "main.pick";
pub const CLEAR_PICKS: &str = "main.clearPicks";
pub const MOVE_TAB: &str = "main.moveTab";
pub const MOVE_ARROWS: &str = "main.moveArrows";
pub const ACTIONS: &str = "main.actions";
pub const EDIT: &str = "main.edit";
pub const TERMINAL: &str = "main.terminal";
pub const REVEAL: &str = "main.reveal";
pub const WEB_SEARCH: &str = "main.webSearch";
pub const COMMAND_MODE: &str = "main.commandMode";
pub const COMMAND_JUMP: &str = "main.commandJump";
pub const SETTINGS: &str = "main.settings";
pub const RELOAD_CONFIG: &str = "main.reloadConfig";
pub const HIDE_APP: &str = "main.hideApp";
pub const HELP: &str = "main.help";
pub const BACK: &str = "main.back";
pub const CLIP_COPY_BACK: &str = "clipboard.copyBack";
pub const CLIP_PASTE: &str = "clipboard.paste";
pub const CLIP_REMOVE: &str = "clipboard.remove";
pub const PROCESS_CPU: &str = "process.cpu";
pub const PROCESS_KILL: &str = "process.kill";
pub const PROCESS_COPY_PID: &str = "process.copyPid";
pub const COMMAND_SWITCH_TAB: &str = "command.switchTab";
pub const COMMAND_BACK: &str = "command.back";
pub const KILL_CONFIRM: &str = "kill.confirm";
pub const POMO_START_PAUSE: &str = "pomo.startPause";
pub const POMO_RESET: &str = "pomo.reset";
pub const TODO_SAVE: &str = "todo.save";
pub const TODO_UNDO: &str = "todo.undo";
pub const SPEED_RERUN: &str = "speed.rerun";
pub const SPEED_REVEAL: &str = "speed.revealAddress";
pub const CALC_EVALUATE: &str = "calc.evaluate";

const BIN_NAME: &str = if cfg!(windows) {
    "Recycle Bin"
} else {
    "Trash"
};

/// The default spelling of the launcher key; the live one comes from the
/// hotkey state when Settings shows the row.
pub const TOGGLE_LAUNCHER_KEYS: &str = "Alt+Space";

static CATALOG: LazyLock<Vec<Group>> = LazyLock::new(build);

pub fn groups() -> &'static [Group] {
    &CATALOG
}

pub fn groups_for(topic: Topic) -> impl Iterator<Item = &'static Group> {
    groups().iter().filter(move |g| g.topic == topic)
}

pub fn entry(id: &str) -> Option<&'static Entry> {
    groups()
        .iter()
        .flat_map(|g| &g.entries)
        .find(|e| e.id == id)
}

fn build() -> Vec<Group> {
    let command_names: Vec<String> = COMMAND_ENTRIES
        .iter()
        .map(|e| format!("/{}", e.id))
        .collect();
    vec![
        Group {
            title: "Global",
            topic: Topic::Main,
            entries: vec![Entry::new(
                TOGGLE_LAUNCHER,
                TOGGLE_LAUNCHER_KEYS,
                "Show or hide Look from any app",
            )],
        },
        Group {
            title: "Main",
            topic: Topic::Main,
            entries: vec![
                Entry::new(
                    OPEN,
                    "Enter",
                    "Open selected app/file/folder or copy selected clipboard item",
                ),
                Entry::new(
                    OPEN_PICKED,
                    "Shift+Enter",
                    "Open all picked files/folders at once",
                ),
                Entry::new(
                    COPY,
                    "Ctrl+C",
                    "Copy the selected file/folder: paste it as a file in a file manager, as its path anywhere else",
                ),
                Entry::new(
                    PASTE,
                    "Ctrl+I",
                    "In clipboard history: paste the selected clip into the app you came from (text or image)",
                ),
                Entry::new(
                    TRASH,
                    "Ctrl+D",
                    format!(
                        "Move selected file/folder to {BIN_NAME}; on the {BIN_NAME} pin, confirm Y/N to empty"
                    ),
                ),
                Entry::new(PICK, "Ctrl+P", "Pick / unpick item (multi-select copy)"),
                Entry::new(CLEAR_PICKS, "Ctrl+Shift+P", "Clear all picks"),
                Entry::new(MOVE_TAB, "Tab / Shift+Tab", "Select next / previous result"),
                Entry::new(MOVE_ARROWS, "Up / Down", "Move selection"),
                Entry::new(
                    ACTIONS,
                    "Ctrl+K / Ctrl+J",
                    "Show the selected result's actions; inside it, Ctrl+J / Ctrl+K move, Esc closes",
                ),
                Entry::new(
                    EDIT,
                    "Ctrl+E",
                    "Edit selected file/folder in your preferred editor",
                ),
                Entry::new(
                    TERMINAL,
                    "Ctrl+T",
                    "Open a terminal at the selected file/folder",
                ),
                Entry::new(REVEAL, "Ctrl+F", "Reveal selected item in file manager"),
                Entry::new(WEB_SEARCH, "Ctrl+Enter", "Search query on Google"),
                Entry::new(COMMAND_MODE, "Ctrl+/", "Enter command mode"),
                Entry::new(
                    COMMAND_JUMP,
                    ":cmd",
                    "Jump to a command from home (e.g. :calc 2+2, :kill chrome, :sys, :pomo)",
                ),
                Entry::new(SETTINGS, "Ctrl+Shift+,", "Open/close settings panel"),
                Entry::new(RELOAD_CONFIG, "Ctrl+Shift+;", "Reload .look/config"),
                Entry::new(HIDE_APP, "Ctrl+Shift+H", "Hide the selected app from Look"),
                Entry::new(HELP, "Ctrl+H", "Toggle the help screen"),
                Entry::new(BACK, "Esc", "Close help / back / hide launcher"),
            ],
        },
        Group {
            title: "Running apps",
            topic: Topic::Main,
            entries: vec![
                Entry::new(
                    "apps.focus",
                    "Alt+1..9",
                    "Focus a running app by its badge number (strip at the right of the search bar)",
                ),
                Entry::new(
                    "apps.click",
                    "Click icon",
                    "Focus the running app under the pointer",
                ),
                Entry::new(
                    "apps.strip",
                    "Settings > Appearance",
                    "Toggle the running apps strip on or off",
                ),
            ],
        },
        Group {
            title: "Super actions",
            topic: Topic::Main,
            entries: vec![
                Entry::new(
                    "super.bluetoothWifi",
                    "Alt+B / Alt+W",
                    "Toggle Bluetooth / Wi-Fi",
                ),
                Entry::new(
                    "super.themeAwake",
                    "Alt+T / Alt+K",
                    "Switch theme / toggle Keep Awake",
                ),
                Entry::new(
                    "super.screensaverMic",
                    "Alt+S / Alt+M",
                    "Start screensaver / mute mic",
                ),
                Entry::new("super.playPause", "Alt+P", "Play/pause the current track"),
                Entry::new(
                    "super.power",
                    "Alt+R / Alt+D",
                    "Restart / Shut Down (press twice to confirm)",
                ),
                Entry::new(
                    "super.toggleStrip",
                    "Settings > Appearance",
                    "Show or hide the super actions strip",
                ),
            ],
        },
        Group {
            title: "Clipboard history",
            topic: Topic::Prefixes,
            entries: vec![
                Entry::new(
                    CLIP_COPY_BACK,
                    "Enter",
                    "Copy selected clip back to the clipboard",
                ),
                Entry::new(
                    CLIP_PASTE,
                    "Ctrl+I",
                    "Paste the selected clip into the app you came from (text or image)",
                ),
                Entry::new(
                    CLIP_REMOVE,
                    "Ctrl+D",
                    "Remove the selected clip from history",
                ),
            ],
        },
        Group {
            title: "Processes (ps\")",
            topic: Topic::Prefixes,
            entries: vec![
                Entry::new(PROCESS_CPU, "Enter", "Show the selected process's CPU use"),
                Entry::new(PROCESS_KILL, "Ctrl+D", "Kill the selected process"),
                Entry::new(
                    PROCESS_COPY_PID,
                    "Ctrl+C",
                    "Copy the selected process's PID",
                ),
            ],
        },
        Group {
            title: "Query prefixes",
            topic: Topic::Prefixes,
            entries: std::iter::once(Entry::new(
                "prefix.browse",
                "\"",
                "Browse all prefixes: type a letter to filter, Enter to pick",
            ))
            .chain(
                modes::prefix_entries().map(|(prefix, arg, description)| Entry {
                    id: prefix,
                    keys: format!("{prefix}{arg}"),
                    action: description.to_string(),
                }),
            )
            .collect(),
        },
        Group {
            title: "Command mode",
            topic: Topic::Command,
            entries: vec![
                Entry::new(
                    "command.browse",
                    ":",
                    "Browse all commands: type to filter, Enter to run",
                ),
                Entry::new(
                    "command.jump",
                    ":cmd args",
                    "Jump straight into a command, e.g. :calc 2+2",
                ),
                Entry::new(COMMAND_SWITCH_TAB, "Tab / Shift+Tab", "Switch command"),
                Entry::new(
                    "command.switchByIndex",
                    format!("Ctrl+1..{}", COMMAND_ENTRIES.len()),
                    format!("Switch directly to {}", command_names.join(", ")),
                ),
                Entry::new(COMMAND_BACK, "Esc", "Back to the app list"),
            ],
        },
        Group {
            title: "Kill (/kill)",
            topic: Topic::Command,
            entries: vec![
                Entry::new("kill.byPort", "3000", "Match by port or PID, not just name"),
                Entry::new("kill.select", "Up / Down", "Select app in kill results"),
                Entry::new(KILL_CONFIRM, "Y / N", "Confirm / cancel the kill"),
            ],
        },
        Group {
            title: "Pomodoro (/pomo)",
            topic: Topic::Command,
            entries: vec![
                Entry::new(
                    POMO_START_PAUSE,
                    "Space",
                    "Start / pause the active session",
                ),
                Entry::new(POMO_RESET, "R", "Reset the timer back to idle"),
                Entry::new("pomo.music", "P", "Toggle music play / pause"),
                Entry::new(
                    "pomo.standby",
                    "Mouse / key idle",
                    "After 5s, the panel fades to the ring alone; any input restores",
                ),
            ],
        },
        Group {
            title: "Todo, Speed, Calc panels",
            topic: Topic::Command,
            entries: vec![
                Entry::new(
                    "todo.togglePage",
                    "Ctrl+N",
                    "Switch the Tasks / Stats page inside /todo",
                ),
                Entry::new(TODO_SAVE, "Ctrl+S", "Save changes inside /todo"),
                Entry::new(
                    TODO_UNDO,
                    "Ctrl+Z",
                    "Undo the last task change inside /todo",
                ),
                Entry::new(
                    "todo.redo",
                    "Ctrl+Shift+Z",
                    "Redo an undone task change inside /todo",
                ),
                Entry::new(SPEED_RERUN, "R", "Run the test again inside /speed"),
                Entry::new(
                    SPEED_REVEAL,
                    "E",
                    "Show or hide the public address inside /speed",
                ),
                Entry::new(CALC_EVALUATE, "Enter", "Copy the result inside /calc"),
            ],
        },
    ]
}

// --- Hints -----------------------------------------------------------------------

/// One `Key: Label` of a footer hint. An `Id` takes its key from the
/// catalog; a `Key` is a chord the catalog has no row for; `Text` is a
/// plain phrase.
pub enum Piece {
    Id(&'static str, &'static str),
    Key(&'static str, &'static str),
    Text(&'static str),
}

pub const HINT_SEP: &str = " \u{2022} ";

/// The footer line for `pieces`.
pub fn hint(pieces: &[Piece]) -> String {
    pieces
        .iter()
        .map(|piece| match piece {
            Piece::Id(id, label) => {
                let keys = entry(id).map_or(*id, |e| e.keys.as_str());
                format!("{keys}: {label}")
            }
            Piece::Key(keys, label) => format!("{keys}: {label}"),
            Piece::Text(text) => (*text).to_string(),
        })
        .collect::<Vec<_>>()
        .join(HINT_SEP)
}

// --- Render ----------------------------------------------------------------------

const SECTION_PADDING_TOP: f32 = 10.0;
const SECTION_PADDING_BOTTOM: f32 = 4.0;
const ROW_GAP: f32 = 10.0;
const ROW_PADDING_Y: f32 = 3.0;
const KEY_PADDING_X: f32 = 8.0;
const KEY_PADDING_Y: f32 = 3.0;

/// A group's title, the webview's `.settings-shortcut-section`.
pub fn section_title(title: impl Into<SharedString>, th: &Theme) -> Div {
    div()
        .pt(px(SECTION_PADDING_TOP))
        .pb(px(SECTION_PADDING_BOTTOM))
        .text_size(px(th.font_size + 1.0))
        .font_weight(FontWeight::BOLD)
        .text_color(th.text)
        .child(title.into())
}

/// The key capsule, the webview's `kbd`.
pub fn key_cap(keys: impl Into<SharedString>, th: &Theme) -> Div {
    div()
        .px(px(KEY_PADDING_X))
        .py(px(KEY_PADDING_Y))
        .rounded_full()
        .bg(th.control_fill)
        .font_family(th.mono_family.clone())
        .text_size(px(th.font_size - 2.0))
        .text_color(th.text)
        .whitespace_nowrap()
        .flex_shrink_0()
        .child(keys.into())
}

/// One row: the capsule, then what it does.
pub fn row(keys: impl Into<SharedString>, action: impl Into<SharedString>, th: &Theme) -> Div {
    div()
        .py(px(ROW_PADDING_Y))
        .flex()
        .items_baseline()
        .gap(px(ROW_GAP))
        .child(key_cap(keys, th))
        .child(
            div()
                .min_w_0()
                .text_size(px(th.font_size - 1.0))
                .text_color(th.text_secondary)
                .child(action.into()),
        )
}

/// A group as a titled block of rows.
pub fn group_view(group: &Group, th: &Theme) -> Div {
    div()
        .flex()
        .flex_col()
        .child(section_title(group.title, th))
        .children(
            group
                .entries
                .iter()
                .map(|e| row(e.keys.clone(), e.action.clone(), th)),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_the_named_ones_exist() {
        let mut seen = std::collections::HashSet::new();
        for e in groups().iter().flat_map(|g| &g.entries) {
            assert!(seen.insert(e.id), "duplicate id {}", e.id);
        }
        for id in [
            OPEN,
            ACTIONS,
            HELP,
            BACK,
            REVEAL,
            WEB_SEARCH,
            MOVE_ARROWS,
            CLIP_COPY_BACK,
            CLIP_PASTE,
            CLIP_REMOVE,
            PROCESS_CPU,
            PROCESS_KILL,
            PROCESS_COPY_PID,
            COMMAND_BACK,
            POMO_START_PAUSE,
            POMO_RESET,
            TODO_SAVE,
            SPEED_RERUN,
            SPEED_REVEAL,
            CALC_EVALUATE,
            KILL_CONFIRM,
            TOGGLE_LAUNCHER,
        ] {
            assert!(entry(id).is_some(), "missing {id}");
        }
    }

    #[test]
    fn hints_take_keys_from_the_catalog() {
        let line = hint(&[
            Piece::Id(OPEN, "Open"),
            Piece::Key("Y", "Confirm"),
            Piece::Text("No match"),
        ]);
        assert_eq!(line, "Enter: Open \u{2022} Y: Confirm \u{2022} No match");
    }

    #[test]
    fn every_topic_has_a_group_and_prefixes_follow_the_menu() {
        for topic in Topic::ALL {
            assert!(groups_for(topic).next().is_some());
        }
        let prefixes = groups()
            .iter()
            .find(|g| g.title == "Query prefixes")
            .unwrap();
        assert!(prefixes.entries.iter().any(|e| e.keys == "a\"word"));
    }
}
