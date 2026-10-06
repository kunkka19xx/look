//! What the query's leading characters put the launcher into, and the two
//! discovery menus. One enum rather than the webview's scattered booleans;
//! the prefixes are spelled as `core/engine/src/modes.rs` spells them.

use crate::glyphs;
use crate::rows::Row;

const DISCOVERY: char = '"';
const COMMAND: char = ':';
pub const PREFIX_TRANSLATE: &str = "t\"";
pub const PREFIX_CLIPBOARD: &str = "c\"";
pub const PREFIX_CLIPBOARD_IMAGE: &str = "ci\"";
pub const PREFIX_RECENT: &str = "rc\"";
pub const PREFIX_PROCESS: &str = "ps\"";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Plain search: the engine, pinned folders, calc, URLs.
    Search,
    /// `"`: every query prefix.
    PrefixMenu,
    /// `:`: every command screen.
    CommandMenu,
    /// `t"`: a translation on Enter.
    Translate,
    /// `c"`: the text clips.
    Clipboard,
    /// `ci"`: the copied images.
    ClipboardImage,
    /// `rc"`: recent files and folders, scored by the engine.
    Recent,
    /// `ps"`: running processes.
    Process,
}

impl Mode {
    /// The mode a query is in and the text after its prefix. The menus are
    /// read first since their characters overlap the prefixes.
    pub fn of(query: &str) -> (Self, &str) {
        let trimmed = query.trim_start();
        if let Some(rest) = trimmed.strip_prefix(DISCOVERY) {
            return (Self::PrefixMenu, rest);
        }
        if let Some(rest) = trimmed.strip_prefix(COMMAND) {
            return (Self::CommandMenu, rest);
        }
        for (prefix, mode) in [
            (PREFIX_TRANSLATE, Self::Translate),
            (PREFIX_CLIPBOARD_IMAGE, Self::ClipboardImage),
            (PREFIX_CLIPBOARD, Self::Clipboard),
            (PREFIX_RECENT, Self::Recent),
            (PREFIX_PROCESS, Self::Process),
        ] {
            if let Some(rest) = query.strip_prefix(prefix) {
                return (mode, rest);
            }
        }
        (Self::Search, query)
    }

    /// Whether Escape clears the query rather than hiding the launcher.
    pub fn is_prefixed(self) -> bool {
        self != Self::Search
    }

    /// The menus: side actions have nothing to act on.
    pub fn is_menu(self) -> bool {
        matches!(self, Self::PrefixMenu | Self::CommandMenu)
    }

    pub fn is_clipboard(self) -> bool {
        matches!(self, Self::Clipboard | Self::ClipboardImage)
    }

    /// The footer line, the webview's per-mode hints.
    pub fn hint(self) -> &'static str {
        match self {
            Self::Search | Self::Recent => {
                "Enter: Open \u{2022} Ctrl+K: Actions \u{2022} Ctrl+F: Reveal"
            }
            Self::PrefixMenu => "Enter: Pick prefix \u{2022} Up/Down: Move \u{2022} Esc: Clear",
            Self::CommandMenu => "Enter: Run command \u{2022} Up/Down: Move \u{2022} Esc: Clear",
            Self::Translate => "Enter: Translate \u{2022} Copy per result \u{2022} Esc: Clear",
            Self::Clipboard => {
                "Enter: Copy clip \u{2022} Ctrl+I: Paste \u{2022} Ctrl+D: Remove clip"
            }
            Self::ClipboardImage => {
                "Enter: Copy image \u{2022} Ctrl+I: Paste \u{2022} Ctrl+D: Remove image"
            }
            Self::Process => "Enter: CPU \u{2022} Ctrl+D: Kill \u{2022} Ctrl+C: Copy PID",
        }
    }
}

/// One `"` menu row: the prefix, what goes after it, and what it does.
struct PrefixEntry {
    prefix: &'static str,
    arg_hint: &'static str,
    description: &'static str,
}

const PREFIX_ENTRIES: &[PrefixEntry] = &[
    PrefixEntry {
        prefix: "a\"",
        arg_hint: "word",
        description: "Apps only",
    },
    PrefixEntry {
        prefix: "f\"",
        arg_hint: "word",
        description: "Files only",
    },
    PrefixEntry {
        prefix: "d\"",
        arg_hint: "word",
        description: "Folders only",
    },
    PrefixEntry {
        prefix: PREFIX_RECENT,
        arg_hint: "word",
        description: "Recent files/folders, newest first (optional filter)",
    },
    PrefixEntry {
        prefix: "r\"",
        arg_hint: "pattern",
        description: "Regex search",
    },
    PrefixEntry {
        prefix: PREFIX_PROCESS,
        arg_hint: "word",
        description: "Find & kill running processes",
    },
    PrefixEntry {
        prefix: PREFIX_CLIPBOARD,
        arg_hint: "word",
        description: "Clipboard history search (latest 10 text clips)",
    },
    PrefixEntry {
        prefix: PREFIX_CLIPBOARD_IMAGE,
        arg_hint: "word",
        description: "Copied images, newest first",
    },
    PrefixEntry {
        prefix: PREFIX_TRANSLATE,
        arg_hint: "word",
        description: "Web translate (VI/EN/JA)",
    },
];

