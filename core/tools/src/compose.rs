//! Turning an action plus an object into one thing the shell can launch.

use std::path::Path;

use crate::catalog::{AppleScript, CommandStyle, Surface, command_style, entry, surface};
use crate::quote::{applescript_quote, shell_quote};
use crate::{Tools, key};

/// Expanded by the inner shell, not by whatever spawned the launcher.
const SHELL_VAR: &str = "\"$SHELL\"";
const LOGIN_COMMAND_FLAGS: &str = "-lc";
/// Replaces the wrapper so closing the shell closes the window once, not twice.
const INTERACTIVE_TAIL: &str = "exec \"$SHELL\" -l";
const OSASCRIPT: &str = "osascript";
const OSASCRIPT_EXPRESSION: &str = "-e";
/// Where a path with no directory of its own lives.
const CURRENT_DIR: &str = ".";
/// `-n` is what makes this work at all: plain `open -a` only focuses an app that
/// is already running and drops the arguments.
const MACOS_OPEN: &str = "open -na";
const MACOS_OPEN_ARGS: &str = "--args";

#[cfg(target_os = "macos")]
const IS_MACOS: bool = true;
#[cfg(not(target_os = "macos"))]
const IS_MACOS: bool = false;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    File(String),
    Folder(String),
}

impl Target {
    pub fn path(&self) -> &str {
        match self {
            Target::File(path) | Target::Folder(path) => path,
        }
    }

    /// A folder is its own directory, a file's is its parent. Lexical: core
    /// never touches the filesystem to answer this.
    pub fn dir(&self) -> String {
        match self {
            Target::Folder(path) => path.clone(),
            // A bare filename has an empty parent and a root path has none.
            // Either would compose `cd ''`, which fails in the new window.
            Target::File(path) => Path::new(path)
                .parent()
                .map(|parent| parent.to_string_lossy().into_owned())
                .filter(|parent| !parent.is_empty())
                .unwrap_or_else(|| CURRENT_DIR.to_string()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Launch {
    Shell {
        tool: String,
        command: String,
    },
    /// For the platform's application launcher (`open -a`), since a GUI app may
    /// ship no CLI and only the native side can find a bundle.
    Application {
        tool: String,
        path: String,
    },
    /// No tool declared, so the platform's own handler does it. This is today's
    /// behavior, and the reason declaring nothing changes nothing.
    SystemDefault {
        path: String,
    },
}

impl Launch {
    pub fn tool(&self) -> Option<&str> {
        match self {
            Launch::Shell { tool, .. } | Launch::Application { tool, .. } => Some(tool),
            Launch::SystemDefault { .. } => None,
        }
    }
}

/// Every variant names something the user can act on: an action that is merely
/// absent reads as the feature being broken.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unavailable {
    NotDeclared {
        key: &'static str,
    },
    TerminalRequired {
        tool: String,
        key: &'static str,
    },
    /// A terminal declared where an editor was meant. Named rather than tried:
    /// a terminal handed a path exits on the argument it cannot read, which
    /// reads as the action doing nothing at all.
    NotAnEditor {
        tool: String,
        key: &'static str,
    },
    CannotRunCommand {
        tool: String,
    },
    UnsupportedPlatform,
}

impl Unavailable {
    /// Shown as-is, so both shells word it the same way.
    pub fn message(&self) -> String {
        match self {
            Unavailable::NotDeclared { key } => format!("Set {key} in your Look config"),
            Unavailable::TerminalRequired { tool, key } => {
                format!("{tool} runs in a terminal; set {key} in your Look config")
            }
            Unavailable::NotAnEditor { tool, key } => {
                format!("{tool} is a terminal; set {key} to the editor it should run")
            }
            Unavailable::CannotRunCommand { tool } => {
                format!("{tool} cannot be told to run a command")
            }
            Unavailable::UnsupportedPlatform => {
                "This platform has no POSIX shell to run actions through".to_string()
            }
        }
    }

    /// The config key that would fix this, when one would.
    pub fn key(&self) -> Option<&'static str> {
        match self {
            Unavailable::NotDeclared { key }
            | Unavailable::TerminalRequired { key, .. }
            | Unavailable::NotAnEditor { key, .. } => Some(key),
            Unavailable::CannotRunCommand { .. } | Unavailable::UnsupportedPlatform => None,
        }
    }
}

/// What Look can do to a row through a declared tool. Ids are shared so every
/// shell names the same action the same way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Edit,
    TerminalHere,
    Reveal,
}

impl Action {
    pub const ALL: &'static [Action] = &[Action::Edit, Action::TerminalHere, Action::Reveal];

