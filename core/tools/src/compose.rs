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
            Target::File(path) => Path::new(path)
                .parent()
                .map(|parent| parent.to_string_lossy().into_owned())
                .unwrap_or_default(),
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
    NotDeclared { key: &'static str },
    TerminalRequired { tool: String, key: &'static str },
    CannotRunCommand { tool: String },
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
            Unavailable::NotDeclared { key } | Unavailable::TerminalRequired { key, .. } => {
                Some(key)
            }
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
    match tools
        .file_manager
        .as_deref()
        .filter(|name| !name.trim().is_empty())
    {
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

    let editor = preferred
        .as_deref()
        .or(fallback.as_deref())
        .filter(|name| !name.trim().is_empty())
        .ok_or(Unavailable::NotDeclared { key: missing })?;

    match surface(editor) {
        Surface::Gui => Ok(Launch::Application {
            tool: editor.to_string(),
            path: target.path().to_string(),
        }),
        Surface::Tty => {
            let inner = format!("{} {}", editor, shell_quote(target.path()));
            in_terminal(tools, &inner, editor)
        }
    }
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

    let terminal = tools
        .terminal
        .as_deref()
        .filter(|name| !name.trim().is_empty())
        .ok_or_else(|| {
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

    #[test]
    fn nothing_declared_names_the_key_to_set() {
        let empty = Tools::default();
        assert_eq!(
            edit(&empty, &Target::File("/tmp/a.txt".into())),
            Err(Unavailable::NotDeclared {
                key: key::TEXT_EDITOR
            })
        );
        assert_eq!(
            edit(&empty, &Target::Folder("/tmp".into())),
            Err(Unavailable::NotDeclared {
                key: key::CODE_EDITOR
            })
        );
        assert_eq!(
            terminal_here(&empty, &Target::Folder("/tmp".into())),
            Err(Unavailable::NotDeclared { key: key::TERMINAL })
        );
    }

    #[test]
    fn a_gui_editor_goes_to_the_application_launcher() {
        let declared = tools(None, Some("zed"), None);
        assert_eq!(
            edit(&declared, &Target::Folder("/tmp/look".into())),
            Ok(Launch::Application {
                tool: "zed".into(),
                path: "/tmp/look".into()
            })
        );
    }

    #[test]
    fn one_editor_declared_serves_both_row_kinds() {
        let only_code = tools(None, Some("zed"), None);
        assert_eq!(
            edit(&only_code, &Target::File("/tmp/a.txt".into()))
                .unwrap()
                .tool(),
            Some("zed")
        );

        let only_text = tools(Some("zed"), None, None);
        assert_eq!(
            edit(&only_text, &Target::Folder("/tmp".into()))
                .unwrap()
                .tool(),
            Some("zed")
        );
    }

    #[test]
    fn a_tty_editor_without_a_terminal_says_which_key_is_missing() {
        let declared = tools(Some("nvim"), None, None);
        assert_eq!(
            edit(&declared, &Target::File("/tmp/a.txt".into())),
            Err(Unavailable::TerminalRequired {
                tool: "nvim".into(),
                key: key::TERMINAL
            })
        );
    }

    #[test]
    fn a_tty_editor_is_hosted_by_the_declared_terminal() {
        let declared = tools(Some("nvim"), None, Some("alacritty"));
        let Ok(Launch::Shell { tool, command }) =
            edit(&declared, &Target::File("/tmp/a.txt".into()))
        else {
            panic!("expected a shell launch");
        };
        assert_eq!(tool, "alacritty");
        assert_eq!(
            command,
            "'alacritty' -e \"$SHELL\" -lc 'nvim '\\''/tmp/a.txt'\\'''"
        );
    }

    /// Ghostty is single-instance on macOS: invoking its CLI while the app is
    /// already up exits without ever making a window, so the key looks dead.
    #[cfg(target_os = "macos")]
    #[test]
    fn a_single_instance_mac_app_goes_through_open() {
        let declared = tools(None, None, Some("ghostty"));
        let Ok(Launch::Shell { tool, command }) =
            terminal_here(&declared, &Target::Folder("/tmp".into()))
        else {
            panic!("expected a shell launch");
        };

        assert_eq!(tool, "ghostty");
        assert!(
            command.starts_with("open -na 'Ghostty' --args -e \"$SHELL\" -lc "),
            "got: {command}"
        );
    }

    /// Only the terminals that need it: adding `open -na` everywhere would spawn
    /// a second instance of terminals that are perfectly happy without one.
    #[cfg(target_os = "macos")]
    #[test]
    fn terminals_without_the_override_still_launch_directly() {
        for terminal in ["alacritty", "kitty", "wezterm"] {
            let declared = tools(None, None, Some(terminal));
            let Ok(Launch::Shell { command, .. }) =
                terminal_here(&declared, &Target::Folder("/tmp".into()))
            else {
                panic!("expected a shell launch");
            };
            assert!(!command.starts_with("open"), "{terminal} got: {command}");
        }
    }

    #[test]
    fn kitty_takes_the_command_positionally() {
        let declared = tools(None, None, Some("kitty"));
        let Ok(Launch::Shell { command, .. }) =
            terminal_here(&declared, &Target::Folder("/tmp".into()))
        else {
            panic!("expected a shell launch");
        };
        assert!(
            !command.contains(" -e "),
            "kitty has no -e flag, got: {command}"
        );
        assert!(
            command.starts_with("'kitty' \"$SHELL\" -lc "),
            "got: {command}"
        );
    }

    #[test]
    fn wezterm_goes_through_its_start_subcommand() {
        let declared = tools(None, None, Some("wezterm"));
        let Ok(Launch::Shell { command, .. }) =
            terminal_here(&declared, &Target::Folder("/tmp".into()))
        else {
            panic!("expected a shell launch");
        };
        assert!(
            command.starts_with("'wezterm' start -- \"$SHELL\" -lc "),
            "got: {command}"
        );
    }

    #[test]
    fn a_terminal_on_a_file_row_opens_its_parent() {
        let declared = tools(None, None, Some("ghostty"));
        let Ok(Launch::Shell { command, .. }) =
            terminal_here(&declared, &Target::File("/tmp/look/a.txt".into()))
        else {
            panic!("expected a shell launch");
        };
        assert!(command.contains("/tmp/look"), "got: {command}");
        assert!(!command.contains("a.txt"), "got: {command}");
    }

    #[test]
    fn a_directory_with_spaces_stays_one_argument() {
        let declared = tools(None, None, Some("ghostty"));
        let Ok(Launch::Shell { command, .. }) =
            terminal_here(&declared, &Target::Folder("/tmp/my project".into()))
        else {
            panic!("expected a shell launch");
        };
        assert!(
            command.contains("cd '\\''/tmp/my project'\\''"),
            "got: {command}"
        );
    }

    #[test]
    fn terminal_app_is_driven_through_osascript() {
        let declared = tools(None, None, Some("Terminal.app"));
        let Ok(Launch::Shell { tool, command }) =
            terminal_here(&declared, &Target::Folder("/tmp".into()))
        else {
            panic!("expected a shell launch");
        };
        assert_eq!(tool, "Terminal.app");
        assert!(command.starts_with("osascript -e "), "got: {command}");
        assert!(command.contains("do script"), "got: {command}");
        assert!(command.contains("to activate"), "got: {command}");
    }

    #[test]
    fn iterm_creates_a_window_with_the_command() {
        let declared = tools(None, None, Some("iterm"));
        let Ok(Launch::Shell { command, .. }) =
            terminal_here(&declared, &Target::Folder("/tmp".into()))
        else {
            panic!("expected a shell launch");
        };
        assert!(
            command.contains("create window with default profile command"),
            "got: {command}"
        );
        assert!(command.contains(r#"application "iTerm""#), "got: {command}");
    }

    #[test]
    fn a_terminal_that_cannot_run_commands_says_so() {
        let declared = tools(None, None, Some("warp"));
        assert_eq!(
            terminal_here(&declared, &Target::Folder("/tmp".into())),
            Err(Unavailable::CannotRunCommand {
                tool: "warp".into()
            })
        );
    }

    #[test]
    fn a_blank_declaration_reads_as_undeclared() {
        let blank = tools(Some("   "), None, None);
        assert_eq!(
            edit(&blank, &Target::File("/tmp/a.txt".into())),
            Err(Unavailable::NotDeclared {
                key: key::TEXT_EDITOR
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
        let declared = tools(None, Some("zed"), Some("ghostty"));
        let target = Target::Folder("/tmp/look".into());

        assert_eq!(
            Action::Edit.resolve(&declared, &target),
            edit(&declared, &target)
        );
        assert_eq!(
            Action::TerminalHere.resolve(&declared, &target),
            terminal_here(&declared, &target)
        );
    }

    #[test]
    fn a_reason_names_the_key_that_would_fix_it() {
        let missing = Unavailable::NotDeclared {
            key: key::TEXT_EDITOR,
        };
        assert_eq!(missing.key(), Some(key::TEXT_EDITOR));
        assert!(missing.message().contains(key::TEXT_EDITOR));

        let hosted = Unavailable::TerminalRequired {
            tool: "nvim".into(),
            key: key::TERMINAL,
        };
        assert_eq!(hosted.key(), Some(key::TERMINAL));
        assert!(hosted.message().contains("nvim"));

        let unsupported = Unavailable::CannotRunCommand {
            tool: "warp".into(),
        };
        assert_eq!(unsupported.key(), None);
        assert!(unsupported.message().contains("warp"));
    }

    #[test]
    fn reveal_falls_back_to_the_platform_when_nothing_is_declared() {
        assert_eq!(
            reveal(&Tools::default(), &Target::File("/tmp/a.txt".into())),
            Launch::SystemDefault {
                path: "/tmp/a.txt".into()
            }
        );
    }

    /// Only the platform's own manager can select a file inside its folder, so a
    /// declared one is opened at the containing directory instead.
    #[test]
    fn a_declared_file_manager_opens_the_containing_folder() {
        let declared = Tools {
            file_manager: Some("nautilus".into()),
            ..Tools::default()
        };
        assert_eq!(
            reveal(&declared, &Target::File("/tmp/look/a.txt".into())),
            Launch::Application {
                tool: "nautilus".into(),
                path: "/tmp/look".into()
            }
        );
    }

    #[test]
    fn a_folder_is_its_own_directory() {
        assert_eq!(Target::Folder("/tmp/look".into()).dir(), "/tmp/look");
        assert_eq!(Target::File("/tmp/look/a.txt".into()).dir(), "/tmp/look");
    }
}
