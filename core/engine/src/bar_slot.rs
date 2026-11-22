//! What the right end of the search bar holds. The shells draw it; the
//! names, the old key's fallback and the script's limits live here.

use std::collections::HashMap;
use std::time::Duration;

pub const SLOT_KEY: &str = "bar_slot";
pub const PINNED_APPS_KEY: &str = "pinned_apps";
pub const SCRIPT_KEY: &str = "bar_script";
/// Read when `bar_slot` is absent, never written.
pub const LEGACY_PLACEMENT_KEY: &str = "running_apps_placement";
const LEGACY_PLACEMENT_NONE: &str = "none";

/// One per digit key.
pub const MAX_PINNED: usize = 9;

const SCRIPT_TIMEOUT: Duration = Duration::from_secs(2);
const SCRIPT_MAX_BYTES: usize = 256;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BarSlot {
    #[default]
    Running,
    Pinned,
    Todo,
    Script,
    None,
}

impl BarSlot {
    pub const ALL: [BarSlot; 5] = [
        BarSlot::Running,
        BarSlot::Pinned,
        BarSlot::Todo,
        BarSlot::Script,
        BarSlot::None,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            BarSlot::Running => "running",
            BarSlot::Pinned => "pinned",
            BarSlot::Todo => "todo",
            BarSlot::Script => "script",
            BarSlot::None => "none",
        }
    }

    /// An unknown value reads as the default.
    pub fn from_entries(entries: &HashMap<String, String>) -> Self {
        let value = |key: &str| {
            entries
                .get(key)
                .map(|v| v.trim().to_ascii_lowercase())
                .filter(|v| !v.is_empty())
        };
        match value(SLOT_KEY) {
            Some(slot) => Self::ALL
                .into_iter()
                .find(|s| s.as_str() == slot)
                .unwrap_or_default(),
            None if value(LEGACY_PLACEMENT_KEY).as_deref() == Some(LEGACY_PLACEMENT_NONE) => {
                BarSlot::None
            }
            None => BarSlot::default(),
        }
    }
}

pub fn pinned_ids(value: &str) -> Vec<String> {
    let mut ids = crate::config::parse_csv(value);
    ids.truncate(MAX_PINNED);
    ids
}

/// The first line `command` prints, empty when it printed nothing. Blocks
/// for up to the timeout.
pub fn script_line(command: &str) -> Result<String, String> {
    first_line(command, SCRIPT_TIMEOUT)
}

fn first_line(command: &str, timeout: Duration) -> Result<String, String> {
    let output = look_sources::capture(command, None, timeout, SCRIPT_MAX_BYTES)?;
    let plain = without_terminal_codes(&output);
    Ok(plain
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default()
        .to_string())
}

const ESCAPE: char = '\u{1b}';
const BELL: char = '\u{7}';

/// A shell config that sets the terminal title prints one ahead of the output.
fn without_terminal_codes(text: &str) -> String {
    let mut plain = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != ESCAPE {
            if ch == '\n' || !ch.is_control() {
                plain.push(ch);
            }
            continue;
        }
        match chars.next() {
            // Title: closed by a bell or `ESC \`.
            Some(']') => {
                while let Some(inner) = chars.next() {
                    if inner == BELL || (inner == ESCAPE && chars.next_if_eq(&'\\').is_some()) {
                        break;
                    }
                }
            }
            // Color and cursor: closed by the first byte from `@` on.
            Some('[') => {
                for inner in chars.by_ref() {
                    if ('@'..='~').contains(&inner) {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    plain
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A loaded CI runner can take longer than the bar's own timeout to
    /// start a shell.
    const PATIENT: Duration = Duration::from_secs(30);

    fn slot(pairs: &[(&str, &str)]) -> BarSlot {
        let entries = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        BarSlot::from_entries(&entries)
    }

    #[test]
    fn every_slot_reads_back_from_its_own_spelling() {
        for s in BarSlot::ALL {
            assert_eq!(slot(&[(SLOT_KEY, s.as_str())]), s);
        }
        assert_eq!(slot(&[(SLOT_KEY, " Todo ")]), BarSlot::Todo);
    }

    #[test]
    fn an_old_config_keeps_the_bar_it_had() {
        assert_eq!(slot(&[]), BarSlot::Running);
        assert_eq!(slot(&[(LEGACY_PLACEMENT_KEY, "none")]), BarSlot::None);
        for on in ["right", "top", "bottom"] {
            assert_eq!(slot(&[(LEGACY_PLACEMENT_KEY, on)]), BarSlot::Running);
        }
        assert_eq!(
            slot(&[(SLOT_KEY, ""), (LEGACY_PLACEMENT_KEY, "none")]),
            BarSlot::None
        );
    }

    #[test]
    fn the_new_key_wins_over_the_old_one() {
        assert_eq!(
            slot(&[(SLOT_KEY, "pinned"), (LEGACY_PLACEMENT_KEY, "none")]),
            BarSlot::Pinned
        );
        assert_eq!(
            slot(&[(SLOT_KEY, "clock"), (LEGACY_PLACEMENT_KEY, "none")]),
            BarSlot::Running
        );
    }

    #[test]
    fn pinned_ids_skip_blanks_and_stop_at_nine() {
        assert_eq!(pinned_ids(" a, ,b,"), vec!["a", "b"]);
        assert!(pinned_ids("").is_empty());
        let ten = (0..10).map(|n| n.to_string()).collect::<Vec<_>>().join(",");
        assert_eq!(pinned_ids(&ten).len(), MAX_PINNED);
        assert_eq!(pinned_ids(&ten)[0], "0");
    }

    #[test]
    fn the_script_gives_its_first_line() {
        assert_eq!(first_line("echo one&& echo two", PATIENT).unwrap(), "one");
    }

    #[test]
    fn a_script_that_prints_nothing_gives_an_empty_line() {
        assert_eq!(first_line("exit 0", PATIENT).unwrap(), "");
    }

    #[test]
    fn terminal_codes_do_not_reach_the_bar() {
        assert_eq!(without_terminal_codes("\u{1b}]0;title\u{7}09:41"), "09:41");
        assert_eq!(
            without_terminal_codes("\u{1b}]0;title\u{1b}\\09:41"),
            "09:41"
        );
        assert_eq!(
            without_terminal_codes("\u{1b}[1;32mok\u{1b}[0m\r\n2"),
            "ok\n2"
        );
    }

    #[test]
    fn a_failing_script_says_why() {
        #[cfg(not(windows))]
        let failing = "echo boom >&2; exit 1";
        #[cfg(windows)]
        let failing = "echo boom 1>&2& exit 1";
        assert_eq!(first_line(failing, PATIENT).unwrap_err(), "boom");
    }

    #[cfg(unix)]
    #[test]
    fn a_script_that_overruns_is_stopped() {
        let err = first_line("sleep 5", Duration::from_millis(100)).unwrap_err();
        assert!(err.starts_with("timed out"), "{err}");
    }
}