    pub const fn id(self) -> &'static str {
        match self {
            Action::Edit => "edit",
            Action::TerminalHere => "terminal",
            Action::Reveal => "reveal",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|action| action.id() == id)
    }

    pub fn resolve(self, tools: &Tools, target: &Target) -> Result<Launch, Unavailable> {
        match self {
            Action::Edit => edit(tools, target),
            Action::TerminalHere => terminal_here(tools, target),
            Action::Reveal => Ok(reveal(tools, target)),
        }
    }
}

/// Show the target in a file manager. Undeclared means the platform's own, which
/// is what the launcher does today, so this never fails.
pub fn reveal(tools: &Tools, target: &Target) -> Launch {
    let path = target.path().to_string();
    match declared(&tools.file_manager) {
        // A custom manager is opened at the containing folder: only the
        // platform's own can select the file inside it.
        Some(manager) => Launch::Application {
            tool: manager.to_string(),
            path: target.dir(),
        },
        None => Launch::SystemDefault { path },
    }
}

/// Composition emits POSIX shell text down to its quoting, so a platform without
/// a POSIX shell is refused rather than handed text `cmd` would mangle.
#[cfg(unix)]
const POSIX_SHELL_AVAILABLE: bool = true;
#[cfg(not(unix))]
const POSIX_SHELL_AVAILABLE: bool = false;

/// `text_editor` for a file, `code_editor` for a folder; declaring one covers
/// both.
pub fn edit(tools: &Tools, target: &Target) -> Result<Launch, Unavailable> {
    let (preferred, fallback, missing) = match target {
        Target::File(_) => (&tools.text_editor, &tools.code_editor, key::TEXT_EDITOR),
        Target::Folder(_) => (&tools.code_editor, &tools.text_editor, key::CODE_EDITOR),
    };

    // Filtered before the choice, not after: a blank preferred key must fall
    // through to the other editor rather than swallow it.
    let editor = declared(preferred)
        .or_else(|| declared(fallback))
        .ok_or(Unavailable::NotDeclared { key: missing })?;

    match surface(editor) {
        Surface::Terminal => Err(Unavailable::NotAnEditor {
            tool: editor.to_string(),
            key: missing,
        }),
        Surface::Gui => Ok(Launch::Application {
            tool: editor.to_string(),
            path: target.path().to_string(),
        }),
        Surface::Tty => {
            // Quoted like the path: a tool declared as `/opt/my tools/nvim` is
            // one word. A tool is a name, never a command with its own args.
            let inner = format!("{} {}", shell_quote(editor), shell_quote(target.path()));
            in_terminal(tools, &inner, editor)
        }
    }
}

/// A declared name, trimmed, or `None` when the key is absent or blank.
fn declared(value: &Option<String>) -> Option<&str> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
}

pub fn terminal_here(tools: &Tools, target: &Target) -> Result<Launch, Unavailable> {
    let inner = format!("cd {} && {INTERACTIVE_TAIL}", shell_quote(&target.dir()));
    in_terminal(tools, &inner, "")
}

/// `requester` is the TTY tool needing a host, or empty when the terminal is the
/// point of the action.
fn in_terminal(tools: &Tools, inner: &str, requester: &str) -> Result<Launch, Unavailable> {
    if !POSIX_SHELL_AVAILABLE {
        return Err(Unavailable::UnsupportedPlatform);
    }

    let terminal = declared(&tools.terminal).ok_or_else(|| {
        if requester.is_empty() {
            Unavailable::NotDeclared { key: key::TERMINAL }
        } else {
            Unavailable::TerminalRequired {
                tool: requester.to_string(),
                key: key::TERMINAL,
            }
        }
    })?;

    let command = match command_style(terminal) {
        CommandStyle::Argv(prefix) => match macos_app(terminal) {
            Some(app) => open_command(app, prefix, inner),
            None => argv_command(terminal, prefix, inner),
        },
        CommandStyle::Script(dialect) => applescript_command(dialect, inner),
        CommandStyle::Unsupported => {
            return Err(Unavailable::CannotRunCommand {
                tool: terminal.to_string(),
            });
        }
    };

    Ok(Launch::Shell {
        tool: terminal.to_string(),
        command,
    })
}

