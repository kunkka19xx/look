//! `launcher_hotkey`: the shortcut that shows and hides the launcher. The
//! grammar lives here so every shell accepts the same spellings.

use serde::Serialize;

pub const LAUNCHER_HOTKEY_CONFIG_KEY: &str = "launcher_hotkey";

#[cfg(target_os = "macos")]
pub const DEFAULT_LAUNCHER_HOTKEY: &str = "cmd+space";
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_LAUNCHER_HOTKEY: &str = "alt+space";

/// Registers nothing, for a desktop that binds `lookapp --toggle` itself.
pub const DISABLED_SPEC: &str = "none";
pub const DISABLED_DISPLAY: &str = "lookapp --toggle";

/// Linux binds through the compositor, so it honours `none` only.
const ONLY_DISABLE_IS_CONFIGURABLE: bool = cfg!(target_os = "linux");
const IS_MAC: bool = cfg!(target_os = "macos");
const SEPARATOR: &str = "+";
const ARROW_PREFIX: &str = "Arrow";
const MAX_FUNCTION_KEY: u8 = 20;

/// Declaration order is the canonical order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Modifier {
    Control,
    Option,
    Shift,
    Command,
}

impl Modifier {
    fn parse(token: &str) -> Option<Self> {
        match token {
            "cmd" | "command" | "super" | "win" | "meta" => Some(Self::Command),
            "ctrl" | "control" => Some(Self::Control),
            "alt" | "opt" | "option" => Some(Self::Option),
            "shift" => Some(Self::Shift),
            _ => None,
        }
    }

    /// `(config spelling, display, global-hotkey accelerator)`.
    fn names(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Control => ("ctrl", "Ctrl", "Control"),
            Self::Option if IS_MAC => ("option", "Option", "Alt"),
            Self::Option => ("alt", "Alt", "Alt"),
            Self::Shift => ("shift", "Shift", "Shift"),
            Self::Command if IS_MAC => ("cmd", "Cmd", "Super"),
            Self::Command => ("win", "Win", "Super"),
        }
    }
}

