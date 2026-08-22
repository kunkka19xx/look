//! What Look knows about the tools a user can name.
//!
//! An exceptions list, not an inventory (`specs/preferred-tools.md` §4). `-e` is
//! the xterm convention and the fallback, and a working directory is composed
//! into the command, so only deviating tools need a row.
//!
//! To add a tool, append to [`TERMINALS`] or [`TTY_TOOLS`]. Nothing else reads
//! anything but these tables. Cite the tool's own docs in `source`, and store
//! names lowercased without `.app`; tests enforce both.

/// How a terminal accepts the command it should run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandStyle {
    /// Argv inserted between the terminal and the command.
    Argv(&'static [&'static str]),
    Script(AppleScript),
    /// Known to offer no way to run a command, so the user is told why.
    Unsupported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppleScript {
    TerminalApp,
    ITerm2,
}

impl AppleScript {
    pub fn app_name(self) -> &'static str {
        match self {
            AppleScript::TerminalApp => "Terminal",
            AppleScript::ITerm2 => "iTerm",
        }
    }
}

const DASH_E: &[&str] = &["-e"];
const POSITIONAL: &[&str] = &[];
const DASH_DASH: &[&str] = &["--"];
const WEZTERM_START: &[&str] = &["start", "--"];

pub const DEFAULT_COMMAND_STYLE: CommandStyle = CommandStyle::Argv(DASH_E);

/// One terminal that deviates from `-e`.
#[derive(Clone, Copy, Debug)]
pub struct Terminal {
    /// Every name selecting this row, already normalized.
    pub names: &'static [&'static str],
    pub style: CommandStyle,
    /// A single-instance macOS `.app` that has to be started through
    /// `open -na <app> --args`. Running its CLI directly works only while the
    /// app is not already up; afterwards the process exits without ever making
    /// a window, which reads as the key doing nothing.
    pub macos_app: Option<&'static str>,
    /// Where `style` was verified.
    pub source: &'static str,
    /// Why the row exists.
    pub note: &'static str,
}

pub const TERMINALS: &[Terminal] = &[
    Terminal {
        names: &["ghostty"],
        style: CommandStyle::Argv(DASH_E),
        macos_app: Some("Ghostty"),
        source: "https://github.com/ghostty-org/ghostty/discussions/9221",
        note: "honors -e, but on macOS it is single-instance: running the CLI while the \
               app is already up exits without ever making a window",
    },
    Terminal {
        names: &["kitty"],
        style: CommandStyle::Argv(POSITIONAL),
        macos_app: None,
        source: "https://sw.kovidgoyal.net/kitty/invocation/",
        note: "\"kitty [options] [program-to-run ...]\": positional, no -e exists",
    },
    Terminal {
        names: &["foot"],
        style: CommandStyle::Argv(POSITIONAL),
        macos_app: None,
        source: "https://man.archlinux.org/man/foot.1.en",
        note: "\"All trailing (non-option) arguments are treated as a command\"",
    },
    Terminal {
        names: &["wezterm"],
        style: CommandStyle::Argv(WEZTERM_START),
        macos_app: None,
        source: "https://wezterm.org/cli/start.html",
        note: "\"wezterm start [OPTIONS] [PROG]...\", e.g. `wezterm start -- bash -l`",
    },
    Terminal {
        names: &["gnome-terminal"],
        style: CommandStyle::Argv(DASH_DASH),
        macos_app: None,
        source: "https://bugs.launchpad.net/ubuntu/+source/gnome-terminal/+bug/1726380",
        note: "-e is deprecated upstream and warns; -- is the supported spelling",
    },
    Terminal {
        names: &["ptyxis"],
        style: CommandStyle::Argv(DASH_DASH),
        macos_app: None,
        source: "https://man.archlinux.org/man/ptyxis.1.en",
        note: "\"In general, you should use -- instead of\" --execute",
    },
    Terminal {
        names: &["terminal", "apple terminal"],
        style: CommandStyle::Script(AppleScript::TerminalApp),
        macos_app: None,
        source: "https://support.apple.com/guide/terminal/welcome/mac",
        note: "an application rather than a CLI",
    },
    Terminal {
        names: &["iterm", "iterm2"],
        style: CommandStyle::Script(AppleScript::ITerm2),
        macos_app: None,
        source: "https://iterm2.com/documentation-scripting.html",
        note: "an application rather than a CLI; its AppleScript name is still iTerm",
    },
    Terminal {
        names: &["warp", "hyper"],
        style: CommandStyle::Unsupported,
        macos_app: None,
        source: "https://docs.warp.dev/",
        note: "neither documents a way to open a window running a given command",
    },
];

