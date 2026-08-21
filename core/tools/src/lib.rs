//! Preferred tools: how to drive the tools a user named, and how an action plus
//! an object become one thing the platform can launch.
//!
//! The user declares nouns in `~/.look/config` (`terminal = ghostty`); this
//! crate owns the verbs. Nothing here reads a command string out of config, by
//! design: that is what a source block is for. See `specs/preferred-tools.md`.

mod catalog;
mod compose;
mod quote;

pub use catalog::{
    AppleScript, CommandStyle, DEFAULT_COMMAND_STYLE, Surface, TERMINALS, TTY_TOOLS, Terminal,
    command_style, entry, surface,
};
pub use compose::{Action, Launch, Target, Unavailable, edit, reveal, terminal_here};
pub use quote::{applescript_quote, shell_quote};

/// The tool keys read from `~/.look/config`, as constants so the config parser,
/// the catalog, and the "which key to set" message cannot drift apart.
pub mod key {
    pub const TEXT_EDITOR: &str = "text_editor";
    pub const CODE_EDITOR: &str = "code_editor";
    pub const TERMINAL: &str = "terminal";
    pub const BROWSER: &str = "browser";
    pub const FILE_MANAGER: &str = "file_manager";

    pub const ALL: &[&str] = &[TEXT_EDITOR, CODE_EDITOR, TERMINAL, BROWSER, FILE_MANAGER];
}

/// All-`None` means the app behaves exactly as it does without this feature.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tools {
    pub text_editor: Option<String>,
    pub code_editor: Option<String>,
    pub terminal: Option<String>,
    pub browser: Option<String>,
    pub file_manager: Option<String>,
}

impl Tools {
    pub fn is_empty(&self) -> bool {
        self.text_editor.is_none()
            && self.code_editor.is_none()
            && self.terminal.is_none()
            && self.browser.is_none()
            && self.file_manager.is_none()
    }

    /// Unknown keys are ignored so a config written for a newer version loads.
    pub fn set(&mut self, key_name: &str, value: &str) {
        let value = value.trim();
        let slot = match key_name.trim() {
            key::TEXT_EDITOR => &mut self.text_editor,
            key::CODE_EDITOR => &mut self.code_editor,
            key::TERMINAL => &mut self.terminal,
            key::BROWSER => &mut self.browser,
            key::FILE_MANAGER => &mut self.file_manager,
            _ => return,
        };
        *slot = if value.is_empty() {
            None
        } else {
            Some(value.to_string())
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_empty() {
        assert!(Tools::default().is_empty());
    }

    #[test]
    fn set_records_known_keys_and_ignores_the_rest() {
        let mut tools = Tools::default();
        tools.set(key::TERMINAL, "ghostty");
        tools.set("verb_edit", "nvim {path}");

        assert_eq!(tools.terminal.as_deref(), Some("ghostty"));
        assert!(tools.text_editor.is_none());
        assert!(!tools.is_empty());
    }

    #[test]
    fn an_empty_value_clears_rather_than_declaring_nothing() {
        let mut tools = Tools::default();
        tools.set(key::TERMINAL, "  ");
        assert!(tools.is_empty());
    }

    #[test]
    fn every_key_is_settable() {
        let mut tools = Tools::default();
        for name in key::ALL {
            tools.set(name, "x");
        }
        assert_eq!(
            tools,
            Tools {
                text_editor: Some("x".into()),
                code_editor: Some("x".into()),
                terminal: Some("x".into()),
                browser: Some("x".into()),
                file_manager: Some("x".into()),
            }
        );
    }
}