/// One `:` menu row. Catalog order is the Ctrl+N mapping on the command
/// screens, as the macOS `commandDefinitions`.
pub struct CommandEntry {
    pub id: &'static str,
    pub detail: &'static str,
    pub glyph: &'static str,
}

pub const COMMAND_ENTRIES: &[CommandEntry] = &[
    CommandEntry {
        id: "calc",
        detail: "Evaluate math expression",
        glyph: glyphs::CALC,
    },
    CommandEntry {
        id: "pomo",
        detail: "Pomodoro focus timer",
        glyph: glyphs::TIMER,
    },
    CommandEntry {
        id: "todo",
        detail: "Daily tasks & progress",
        glyph: glyphs::LIST_CHECKS,
    },
    CommandEntry {
        id: "speed",
        detail: "Measure internet download, upload, and latency",
        glyph: glyphs::GAUGE,
    },
    CommandEntry {
        id: "kill",
        detail: "Force kill app or process by name, port, or PID",
        glyph: glyphs::X_CIRCLE,
    },
    CommandEntry {
        id: "shell",
        detail: "Run a shell command",
        glyph: glyphs::TERMINAL,
    },
    CommandEntry {
        id: "sys",
        detail: "Show system information",
        glyph: glyphs::INFO,
    },
];

/// The `"` menu, narrowed by what follows the quote. A substring of the
/// prefix, its display form or its description, so `"folder` finds `d"`.
pub fn prefix_rows(filter: &str) -> Vec<Row> {
    let filter = filter.trim().to_lowercase();
    let entries: Vec<&PrefixEntry> = PREFIX_ENTRIES
        .iter()
        .filter(|e| {
            filter.is_empty()
                || e.prefix.to_lowercase().contains(&filter)
                || format!("{}{}", e.prefix, e.arg_hint)
                    .to_lowercase()
                    .contains(&filter)
                || e.description.to_lowercase().contains(&filter)
        })
        .collect();
    let count = entries.len() as i64;
    entries
        .into_iter()
        .enumerate()
        .map(|(i, e)| Row::prefix_hint(e.prefix, e.arg_hint, e.description, count - i as i64))
        .collect()
}

/// The `:` menu: `:end` or `:process` both surface `kill`.
pub fn command_rows(filter: &str) -> Vec<Row> {
    let filter = filter.trim().to_lowercase();
    let entries: Vec<&CommandEntry> = COMMAND_ENTRIES
        .iter()
        .filter(|e| {
            filter.is_empty() || e.id.contains(&filter) || e.detail.to_lowercase().contains(&filter)
        })
        .collect();
    let count = entries.len() as i64;
    entries
        .into_iter()
        .enumerate()
        .map(|(i, e)| Row::command_hint(e.id, e.detail, e.glyph, count - i as i64))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefixes_resolve_to_their_modes() {
        assert_eq!(Mode::of("firefox"), (Mode::Search, "firefox"));
        assert_eq!(Mode::of("\"fold"), (Mode::PrefixMenu, "fold"));
        assert_eq!(Mode::of(":ki"), (Mode::CommandMenu, "ki"));
        assert_eq!(Mode::of("c\"mail"), (Mode::Clipboard, "mail"));
        assert_eq!(Mode::of("ci\""), (Mode::ClipboardImage, ""));
        assert_eq!(Mode::of("t\"xin chào"), (Mode::Translate, "xin chào"));
        assert_eq!(Mode::of("ps\"node"), (Mode::Process, "node"));
        assert_eq!(Mode::of("rc\""), (Mode::Recent, ""));
    }

    #[test]
    fn the_prefix_menu_finds_by_intent() {
        let rows = prefix_rows("folder");
        assert_eq!(rows.len(), 2, "d\" and rc\" both mention folders");
        assert!(rows[0].title.starts_with("d\""));
        assert_eq!(prefix_rows("").len(), PREFIX_ENTRIES.len());
    }

    #[test]
    fn the_command_menu_matches_details() {
        let rows = command_rows("process");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].title, "kill");
    }
}