/// W3C `KeyboardEvent.code` names, spelled in config as the code in lowercase
/// or, for printable keys, as the character.
const NAMED_KEYS: &[(&str, Option<char>)] = &[
    ("Space", None),
    ("Enter", None),
    ("Tab", None),
    ("Escape", None),
    ("ArrowUp", None),
    ("ArrowDown", None),
    ("ArrowLeft", None),
    ("ArrowRight", None),
    ("Backquote", Some('`')),
    ("Minus", Some('-')),
    ("Equal", Some('=')),
    ("BracketLeft", Some('[')),
    ("BracketRight", Some(']')),
    ("Backslash", Some('\\')),
    ("Semicolon", Some(';')),
    ("Quote", Some('\'')),
    ("Comma", Some(',')),
    ("Period", Some('.')),
    ("Slash", Some('/')),
];
const KEY_ALIASES: &[(&str, &str)] = &[
    ("esc", "escape"),
    ("return", "enter"),
    ("up", "arrowup"),
    ("down", "arrowdown"),
    ("left", "arrowleft"),
    ("right", "arrowright"),
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Key {
    pub code: String,
    /// What the key types, for printable keys: shells find it by this on
    /// non-US layouts.
    pub character: Option<char>,
}

impl Key {
    fn parse(token: &str) -> Option<Self> {
        let token = KEY_ALIASES
            .iter()
            .find(|(alias, _)| *alias == token)
            .map_or(token, |(_, name)| name);
        let single = single_char(token);

        if let Some((code, character)) = NAMED_KEYS.iter().find(|(code, character)| {
            code.eq_ignore_ascii_case(token) || (character.is_some() && *character == single)
        }) {
            return Some(Self::new(code, *character));
        }
        if let Some(number) = token.strip_prefix('f').and_then(|n| n.parse::<u8>().ok())
            && (1..=MAX_FUNCTION_KEY).contains(&number)
        {
            return Some(Self::new(&format!("F{number}"), None));
        }
        match single? {
            c if c.is_ascii_lowercase() => Some(Self::new(
                &format!("Key{}", c.to_ascii_uppercase()),
                Some(c),
            )),
            c if c.is_ascii_digit() => Some(Self::new(&format!("Digit{c}"), Some(c))),
            _ => None,
        }
    }

    fn new(code: &str, character: Option<char>) -> Self {
        Self {
            code: code.to_string(),
            character,
        }
    }

    fn spec(&self) -> String {
        self.character
            .map_or_else(|| self.code.to_lowercase(), String::from)
    }

    /// Arrows read as `Up`, not `ArrowUp`: the code is the config spelling, the
    /// display is what a key capsule shows.
    fn display(&self) -> String {
        self.character.map_or_else(
            || {
                self.code
                    .strip_prefix(ARROW_PREFIX)
                    .unwrap_or(&self.code)
                    .to_string()
            },
            |c| c.to_ascii_uppercase().to_string(),
        )
    }
}

fn single_char(token: &str) -> Option<char> {
    let mut chars = token.chars();
    chars.next().filter(|_| chars.next().is_none())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Hotkey {
    pub modifiers: Vec<Modifier>,
    pub key: Key,
}

/// Where a chord fires, which is what decides whether Shift alone is a
/// modifier. Global chords are typed at the system; local ones only reach Look
/// while it is focused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Global,
    Local,
}

impl Hotkey {
    /// The global launcher hotkey.
    pub fn parse(spec: &str) -> Result<Self, String> {
        Self::parse_scoped(spec, Scope::Global)
    }

    /// An in-app shortcut. Shift+Enter and Shift+Esc are real chords here - the
    /// keys they hold type nothing - but a chord still needs one modifier, so
    /// ordinary typing stays ordinary typing.
    pub fn parse_local(spec: &str) -> Result<Self, String> {
        Self::parse_scoped(spec, Scope::Local)
    }

    fn parse_scoped(spec: &str, scope: Scope) -> Result<Self, String> {
        let spec = spec.to_lowercase();
        let mut modifiers = Vec::new();
        let mut key = None;
        for token in spec
            .split(SEPARATOR)
            .map(str::trim)
            .filter(|t| !t.is_empty())
        {
            if let Some(modifier) = Modifier::parse(token) {
                modifiers.push(modifier);
            } else if key.is_some() {
                return Err("it names more than one key".to_string());
            } else {
                key =
                    Some(Key::parse(token).ok_or(format!("\"{token}\" is not a key Look knows"))?);
            }
        }
        let key = key.ok_or("it names no key")?;
        modifiers.sort();
        modifiers.dedup();

        // Held with nothing but Shift, a printable key is typing every app needs.
        // A local chord may hold Shift with a key that types nothing.
        let is_function_key = key.character.is_none() && key.code.starts_with('F');
        let shift_is_enough =
            scope == Scope::Local && !modifiers.is_empty() && key.character.is_none();
        if modifiers.iter().all(|m| *m == Modifier::Shift) && !is_function_key && !shift_is_enough {
            return Err(
                "it needs a modifier other than Shift (only F-keys work alone)".to_string(),
            );
        }
        Ok(Self { modifiers, key })
    }

    pub fn spec(&self) -> String {
        self.join(|m| m.names().0, self.key.spec())
    }

    pub fn display(&self) -> String {
        self.join(|m| m.names().1, self.key.display())
    }

    /// The spelling Tauri's `Shortcut` parses.
    pub fn accelerator(&self) -> String {
        self.join(|m| m.names().2, self.key.code.clone())
    }

    fn join(&self, name: impl Fn(Modifier) -> &'static str, key: String) -> String {
        let mut parts: Vec<String> = self
            .modifiers
            .iter()
            .map(|m| name(*m).to_string())
            .collect();
        parts.push(key);
        parts.join(SEPARATOR)
    }
}

/// A spec checked for a settings screen: its canonical spelling and display,
/// or why it is rejected. `hotkey` carries the parsed chord, which is what a
/// shell needs to match the key itself.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct HotkeyCheck {
    pub spec: String,
    pub display: Option<String>,
    pub error: Option<String>,
    pub hotkey: Option<Hotkey>,
}

impl HotkeyCheck {
    pub fn new(spec: &str) -> Self {
        if is_disabled(spec) {
            return Self {
                spec: DISABLED_SPEC.to_string(),
                display: Some(DISABLED_DISPLAY.to_string()),
                error: None,
                hotkey: None,
            };
        }
        Self::checked(spec, Hotkey::parse)
    }

    /// An in-app shortcut, which accepts the Shift+Enter family `new` rejects.
    pub fn local(spec: &str) -> Self {
        Self::checked(spec, Hotkey::parse_local)
    }

    fn checked(spec: &str, parse: impl Fn(&str) -> Result<Hotkey, String>) -> Self {
        match parse(spec) {
            Ok(hotkey) => Self {
                spec: hotkey.spec(),
                display: Some(hotkey.display()),
                error: None,
                hotkey: Some(hotkey),
            },
            Err(error) => Self {
                spec: spec.trim().to_string(),
                display: None,
                error: Some(error),
                hotkey: None,
            },
        }
    }
}

/// The configured launcher hotkey. A missing or invalid value falls back to
/// the default, with `warning` saying why. `enabled` false means register
/// nothing; `hotkey` then holds the unused default.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LauncherHotkey {
    pub hotkey: Hotkey,
    pub enabled: bool,
    pub display: String,
    pub accelerator: String,
    pub default_spec: String,
    pub warning: Option<String>,
}