pub fn command_style(terminal: &str) -> CommandStyle {
    entry(terminal)
        .map(|found| found.style)
        .unwrap_or(DEFAULT_COMMAND_STYLE)
}

/// The row for `terminal`, or `None` when it rides the fallback.
pub fn entry(terminal: &str) -> Option<&'static Terminal> {
    let name = normalize(terminal);
    TERMINALS
        .iter()
        .find(|candidate| candidate.names.contains(&name.as_str()))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    Gui,
    Tty,
}

/// Editors needing a terminal. `emacs` is absent on purpose: `emacs <path>`
/// starts the GUI build, and wanting `emacs -nw` is declaring a command rather
/// than a tool, which is what a source block is for.
pub const TTY_TOOLS: &[&str] = &[
    "vi", "vim", "nvim", "neovim", "hx", "helix", "kak", "kakoune", "nano", "micro", "pico", "ed",
];

pub fn surface(tool: &str) -> Surface {
    if TTY_TOOLS.contains(&normalize(tool).as_str()) {
        Surface::Tty
    } else {
        Surface::Gui
    }
}

/// A declared name reduced to a catalog key. Case, a trailing `.app`, and a
/// leading directory are things a user should not have to get right: naming a
/// specific build (`/opt/homebrew/bin/nvim`) still means nvim.
fn normalize(tool: &str) -> String {
    let trimmed = tool.trim();
    let name = trimmed.rsplit('/').next().unwrap_or(trimmed);
    let lowered = name.to_ascii_lowercase();
    lowered.strip_suffix(".app").unwrap_or(&lowered).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    // These three police contributions to the tables, so a bad new row fails the
    // build rather than failing quietly on someone's machine.

    #[test]
    fn every_entry_cites_its_source() {
        for terminal in TERMINALS {
            assert!(
                terminal.source.starts_with("https://"),
                "{:?} must cite the page its flag was read from",
                terminal.names
            );
            assert!(
                !terminal.note.is_empty(),
                "{:?} must say why it needs an entry",
                terminal.names
            );
        }
    }

    #[test]
    fn names_are_stored_normalized() {
        let stored = TERMINALS
            .iter()
            .flat_map(|terminal| terminal.names.iter())
            .chain(TTY_TOOLS.iter());
        for name in stored {
            assert_eq!(
                &normalize(name),
                name,
                "{name:?} is stored in a form lookup can never match"
            );
        }
    }

    #[test]
    fn no_name_is_claimed_twice() {
        let mut seen: Vec<&str> = Vec::new();
        for name in TERMINALS.iter().flat_map(|terminal| terminal.names.iter()) {
            assert!(
                !seen.contains(name),
                "{name:?} appears twice, so one of the rows is unreachable"
            );
            seen.push(name);
        }
    }

    /// (declared name, expected style, why this row exists).
    ///
    /// The fallback rows are the load-bearing ones: every terminal riding `-e`
    /// works with no catalog entry at all, which is what keeps the table an
    /// exceptions list. `kgx` honors it too, per its Debian man page.
    #[test]
    fn a_declared_name_resolves_to_its_style() {
        const FALLBACK: CommandStyle = DEFAULT_COMMAND_STYLE;
        const POSITIONAL_STYLE: CommandStyle = CommandStyle::Argv(POSITIONAL);
        const DASHDASH: CommandStyle = CommandStyle::Argv(DASH_DASH);
        const WEZ: CommandStyle = CommandStyle::Argv(WEZTERM_START);
        const TERM_APP: CommandStyle = CommandStyle::Script(AppleScript::TerminalApp);
        const ITERM: CommandStyle = CommandStyle::Script(AppleScript::ITerm2);
        const NONE: CommandStyle = CommandStyle::Unsupported;

        let cases: &[(&str, CommandStyle, &str)] = &[
            ("rio", FALLBACK, "unknown rides the fallback"),
            ("alacritty", FALLBACK, "honors -e"),
            ("konsole", FALLBACK, "honors -e"),
            ("xterm", FALLBACK, "honors -e"),
            ("st", FALLBACK, "honors -e"),
            ("urxvt", FALLBACK, "honors -e"),
            ("kgx", FALLBACK, "honors -e"),
            ("xfce4-terminal", FALLBACK, "honors -e"),
            ("tilix", FALLBACK, "honors -e"),
            ("terminator", FALLBACK, "honors -e"),
            ("kitty", POSITIONAL_STYLE, "positional, no -e exists"),
            ("foot", POSITIONAL_STYLE, "trailing args are the command"),
            ("wezterm", WEZ, "runs through its start subcommand"),
            ("gnome-terminal", DASHDASH, "-e is deprecated upstream"),
            ("ptyxis", DASHDASH, "-- is the supported spelling"),
            ("Terminal.app", TERM_APP, "an app, not a CLI"),
            ("apple terminal", TERM_APP, "alias of the same row"),
            ("iterm", ITERM, "an app, not a CLI"),
            ("iTerm2", ITERM, "alias of the same row"),
            ("Warp", NONE, "cannot be told to run a command"),
            ("hyper", NONE, "cannot be told to run a command"),
            ("KITTY", POSITIONAL_STYLE, "case is normalized"),
            ("  WezTerm.app  ", WEZ, "whitespace and .app are normalized"),
            (
                "/opt/homebrew/bin/kitty",
                POSITIONAL_STYLE,
                "a leading path is normalized",
            ),
        ];

        for (declared, want, why) in cases {
            assert_eq!(command_style(declared), *want, "{declared:?} ({why})");
        }
    }

    #[test]
    fn a_terminal_riding_the_fallback_has_no_row() {
        assert!(entry("rio").is_none());
        assert!(entry("alacritty").is_none());
        assert!(entry("kitty").is_some());
    }

    /// Every GUI editor rides the fallback, including ones that did not exist
    /// when this was written: a new editor ships and Look supports it without a
    /// release. `emacs` is deliberately GUI, since `emacs <path>` starts the GUI
    /// build and wanting `emacs -nw` is declaring a command, not a tool.
    #[test]
    fn a_tool_is_tty_only_when_the_catalog_says_so() {
        let tty = [
            "vim",
            "nvim",
            "Helix",
            "hx",
            "kak",
            "nano",
            "micro",
            "/opt/homebrew/bin/nvim",
            "/opt/my tools/nvim",
        ];
        let gui = [
            "zed",
            "vscode",
            "code",
            "cursor",
            "windsurf",
            "sublime",
            "xcode",
            "textmate",
            "emacs",
            "something-nobody-has-shipped-yet",
        ];

        for tool in tty {
            assert_eq!(surface(tool), Surface::Tty, "{tool:?} needs a terminal");
        }
        for tool in gui {
            assert_eq!(surface(tool), Surface::Gui, "{tool:?} draws its own window");
        }
    }

    #[test]
    fn an_applescript_dialect_knows_its_app_name() {
        assert_eq!(AppleScript::TerminalApp.app_name(), "Terminal");
        assert_eq!(AppleScript::ITerm2.app_name(), "iTerm");
    }
}