/// `<terminal> <prefix...> "$SHELL" -lc '<inner>'`. The inner login shell is what
/// makes `nvim` and Homebrew resolve from a window-server-launched app.
fn argv_command(terminal: &str, prefix: &[&str], inner: &str) -> String {
    command_line(&shell_quote(terminal), prefix, inner)
}

/// The same argv, handed to a single-instance macOS app through `open -na`.
fn open_command(app: &str, prefix: &[&str], inner: &str) -> String {
    command_line(
        &format!("{MACOS_OPEN} {} {MACOS_OPEN_ARGS}", shell_quote(app)),
        prefix,
        inner,
    )
}

fn command_line(launcher: &str, prefix: &[&str], inner: &str) -> String {
    let mut parts = vec![launcher.to_string()];
    parts.extend(prefix.iter().map(|argument| (*argument).to_string()));
    parts.push(SHELL_VAR.to_string());
    parts.push(LOGIN_COMMAND_FLAGS.to_string());
    parts.push(shell_quote(inner));
    parts.join(" ")
}

/// The app name to hand `open -na`, when this terminal needs it and we are on
/// the platform where that matters.
fn macos_app(terminal: &str) -> Option<&'static str> {
    if !IS_MACOS {
        return None;
    }
    entry(terminal).and_then(|found| found.macos_app)
}

