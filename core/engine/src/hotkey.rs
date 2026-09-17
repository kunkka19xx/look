//! The global shortcut that shows and hides the launcher, read from
//! `launcher_hotkey` in `~/.look/config`.
//!
//! The grammar lives here so every shell accepts the same spellings and reports
//! the same mistakes. Shells only translate the parsed [`Hotkey`] into their
//! native registration call.

use serde::Serialize;

pub const LAUNCHER_HOTKEY_CONFIG_KEY: &str = "launcher_hotkey";

#[cfg(target_os = "macos")]
pub const DEFAULT_LAUNCHER_HOTKEY: &str = "cmd+space";
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_LAUNCHER_HOTKEY: &str = "alt+space";

const TOKEN_SEPARATOR: char = '+';
const DISPLAY_SEPARATOR: &str = "+";
const FUNCTION_KEY_MIN: u8 = 1;
const FUNCTION_KEY_MAX: u8 = 20;
const FUNCTION_KEY_PREFIX: &str = "f";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Modifier {
    /// Cmd on macOS, the Windows key elsewhere.
    Command,
    Control,
    /// Option on macOS, Alt elsewhere.
    Option,
    Shift,
}

impl Modifier {
    /// Canonical order, so `shift+cmd+k` and `cmd+shift+k` are the same hotkey.
    const ORDER: [Modifier; 4] = [
        Modifier::Control,
        Modifier::Option,
        Modifier::Shift,
        Modifier::Command,
    ];

    fn parse(token: &str) -> Option<Self> {
        match token {
            "cmd" | "command" | "super" | "win" | "windows" | "meta" => Some(Self::Command),
            "ctrl" | "control" => Some(Self::Control),
            "alt" | "opt" | "option" => Some(Self::Option),
            "shift" => Some(Self::Shift),
            _ => None,
        }
    }

    fn display(self) -> &'static str {
        match self {
            #[cfg(target_os = "macos")]
            Self::Command => "Cmd",
            #[cfg(not(target_os = "macos"))]
            Self::Command => "Win",
            Self::Control => "Ctrl",
            #[cfg(target_os = "macos")]
            Self::Option => "Option",
            #[cfg(not(target_os = "macos"))]
            Self::Option => "Alt",
            Self::Shift => "Shift",
        }
    }

    fn spec(self) -> &'static str {
        match self {
            #[cfg(target_os = "macos")]
            Self::Command => "cmd",
            #[cfg(not(target_os = "macos"))]
            Self::Command => "win",
            Self::Control => "ctrl",
            #[cfg(target_os = "macos")]
            Self::Option => "option",
            #[cfg(not(target_os = "macos"))]
            Self::Option => "alt",
            Self::Shift => "shift",
        }
    }

    /// The token `global-hotkey` (Tauri's shortcut parser) accepts.
    fn accelerator(self) -> &'static str {
        match self {
            Self::Command => "Super",
            Self::Control => "Control",
            Self::Option => "Alt",
            Self::Shift => "Shift",
        }
    }
}

/// Named keys use W3C `KeyboardEvent.code` spellings, which is also what
/// `global-hotkey` parses.
const NAMED_KEYS: &[(&[&str], &str, &str)] = &[
    (&["space"], "Space", "Space"),
    (&["enter", "return"], "Enter", "Enter"),
    (&["tab"], "Tab", "Tab"),
    (&["esc", "escape"], "Escape", "Esc"),
    (&["backspace"], "Backspace", "Backspace"),
    (&["delete", "del"], "Delete", "Delete"),
    (&["up", "arrowup"], "ArrowUp", "Up"),
    (&["down", "arrowdown"], "ArrowDown", "Down"),
    (&["left", "arrowleft"], "ArrowLeft", "Left"),
    (&["right", "arrowright"], "ArrowRight", "Right"),
    (&["home"], "Home", "Home"),
    (&["end"], "End", "End"),
    (&["pageup"], "PageUp", "PageUp"),
    (&["pagedown"], "PageDown", "PageDown"),
];

