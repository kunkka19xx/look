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
    /// Where `style` was verified.
    pub source: &'static str,
    /// Why the row exists.
    pub note: &'static str,
}

pub const TERMINALS: &[Terminal] = &[
    Terminal {
        names: &["kitty"],
        style: CommandStyle::Argv(POSITIONAL),
        source: "https://sw.kovidgoyal.net/kitty/invocation/",
        note: "\"kitty [options] [program-to-run ...]\": positional, no -e exists",
    },
    Terminal {
        names: &["foot"],
        style: CommandStyle::Argv(POSITIONAL),
        source: "https://man.archlinux.org/man/foot.1.en",
        note: "\"All trailing (non-option) arguments are treated as a command\"",
    },
    Terminal {
        names: &["wezterm"],
        style: CommandStyle::Argv(WEZTERM_START),
        source: "https://wezterm.org/cli/start.html",
        note: "\"wezterm start [OPTIONS] [PROG]...\", e.g. `wezterm start -- bash -l`",
    },
    Terminal {
        names: &["gnome-terminal"],
        style: CommandStyle::Argv(DASH_DASH),
        source: "https://bugs.launchpad.net/ubuntu/+source/gnome-terminal/+bug/1726380",
        note: "-e is deprecated upstream and warns; -- is the supported spelling",
    },
    Terminal {
        names: &["ptyxis"],
        style: CommandStyle::Argv(DASH_DASH),
        source: "https://man.archlinux.org/man/ptyxis.1.en",
        note: "\"In general, you should use -- instead of\" --execute",
    },
    Terminal {
        names: &["terminal", "apple terminal"],
        style: CommandStyle::Script(AppleScript::TerminalApp),
        source: "https://support.apple.com/guide/terminal/welcome/mac",
        note: "an application rather than a CLI",
    },
    Terminal {
        names: &["iterm", "iterm2"],
        style: CommandStyle::Script(AppleScript::ITerm2),
        source: "https://iterm2.com/documentation-scripting.html",
        note: "an application rather than a CLI; its AppleScript name is still iTerm",
    },
    Terminal {
        names: &["warp", "hyper"],
        style: CommandStyle::Unsupported,
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

fn normalize(tool: &str) -> String {
    let lowered = tool.trim().to_ascii_lowercase();
    lowered.strip_suffix(".app").unwrap_or(&lowered).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn unknown_terminal_falls_back_to_dash_e() {
        assert_eq!(command_style("rio"), CommandStyle::Argv(DASH_E));
        assert!(entry("rio").is_none());
    }

    #[test]
    fn convention_followers_need_no_entry() {
        for terminal in [
            "ghostty",
            "alacritty",
            "konsole",
            "xterm",
            "st",
            "urxvt",
            // kgx honors -e: https://manpages.debian.org/testing/gnome-console/kgx.1.en.html
            "kgx",
            "xfce4-terminal",
            "tilix",
            "terminator",
        ] {
            assert_eq!(
                command_style(terminal),
                DEFAULT_COMMAND_STYLE,
                "{terminal} should ride the fallback rather than carry an entry"
            );
        }
    }

    #[test]
    fn deviating_terminals_are_stored() {
        assert_eq!(command_style("kitty"), CommandStyle::Argv(POSITIONAL));
        assert_eq!(command_style("foot"), CommandStyle::Argv(POSITIONAL));
        assert_eq!(command_style("wezterm"), CommandStyle::Argv(WEZTERM_START));
        assert_eq!(
            command_style("gnome-terminal"),
            CommandStyle::Argv(DASH_DASH)
        );
        assert_eq!(command_style("ptyxis"), CommandStyle::Argv(DASH_DASH));
    }

    #[test]
    fn macos_apps_use_applescript() {
        assert_eq!(
            command_style("Terminal.app"),
            CommandStyle::Script(AppleScript::TerminalApp)
        );
        assert_eq!(
            command_style("iTerm2"),
            CommandStyle::Script(AppleScript::ITerm2)
        );
        assert_eq!(AppleScript::ITerm2.app_name(), "iTerm");
    }

    #[test]
    fn terminals_without_command_support_are_named() {
        assert_eq!(command_style("Warp"), CommandStyle::Unsupported);
        assert_eq!(command_style("hyper"), CommandStyle::Unsupported);
    }

    #[test]
    fn an_alias_reaches_the_same_row() {
        assert_eq!(command_style("iterm"), command_style("iterm2"));
        assert_eq!(command_style("terminal"), command_style("apple terminal"));
    }

    #[test]
    fn tty_editors_are_known() {
        for editor in ["vim", "nvim", "Helix", "hx", "kak", "nano", "micro"] {
            assert_eq!(surface(editor), Surface::Tty, "{editor} needs a terminal");
        }
    }

    #[test]
    fn gui_editors_need_no_entry() {
        for editor in [
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
        ] {
            assert_eq!(
                surface(editor),
                Surface::Gui,
                "{editor} draws its own window"
            );
        }
    }

    #[test]
    fn names_normalize_on_case_and_app_suffix() {
        assert_eq!(command_style("KITTY"), CommandStyle::Argv(POSITIONAL));
        assert_eq!(
            command_style("  WezTerm.app  "),
            CommandStyle::Argv(WEZTERM_START)
        );
    }
}