/// The second expression raises the window, which neither dialect does itself.
fn applescript_command(dialect: AppleScript, inner: &str) -> String {
    let app = dialect.app_name();
    let run = match dialect {
        AppleScript::TerminalApp => format!(
            "tell application {} to do script {}",
            applescript_quote(app),
            applescript_quote(inner)
        ),
        AppleScript::ITerm2 => format!(
            "tell application {} to create window with default profile command {}",
            applescript_quote(app),
            applescript_quote(inner)
        ),
    };
    let activate = format!("tell application {} to activate", applescript_quote(app));
    format!(
        "{OSASCRIPT} {OSASCRIPT_EXPRESSION} {} {OSASCRIPT_EXPRESSION} {}",
        shell_quote(&run),
        shell_quote(&activate)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tools(text: Option<&str>, code: Option<&str>, terminal: Option<&str>) -> Tools {
        Tools {
            text_editor: text.map(str::to_string),
            code_editor: code.map(str::to_string),
            terminal: terminal.map(str::to_string),
            ..Tools::default()
        }
    }

    /// Only a POSIX platform composes shell text, so every caller is gated.
    #[cfg(unix)]
    fn shell_command(outcome: Result<Launch, Unavailable>) -> String {
        match outcome {
            Ok(Launch::Shell { command, .. }) => command,
            other => panic!("expected a shell launch, got {other:?}"),
        }
    }

    fn file() -> Target {
        Target::File("/tmp/a.txt".into())
    }

    fn folder() -> Target {
        Target::Folder("/tmp".into())
    }

    #[test]
    fn nothing_declared_names_the_key_to_set() {
        let none = Tools::default();
        assert_eq!(
            edit(&none, &file()),
            Err(Unavailable::NotDeclared {
                key: key::TEXT_EDITOR
            })
        );
        assert_eq!(
            edit(&none, &folder()),
            Err(Unavailable::NotDeclared {
                key: key::CODE_EDITOR
            })
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_terminal_with_nothing_declared_names_the_terminal_key() {
        assert_eq!(
            terminal_here(&Tools::default(), &folder()),
            Err(Unavailable::NotDeclared { key: key::TERMINAL })
        );
    }

    /// Composition emits POSIX shell text down to its quoting, so a platform
    /// without a POSIX shell is refused outright rather than handed something
    /// `cmd` would mangle. Windows needs its own composition path, not a few
    /// catalog rows.
    #[cfg(not(unix))]
    #[test]
    fn a_platform_without_a_posix_shell_is_refused() {
        assert_eq!(
            terminal_here(&tools(None, None, Some("wt")), &folder()),
            Err(Unavailable::UnsupportedPlatform)
        );
        assert_eq!(
            edit(&tools(Some("nvim"), None, Some("wt")), &file()),
            Err(Unavailable::UnsupportedPlatform)
        );
    }

    #[test]
    fn edit_prefers_the_editor_for_the_row_kind() {
        let both = tools(Some("zed"), Some("cursor"), None);
        assert_eq!(edit(&both, &file()).unwrap().tool(), Some("zed"));
        assert_eq!(edit(&both, &folder()).unwrap().tool(), Some("cursor"));
    }

    #[test]
    fn one_editor_declared_serves_both_row_kinds() {
        assert_eq!(
            edit(&tools(None, Some("zed"), None), &file())
                .unwrap()
                .tool(),
            Some("zed")
        );
        assert_eq!(
            edit(&tools(Some("zed"), None, None), &folder())
                .unwrap()
                .tool(),
            Some("zed")
        );
    }

    /// `Option::or` would keep the blank and never consult the fallback, so a
    /// usable editor would be reported as undeclared.
    #[test]
    fn a_blank_key_falls_through_to_the_other_editor() {
        assert_eq!(
            edit(&tools(Some("   "), Some("zed"), None), &file())
                .unwrap()
                .tool(),
            Some("zed")
        );
        assert_eq!(
            edit(&tools(Some("zed"), Some(""), None), &folder())
                .unwrap()
                .tool(),
            Some("zed")
        );
        assert_eq!(
            edit(&tools(Some("   "), None, None), &file()),
            Err(Unavailable::NotDeclared {
                key: key::TEXT_EDITOR
            })
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_tty_editor_needs_a_terminal_and_then_runs_inside_it() {
        assert_eq!(
            edit(&tools(Some("nvim"), None, None), &file()),
            Err(Unavailable::TerminalRequired {
                tool: "nvim".into(),
                key: key::TERMINAL
            })
        );
        assert_eq!(
            edit(&tools(Some("nvim"), None, Some("alacritty")), &file())
                .unwrap()
                .tool(),
            Some("alacritty")
        );
    }

    /// A GUI editor is handed to the platform launcher with the path, since only
    /// native code can find and start a bundle.
    /// The trap this exists for: a terminal handed a path exits on the argument
    /// it cannot read, so the row reads as doing nothing at all.
    #[test]
    fn a_terminal_named_as_an_editor_says_which_key_to_fix() {
        for terminal in ["alacritty", "ghostty", "konsole", "iTerm2"] {
            assert_eq!(
                edit(&tools(Some(terminal), None, Some("ghostty")), &file()),
                Err(Unavailable::NotAnEditor {
                    tool: terminal.into(),
                    key: key::TEXT_EDITOR
                }),
                "{terminal:?} on a file row"
            );
            assert_eq!(
                edit(&tools(None, Some(terminal), None), &folder()),
                Err(Unavailable::NotAnEditor {
                    tool: terminal.into(),
                    key: key::CODE_EDITOR
                }),
                "{terminal:?} on a folder row"
            );
        }
    }

    /// Naming a terminal under `terminal` is the whole point of that key, so it
    /// must not be caught by the check above.
    #[cfg(unix)]
    #[test]
    fn the_terminal_key_still_takes_a_terminal() {
        assert_eq!(
            terminal_here(&tools(None, None, Some("alacritty")), &folder())
                .unwrap()
                .tool(),
            Some("alacritty")
        );
    }

    #[test]
    fn a_gui_editor_goes_to_the_application_launcher() {
        assert_eq!(
            edit(
                &tools(None, Some("zed"), None),
                &Target::Folder("/tmp/look".into())
            ),
            Ok(Launch::Application {
                tool: "zed".into(),
                path: "/tmp/look".into()
            })
        );
    }

    /// (terminal, command prefix, must contain, must not contain).
    ///
    /// The prefixes are the contract with the catalog, so a changed flag fails
    /// here rather than at a keypress. Ghostty is absent: its expected output
    /// differs by platform, so it has its own test below.
    #[cfg(unix)]
    #[test]
    fn each_terminal_is_handed_the_command_its_own_way() {
        let cases: &[(&str, &str, &[&str], &[&str])] = &[
            ("alacritty", "'alacritty' -e \"$SHELL\" -lc ", &[], &[]),
            ("kitty", "'kitty' \"$SHELL\" -lc ", &[], &[" -e "]),
            ("wezterm", "'wezterm' start -- \"$SHELL\" -lc ", &[], &[]),
            (
                "gnome-terminal",
                "'gnome-terminal' -- \"$SHELL\" -lc ",
                &[],
                &[],
            ),
            ("ptyxis", "'ptyxis' -- \"$SHELL\" -lc ", &[], &[]),
            (
                "Terminal.app",
                "osascript -e ",
                &["do script", "to activate"],
                &[],
            ),
            (
                "iterm",
                "osascript -e ",
                &[
                    "create window with default profile command",
                    r#"application "iTerm""#,
                ],
                &[],
            ),
        ];

        for (terminal, prefix, present, absent) in cases {
            let command =
                shell_command(terminal_here(&tools(None, None, Some(terminal)), &folder()));
            assert!(command.starts_with(prefix), "{terminal}: got {command}");
            for needle in *present {
                assert!(
                    command.contains(needle),
                    "{terminal}: missing {needle:?} in {command}"
                );
            }
            for needle in *absent {
                assert!(
                    !command.contains(needle),
                    "{terminal}: unexpected {needle:?} in {command}"
                );
            }
        }
    }

    /// An empty `cd ''` fails inside the new window, so every target has to
    /// resolve to a directory something can change into.
    #[test]
    fn every_target_resolves_to_a_usable_directory() {
        let cases = [
            (Target::Folder("/tmp/look".into()), "/tmp/look"),
            (Target::File("/tmp/look/a.txt".into()), "/tmp/look"),
            (Target::File("a.txt".into()), CURRENT_DIR),
            (Target::File("/".into()), CURRENT_DIR),
        ];

        for (target, expected) in cases {
            assert_eq!(target.dir(), expected, "{target:?}");
        }
    }

    /// (label, file_manager, target, expected). Only the platform's own manager
    /// can select a file inside its folder, so a declared one is opened at the
    /// containing directory instead.
    #[test]
    fn reveal_uses_the_declared_manager_or_the_platform() {
        let platform = |path: &str| Launch::SystemDefault { path: path.into() };
        let app = |path: &str| Launch::Application {
            tool: "nautilus".into(),
            path: path.into(),
        };
        let cases: &[(&str, Option<&str>, Target, Launch)] = &[
            ("nothing declared", None, file(), platform("/tmp/a.txt")),
            (
                "a blank declaration",
                Some("  "),
                file(),
                platform("/tmp/a.txt"),
            ),
            (
                "a file opens its folder",
                Some("nautilus"),
                Target::File("/tmp/look/a.txt".into()),
                app("/tmp/look"),
            ),
            (
                "a folder is its own",
                Some("nautilus"),
                Target::Folder("/tmp/look".into()),
                app("/tmp/look"),
            ),
        ];

        for (label, manager, target, expected) in cases {
            let declared = Tools {
                file_manager: manager.map(str::to_string),
                ..Tools::default()
            };
            assert_eq!(&reveal(&declared, target), expected, "{label}");
        }
    }

    /// (reason, key that would fix it, what the message must name).
    #[test]
    fn a_reason_names_what_would_fix_it() {
        let cases: &[(Unavailable, Option<&str>, &str)] = &[
            (
                Unavailable::NotDeclared {
                    key: key::TEXT_EDITOR,
                },
                Some(key::TEXT_EDITOR),
                key::TEXT_EDITOR,
            ),
            (
                Unavailable::TerminalRequired {
                    tool: "nvim".into(),
                    key: key::TERMINAL,
                },
                Some(key::TERMINAL),
                "nvim",
            ),
            (
                Unavailable::NotAnEditor {
                    tool: "alacritty".into(),
                    key: key::TEXT_EDITOR,
                },
                Some(key::TEXT_EDITOR),
                "alacritty",
            ),
            (
                Unavailable::CannotRunCommand {
                    tool: "warp".into(),
                },
                None,
                "warp",
            ),
            (Unavailable::UnsupportedPlatform, None, "POSIX"),
        ];

        for (reason, expected_key, mentions) in cases {
            assert_eq!(reason.key(), *expected_key, "{reason:?}");
            let message = reason.message();
            assert!(
                message.contains(mentions),
                "{reason:?} should name {mentions:?}: {message}"
            );
        }
    }

    /// The exact composition, kept as a golden string: the nesting of a quoted
    /// command inside a quoted command is what breaks first when this changes.
    #[cfg(unix)]
    #[test]
    fn a_hosted_editor_composes_exactly() {
        let declared = tools(Some("nvim"), None, Some("alacritty"));
        assert_eq!(
            shell_command(edit(&declared, &file())),
            "'alacritty' -e \"$SHELL\" -lc ''\\''nvim'\\'' '\\''/tmp/a.txt'\\'''"
        );
    }

    /// A tool is a name, so one living under a path with a space stays one word.
    #[cfg(unix)]
    #[test]
    fn an_editor_path_with_a_space_stays_one_word() {
        let declared = tools(Some("/opt/my tools/nvim"), None, Some("alacritty"));
        let command = shell_command(edit(&declared, &file()));

        assert!(command.contains("/opt/my tools/nvim"), "got: {command}");
        assert!(!command.contains("tools/nvim '"), "got: {command}");
    }

    #[cfg(unix)]
    #[test]
    fn a_directory_with_spaces_stays_one_argument() {
        let declared = tools(None, None, Some("alacritty"));
        let target = Target::Folder("/tmp/my project".into());
        let command = shell_command(terminal_here(&declared, &target));

        assert!(
            command.contains("cd '\\''/tmp/my project'\\''"),
            "got: {command}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_terminal_on_a_file_row_opens_its_parent() {
        let declared = tools(None, None, Some("alacritty"));
        let target = Target::File("/tmp/look/a.txt".into());
        let command = shell_command(terminal_here(&declared, &target));

        assert!(command.contains("/tmp/look"), "got: {command}");
        assert!(!command.contains("a.txt"), "got: {command}");
    }

    #[cfg(unix)]
    #[test]
    fn a_terminal_that_cannot_run_commands_says_so() {
        assert_eq!(
            terminal_here(&tools(None, None, Some("warp")), &folder()),
            Err(Unavailable::CannotRunCommand {
                tool: "warp".into()
            })
        );
    }

    #[test]
    fn action_ids_round_trip_and_reject_the_unknown() {
        for action in Action::ALL {
            assert_eq!(Action::from_id(action.id()), Some(*action));
        }
        assert_eq!(Action::from_id("frobnicate"), None);
    }

    #[test]
    fn resolve_dispatches_to_the_same_result_as_the_direct_call() {
        let declared = tools(None, Some("zed"), Some("alacritty"));
        let target = Target::Folder("/tmp/look".into());

        assert_eq!(
            Action::Edit.resolve(&declared, &target),
            edit(&declared, &target)
        );
        assert_eq!(
            Action::TerminalHere.resolve(&declared, &target),
            terminal_here(&declared, &target)
        );
        assert_eq!(
            Action::Reveal.resolve(&declared, &target),
            Ok(reveal(&declared, &target))
        );
    }

    /// Ghostty is single-instance on macOS: invoking its CLI while the app is
    /// already up exits without ever making a window, so the key looks dead.
    /// Only the terminals that need it get this, or every other terminal would
    /// spawn a redundant second instance.
    #[cfg(target_os = "macos")]
    #[test]
    fn only_a_single_instance_mac_app_goes_through_open() {
        let ghostty = shell_command(terminal_here(
            &tools(None, None, Some("ghostty")),
            &folder(),
        ));
        assert!(
            ghostty.starts_with("open -na 'Ghostty' --args -e \"$SHELL\" -lc "),
            "got: {ghostty}"
        );

        for terminal in ["alacritty", "kitty", "wezterm"] {
            let command =
                shell_command(terminal_here(&tools(None, None, Some(terminal)), &folder()));
            assert!(!command.starts_with("open"), "{terminal} got: {command}");
        }
    }
}