/// Printable keys that are not letters or digits: `(names, code, character)`.
/// The character is what the key types, which a shell needs to find it on a
/// non-US layout.
const SYMBOL_KEYS: &[(&[&str], &str, char)] = &[
    (&["`", "backquote", "grave"], "Backquote", '`'),
    (&["-", "minus"], "Minus", '-'),
    (&["=", "equal"], "Equal", '='),
    (&["[", "bracketleft"], "BracketLeft", '['),
    (&["]", "bracketright"], "BracketRight", ']'),
    (&["\\", "backslash"], "Backslash", '\\'),
    (&[";", "semicolon"], "Semicolon", ';'),
    (&["'", "quote"], "Quote", '\''),
    (&[",", "comma"], "Comma", ','),
    (&[".", "period"], "Period", '.'),
    (&["/", "slash"], "Slash", '/'),
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Key {
    /// W3C `KeyboardEvent.code` name, for example `Space`, `KeyK`, `F5`.
    pub code: String,
    /// What the key types, for printable keys only.
    pub character: Option<char>,
    display: String,
    #[serde(skip)]
    spec: String,
}

impl Key {
    fn parse(token: &str) -> Option<Self> {
        if let Some((names, code, display)) = NAMED_KEYS
            .iter()
            .find(|(names, _, _)| names.contains(&token))
        {
            return Some(Self::named(code, display, names[0]));
        }
        if let Some((_, code, character)) = SYMBOL_KEYS
            .iter()
            .find(|(names, _, _)| names.contains(&token))
        {
            return Some(Self::printable(code.to_string(), *character));
        }
        if let Some(number) = token.strip_prefix(FUNCTION_KEY_PREFIX)
            && let Ok(number) = number.parse::<u8>()
            && (FUNCTION_KEY_MIN..=FUNCTION_KEY_MAX).contains(&number)
        {
            let name = format!("F{number}");
            return Some(Self::named(&name, &name, &name.to_lowercase()));
        }

        let mut chars = token.chars();
        let (Some(character), None) = (chars.next(), chars.next()) else {
            return None;
        };
        if character.is_ascii_lowercase() {
            let upper = character.to_ascii_uppercase();
            return Some(Self::printable(format!("Key{upper}"), character));
        }
        if character.is_ascii_digit() {
            return Some(Self::printable(format!("Digit{character}"), character));
        }
        None
    }

    fn named(code: &str, display: &str, spec: &str) -> Self {
        Self {
            code: code.to_string(),
            character: None,
            display: display.to_string(),
            spec: spec.to_string(),
        }
    }

    fn printable(code: String, character: char) -> Self {
        Self {
            code,
            character: Some(character),
            display: character.to_ascii_uppercase().to_string(),
            spec: character.to_string(),
        }
    }

    fn is_function_key(&self) -> bool {
        self.code
            .strip_prefix('F')
            .is_some_and(|rest| rest.parse::<u8>().is_ok())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Hotkey {
    pub modifiers: Vec<Modifier>,
    pub key: Key,
}

impl Hotkey {
    pub fn parse(spec: &str) -> Result<Self, String> {
        let spec = spec.trim().to_lowercase();
        let tokens = split_tokens(&spec);
        if tokens.is_empty() {
            return Err("it is empty".to_string());
        }

        let mut modifiers = Vec::new();
        let mut key = None;
        for token in tokens {
            if let Some(modifier) = Modifier::parse(token) {
                if !modifiers.contains(&modifier) {
                    modifiers.push(modifier);
                }
                continue;
            }
            let Some(parsed) = Key::parse(token) else {
                return Err(format!("\"{token}\" is not a key Look knows"));
            };
            if key.replace(parsed).is_some() {
                return Err("it names more than one key".to_string());
            }
        }

        let Some(key) = key else {
            return Err("it has modifiers but no key".to_string());
        };
        modifiers.sort_by_key(|m| Modifier::ORDER.iter().position(|o| o == m));

        // A printable key held with nothing but Shift is ordinary typing, and a
        // global hotkey would swallow it in every app.
        let only_shift = modifiers.iter().all(|m| *m == Modifier::Shift);
        if only_shift && !key.is_function_key() {
            return Err(
                "it needs a modifier other than Shift (only F-keys work alone)".to_string(),
            );
        }

        Ok(Self { modifiers, key })
    }

    pub fn display(&self) -> String {
        self.modifiers
            .iter()
            .map(|m| m.display())
            .chain(std::iter::once(self.key.display.as_str()))
            .collect::<Vec<_>>()
            .join(DISPLAY_SEPARATOR)
    }

    /// The canonical config spelling, the one a shell writes back.
    pub fn spec(&self) -> String {
        self.modifiers
            .iter()
            .map(|m| m.spec())
            .chain(std::iter::once(self.key.spec.as_str()))
            .collect::<Vec<_>>()
            .join(DISPLAY_SEPARATOR)
    }

    /// The spelling Tauri's `Shortcut` parses.
    pub fn accelerator(&self) -> String {
        self.modifiers
            .iter()
            .map(|m| m.accelerator())
            .chain(std::iter::once(self.key.code.as_str()))
            .collect::<Vec<_>>()
            .join(DISPLAY_SEPARATOR)
    }
}

/// `+` is the separator, so it cannot also name a key: the plus key is `equal`.
fn split_tokens(spec: &str) -> Vec<&str> {
    spec.split(TOKEN_SEPARATOR)
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .collect()
}

/// What a shell shows for a hotkey the user is about to save: its display
/// form, or why it would be rejected.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct HotkeyCheck {
    pub spec: String,
    pub display: Option<String>,
    pub error: Option<String>,
}

impl HotkeyCheck {
    pub fn new(spec: &str) -> Self {
        match Hotkey::parse(spec) {
            Ok(hotkey) => Self {
                spec: hotkey.spec(),
                display: Some(hotkey.display()),
                error: None,
            },
            Err(error) => Self {
                spec: spec.trim().to_string(),
                display: None,
                error: Some(error),
            },
        }
    }
}

/// The configured launcher hotkey, falling back to the platform default when
/// the value is missing or invalid. `warning` explains a fallback so a shell
/// can tell the user their value was not used.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LauncherHotkey {
    pub hotkey: Hotkey,
    pub display: String,
    pub accelerator: String,
    /// The config spelling of the platform default, for a "reset" control.
    pub default_spec: String,
    pub warning: Option<String>,
}

impl LauncherHotkey {
    pub fn from_config_value(value: Option<&str>) -> Self {
        let value = value.map(str::trim).filter(|v| !v.is_empty());
        let (hotkey, warning) = match value.map(Hotkey::parse) {
            None => (default_hotkey(), None),
            Some(Ok(hotkey)) => (hotkey, None),
            Some(Err(reason)) => {
                let fallback = default_hotkey();
                let warning = format!(
                    "{LAUNCHER_HOTKEY_CONFIG_KEY}={} ignored: {reason}. Using {}",
                    value.unwrap_or_default(),
                    fallback.display()
                );
                (fallback, Some(warning))
            }
        };
        Self {
            display: hotkey.display(),
            accelerator: hotkey.accelerator(),
            default_spec: default_hotkey().spec(),
            hotkey,
            warning,
        }
    }
}

impl Default for LauncherHotkey {
    fn default() -> Self {
        Self::from_config_value(None)
    }
}

fn default_hotkey() -> Hotkey {
    Hotkey::parse(DEFAULT_LAUNCHER_HOTKEY).expect("default launcher hotkey parses")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(spec: &str) -> Hotkey {
        Hotkey::parse(spec).unwrap_or_else(|e| panic!("{spec}: {e}"))
    }

    #[test]
    fn default_parses() {
        let launcher = LauncherHotkey::default();
        assert!(launcher.warning.is_none());
        assert_eq!(launcher.hotkey.key.code, "Space");
    }

    #[test]
    fn modifiers_are_order_insensitive_and_deduped() {
        assert_eq!(parse("shift+cmd+k"), parse("Command + Shift + K"));
        assert_eq!(parse("cmd+cmd+k").modifiers, vec![Modifier::Command]);
    }

    #[test]
    fn modifier_aliases() {
        assert_eq!(parse("opt+space"), parse("alt+space"));
        assert_eq!(parse("win+space"), parse("cmd+space"));
        assert_eq!(parse("control+space"), parse("ctrl+space"));
    }

    #[test]
    fn keys_carry_code_and_character() {
        let letter = parse("ctrl+k").key;
        assert_eq!(letter.code, "KeyK");
        assert_eq!(letter.character, Some('k'));

        let symbol = parse("cmd+`").key;
        assert_eq!(symbol.code, "Backquote");
        assert_eq!(symbol.character, Some('`'));
        assert_eq!(parse("cmd+backquote"), parse("cmd+`"));

        let digit = parse("alt+1").key;
        assert_eq!(digit.code, "Digit1");

        let named = parse("ctrl+return").key;
        assert_eq!(named.code, "Enter");
        assert_eq!(named.character, None);
    }

    #[test]
    fn function_keys_may_stand_alone() {
        assert_eq!(parse("f13").key.code, "F13");
        assert!(parse("f13").modifiers.is_empty());
        assert!(Hotkey::parse("f21").is_err());
        assert!(Hotkey::parse("f0").is_err());
    }

    #[test]
    fn rejects_hotkeys_that_would_eat_typing() {
        assert!(Hotkey::parse("space").is_err());
        assert!(Hotkey::parse("k").is_err());
        assert!(Hotkey::parse("shift+k").is_err());
        assert!(Hotkey::parse("shift+f5").is_ok());
    }

    #[test]
    fn rejects_malformed_specs() {
        assert!(Hotkey::parse("").is_err());
        assert!(Hotkey::parse("cmd+shift").is_err());
        assert!(Hotkey::parse("cmd+a+b").is_err());
        assert!(Hotkey::parse("cmd+spacebar").is_err());
        assert!(Hotkey::parse("hyper+space").is_err());
    }

    #[test]
    fn accelerator_matches_global_hotkey_spelling() {
        assert_eq!(
            parse("shift+ctrl+alt+cmd+k").accelerator(),
            "Control+Alt+Shift+Super+KeyK"
        );
        assert_eq!(parse("alt+space").accelerator(), "Alt+Space");
    }

    #[test]
    fn display_is_readable() {
        let shown = parse("shift+ctrl+space").display();
        assert_eq!(shown, "Ctrl+Shift+Space");
        assert_eq!(parse("cmd+`").display().rsplit('+').next(), Some("`"));
    }

    #[test]
    fn spec_round_trips_in_canonical_form() {
        for spec in [
            "shift+cmd+k",
            "ctrl+return",
            "option+`",
            "f13",
            "cmd+arrowup",
        ] {
            let canonical = parse(spec).spec();
            assert_eq!(parse(&canonical), parse(spec), "{spec} -> {canonical}");
        }
        assert_eq!(parse("ctrl+return").spec(), "ctrl+enter");
    }

    #[test]
    fn check_reports_display_or_error() {
        let ok = HotkeyCheck::new("Shift+Ctrl+K");
        assert_eq!(ok.display.as_deref(), Some("Ctrl+Shift+K"));
        assert_eq!(ok.spec, "ctrl+shift+k");
        assert!(ok.error.is_none());

        let bad = HotkeyCheck::new("shift+k");
        assert!(bad.display.is_none());
        assert!(bad.error.is_some());
    }

    #[test]
    fn invalid_value_falls_back_with_warning() {
        let launcher = LauncherHotkey::from_config_value(Some("cmd+nope"));
        assert_eq!(launcher.hotkey, default_hotkey());
        let warning = launcher.warning.expect("warning");
        assert!(warning.contains("cmd+nope"));
        assert!(warning.contains("nope"));
    }

    #[test]
    fn blank_value_is_default_without_warning() {
        let launcher = LauncherHotkey::from_config_value(Some("  "));
        assert_eq!(launcher, LauncherHotkey::default());
    }
}
