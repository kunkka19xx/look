//! How a Windows terminal is asked to open in a directory, and to run a command
//! there.
//!
//! Windows has no shell to compose through, so unlike the POSIX `-e` convention
//! in `catalog.rs` there is nothing shared to fall back on except the working
//! directory a console host is started in. That is the fallback, and it is
//! exactly right for `cmd` and `powershell`.
//!
//! To add a terminal, append to [`WINDOWS_TERMINALS`] and cite the page its
//! flags were read from; the tests enforce both.

/// Replaced by the directory the window should open in.
const DIR: &str = "{dir}";

pub struct WindowsTerminal {
    /// Every name selecting this row, already normalized.
    pub names: &'static [&'static str],
    /// Argv opening a window in [`DIR`]. Empty inherits the started-in directory.
    pub in_directory: &'static [&'static str],
    /// What separates those flags from a command to run in the new window.
    /// `None` when the terminal documents no way to run one.
    pub then_run: Option<&'static [&'static str]>,
    /// Where the argv was verified.
    pub source: &'static str,
}

/// A terminal with no row of its own. `then_run` is `None` rather than a guess:
/// a console host handed an argument it does not parse prints usage instead of
/// running anything.
static INHERITS_DIRECTORY: WindowsTerminal = WindowsTerminal {
    names: &[],
    in_directory: &[],
    then_run: None,
    source: "",
};

pub static WINDOWS_TERMINALS: &[WindowsTerminal] = &[
    WindowsTerminal {
        names: &["wt", "windows terminal"],
        // Needed rather than inherited: a profile with its own
        // `startingDirectory` ignores the directory wt was started in.
        in_directory: &["-d", DIR],
        // The command follows the options directly: `wt -d <dir> <cmd> <args>`.
        then_run: Some(&[]),
        source: "https://learn.microsoft.com/windows/terminal/command-line-arguments",
    },
    WindowsTerminal {
        names: &["alacritty"],
        in_directory: &["--working-directory", DIR],
        then_run: Some(&["-e"]),
        source: "https://man.archlinux.org/man/alacritty.1.en",
    },
    WindowsTerminal {
        names: &["wezterm", "wezterm-gui"],
        in_directory: &["start", "--cwd", DIR],
        then_run: Some(&["--"]),
        source: "https://wezterm.org/cli/start.html",
    },
    WindowsTerminal {
        names: &["pwsh"],
        in_directory: &["-WorkingDirectory", DIR],
        // `-Command` takes the rest of the line as one script, which is a
        // command string rather than an argv: that is a source block's job.
        then_run: None,
        source: "https://learn.microsoft.com/powershell/module/microsoft.powershell.core/about/about_pwsh",
    },
];

/// The argv for `terminal`, opening in `dir` and running `command` there when
/// one is given. `None` when a command was asked for and this terminal has no
/// documented way to run one.
pub fn argv(terminal: &str, dir: &str, command: &[&str]) -> Option<Vec<String>> {
    let found = entry(terminal);
    let mut argv: Vec<String> = found
        .in_directory
        .iter()
        .map(|part| if *part == DIR { dir } else { *part }.to_string())
        .collect();

    if command.is_empty() {
        return Some(argv);
    }
    argv.extend(
        found
            .then_run?
            .iter()
            .chain(command)
            .map(|part| part.to_string()),
    );
    Some(argv)
}

/// The row for `terminal`, or the fallback that inherits its directory.
fn entry(terminal: &str) -> &'static WindowsTerminal {
    let name = normalize(terminal);
    WINDOWS_TERMINALS
        .iter()
        .find(|candidate| candidate.names.contains(&name.as_str()))
        .unwrap_or(&INHERITS_DIRECTORY)
}

/// A declared name reduced to a table key: `C:\Tools\wt.exe` still means wt.
fn normalize(terminal: &str) -> String {
    let trimmed = terminal.trim();
    let name = trimmed.rsplit(['/', '\\']).next().unwrap_or(trimmed);
    let lowered = name.to_ascii_lowercase();
    lowered.strip_suffix(".exe").unwrap_or(&lowered).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owned(parts: &[&str]) -> Option<Vec<String>> {
        Some(parts.iter().map(|part| (*part).to_string()).collect())
    }

    /// Polices contributions to the table, so a bad new row fails the build
    /// rather than failing quietly on someone's machine.
    #[test]
    fn every_row_cites_its_source_and_places_the_directory() {
        for terminal in WINDOWS_TERMINALS {
            assert!(
                terminal.source.starts_with("https://"),
                "{:?} must cite the page its argv was read from",
                terminal.names
            );
            assert!(
                !terminal.names.is_empty(),
                "a row nothing can select is dead weight"
            );
            assert!(
                terminal.in_directory.contains(&DIR),
                "{:?} declares flags but never places the directory",
                terminal.names
            );
            for name in terminal.names {
                assert_eq!(
                    &normalize(name),
                    name,
                    "{name:?} must already be normalized"
                );
            }
        }
    }

    #[test]
    fn a_listed_terminal_is_handed_the_directory_its_own_way() {
        let cases: &[(&str, &[&str])] = &[
            ("wt", &["-d", "C:\\look"]),
            ("alacritty", &["--working-directory", "C:\\look"]),
            ("wezterm", &["start", "--cwd", "C:\\look"]),
            ("pwsh", &["-WorkingDirectory", "C:\\look"]),
        ];

        for (terminal, expected) in cases {
            assert_eq!(
                argv(terminal, "C:\\look", &[]),
                owned(expected),
                "{terminal}"
            );
        }
    }

    /// cmd and powershell take no directory flag; being started in one is what
    /// opens them there, so the argv is empty rather than a guess.
    #[test]
    fn an_unlisted_terminal_relies_on_the_directory_it_is_started_in() {
        for terminal in ["cmd", "powershell", "conhost", "some-new-terminal"] {
            assert_eq!(
                argv(terminal, "C:\\look", &[]),
                Some(Vec::new()),
                "{terminal}"
            );
        }
    }

    #[test]
    fn a_command_follows_the_separator_its_terminal_documents() {
        let editor = ["nvim", "C:\\look\\a.txt"];
        assert_eq!(
            argv("wt", "C:\\look", &editor),
            owned(&["-d", "C:\\look", "nvim", "C:\\look\\a.txt"])
        );
        assert_eq!(
            argv("wezterm", "C:\\look", &editor),
            owned(&[
                "start",
                "--cwd",
                "C:\\look",
                "--",
                "nvim",
                "C:\\look\\a.txt"
            ])
        );
    }

    /// A terminal that cannot be told to run a command says so rather than
    /// opening a window that ignores the editor the user asked for.
    #[test]
    fn a_terminal_with_no_way_to_run_one_refuses_the_command() {
        assert_eq!(argv("pwsh", "C:\\look", &["nvim", "a.txt"]), None);
        assert_eq!(argv("cmd", "C:\\look", &["nvim", "a.txt"]), None);
        // The same terminal still opens in the directory on its own.
        assert!(argv("cmd", "C:\\look", &[]).is_some());
    }

    #[test]
    fn a_name_is_matched_however_it_was_spelled() {
        for spelling in ["WT", " wt.exe ", "C:\\Tools\\wt.exe", "C:/Tools/wt.EXE"] {
            assert_eq!(
                argv(spelling, "C:\\look", &[]),
                owned(&["-d", "C:\\look"]),
                "{spelling:?}"
            );
        }
    }
}