impl LauncherHotkey {
    pub fn from_config_value(value: Option<&str>) -> Self {
        let default = default_hotkey();
        let value = value.map(str::trim).filter(|v| !v.is_empty());
        let parsed = match value {
            None => Ok(Some(default.clone())),
            Some(v) if is_disabled(v) => Ok(None),
            Some(_) if ONLY_DISABLE_IS_CONFIGURABLE => {
                Err(format!("this platform only accepts {DISABLED_SPEC}"))
            }
            Some(v) => Hotkey::parse(v).map(Some),
        };
        let warning = parsed.as_ref().err().map(|reason| {
            format!(
                "{LAUNCHER_HOTKEY_CONFIG_KEY}={} ignored: {reason}. Using {}",
                value.unwrap_or_default(),
                default.display()
            )
        });
        let chosen = parsed.unwrap_or_else(|_| Some(default.clone()));
        let hotkey = chosen.clone().unwrap_or_else(|| default.clone());
        Self {
            enabled: chosen.is_some(),
            display: chosen.map_or_else(|| DISABLED_DISPLAY.to_string(), |h| h.display()),
            accelerator: hotkey.accelerator(),
            default_spec: default.spec(),
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

fn is_disabled(spec: &str) -> bool {
    spec.trim().eq_ignore_ascii_case(DISABLED_SPEC)
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
    fn spellings_normalise() {
        assert_eq!(parse("shift+cmd+k"), parse("Command + Shift + K"));
        assert_eq!(parse("opt+`"), parse("alt+backquote"));
        assert_eq!(parse("ctrl+return"), parse("control+enter"));
        assert_eq!(parse("ctrl+k").key, Key::new("KeyK", Some('k')));
        assert_eq!(parse("alt+1").key.code, "Digit1");
        assert!(parse("f13").modifiers.is_empty());
    }

    #[test]
    fn rejects_bad_specs() {
        for spec in [
            "",
            "k",
            "space",
            "shift+k",
            "cmd+shift",
            "cmd+a+b",
            "cmd+spacebar",
            "f21",
        ] {
            assert!(Hotkey::parse(spec).is_err(), "{spec} should be rejected");
        }
    }

    #[test]
    fn local_chords_accept_shift_on_keys_that_type_nothing() {
        for spec in ["shift+enter", "shift+esc", "shift+arrowup", "shift+tab"] {
            assert!(Hotkey::parse_local(spec).is_ok(), "{spec} should be local");
            assert!(
                Hotkey::parse(spec).is_err(),
                "{spec} is not a global hotkey"
            );
        }
        // One modifier is still the floor, and Shift+printable is still typing.
        for spec in ["enter", "esc", "arrowup", "k", "shift+k", ""] {
            assert!(
                Hotkey::parse_local(spec).is_err(),
                "{spec} should be rejected"
            );
        }
        assert_eq!(
            Hotkey::parse_local("opt+up").unwrap(),
            Hotkey::parse_local("option+arrowup").unwrap()
        );
        assert_eq!(
            HotkeyCheck::local("option+up").display.as_deref(),
            Some("Option+Up")
        );
        assert_eq!(HotkeyCheck::local("option+up").spec, "option+arrowup");
        assert_eq!(
            HotkeyCheck::local("option+up").hotkey,
            Some(Hotkey::parse_local("option+up").unwrap())
        );
        assert_eq!(HotkeyCheck::local("shift+k").display, None);
    }

    #[test]
    fn renders_every_spelling() {
        let hotkey = parse("shift+ctrl+alt+cmd+k");
        assert_eq!(hotkey.accelerator(), "Control+Alt+Shift+Super+KeyK");
        assert_eq!(parse(&hotkey.spec()), hotkey);
        assert_eq!(parse("shift+ctrl+space").display(), "Ctrl+Shift+Space");
        assert_eq!(HotkeyCheck::new("shift+k").display, None);
    }

    #[test]
    fn none_disables_registration() {
        let launcher = LauncherHotkey::from_config_value(Some("None"));
        assert!(!launcher.enabled);
        assert_eq!(launcher.display, DISABLED_DISPLAY);
        assert!(launcher.warning.is_none());
    }

    #[test]
    fn config_value_falls_back_with_a_warning() {
        assert_eq!(
            LauncherHotkey::from_config_value(Some(" ")),
            LauncherHotkey::default()
        );
        let invalid = LauncherHotkey::from_config_value(Some("cmd+nope"));
        assert_eq!(invalid.hotkey, default_hotkey());
        assert!(invalid.warning.is_some());
        let valid = LauncherHotkey::from_config_value(Some("ctrl+k"));
        assert_eq!(valid.warning.is_some(), ONLY_DISABLE_IS_CONFIGURABLE);
    }
}
