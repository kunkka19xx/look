//! In-app chords: the shipped table, `shortcut_<group>_<action>` overrides
//! from the config, and the match against a gpui keystroke. Core owns the
//! grammar; the config keys are the macOS ones, so one file serves both.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, RwLock};

use gpui::Keystroke;
use linows_backend::config;
use linows_backend::look_engine::hotkey::{Hotkey, HotkeyCheck, Key, Modifier};

use crate::shortcuts::{
    ACTIONS, CLEAR_PICKS, CLIP_PASTE, CLIP_REMOVE, COMMAND_MODE, COPY, EDIT, HELP, HIDE_APP,
    OPEN_PICKED, PASTE, PICK, PROCESS_COPY_PID, PROCESS_KILL, QUICK_TOGGLE, RELOAD_CONFIG, REVEAL,
    SETTINGS, SWITCH_LAYOUT, TERMINAL, TODO_REDO, TODO_SAVE, TODO_TOGGLE_PAGE, TODO_UNDO, TRASH,
    WEB_SEARCH,
};

pub const CONFIG_KEY_PREFIX: &str = "shortcut_";
/// Several chords for one action; not a comma, `ctrl+shift+,` is a chord.
pub const SPEC_SEPARATOR: &str = " | ";
const SPEC_SPLIT: char = '|';
const DISPLAY_SEPARATOR: &str = " / ";
const CHORD_JOIN: &str = "+";
const ARROW_PREFIX: &str = "Arrow";

/// Where a chord is live, which decides whether two of them collide.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Launcher,
    Todo,
    AppWide,
}

impl Scope {
    fn overlaps(self, other: Scope) -> bool {
        self == Scope::AppWide || other == Scope::AppWide || self == other
    }
}

/// One rebindable chord; `defaults` lists aliases, recording replaces them.
pub struct Definition {
    /// The catalog id.
    pub id: &'static str,
    pub defaults: &'static [&'static str],
    pub scope: Scope,
    /// The other catalog rows this binding drives, like Ctrl+D on a clip.
    pub mirrors: &'static [&'static str],
}

const fn def(id: &'static str, defaults: &'static [&'static str], scope: Scope) -> Definition {
    Definition {
        id,
        defaults,
        scope,
        mirrors: &[],
    }
}

impl Definition {
    const fn mirrors(mut self, mirrors: &'static [&'static str]) -> Self {
        self.mirrors = mirrors;
        self
    }

    pub fn documents(&self, id: &str) -> bool {
        self.id == id || self.mirrors.contains(&id)
    }
}

/// In the order the handlers try them. Enter, Esc, Tab, the arrows and
/// Y/N mean different things per mode, so they stay fixed and are not here.
pub const DEFINITIONS: &[Definition] = &[
    def(COPY, &["ctrl+c"], Scope::Launcher).mirrors(&[PROCESS_COPY_PID]),
    def(PICK, &["ctrl+p"], Scope::Launcher),
    def(CLEAR_PICKS, &["ctrl+shift+p"], Scope::Launcher),
    def(OPEN_PICKED, &["shift+enter"], Scope::Launcher),
    def(TRASH, &["ctrl+d"], Scope::Launcher).mirrors(&[CLIP_REMOVE, PROCESS_KILL]),
    def(ACTIONS, &["ctrl+k", "ctrl+j"], Scope::Launcher),
    def(REVEAL, &["ctrl+f"], Scope::Launcher),
    def(EDIT, &["ctrl+e"], Scope::Launcher),
    def(TERMINAL, &["ctrl+t"], Scope::Launcher),
    def(WEB_SEARCH, &["ctrl+enter"], Scope::Launcher),
    def(COMMAND_MODE, &["ctrl+/", "ctrl+shift+/"], Scope::Launcher),
    def(HIDE_APP, &["ctrl+shift+h"], Scope::Launcher),
    def(QUICK_TOGGLE, &["ctrl+o"], Scope::Launcher),
    def(CLIP_PASTE, &["ctrl+i"], Scope::Launcher).mirrors(&[PASTE]),
    def(HELP, &["ctrl+h"], Scope::AppWide),
    def(SETTINGS, &["ctrl+shift+,"], Scope::AppWide),
    def(RELOAD_CONFIG, &["ctrl+shift+;"], Scope::AppWide),
    def(SWITCH_LAYOUT, &["ctrl+shift+c"], Scope::AppWide),
    def(TODO_TOGGLE_PAGE, &["ctrl+n"], Scope::Todo),
    def(TODO_SAVE, &["ctrl+s"], Scope::Todo),
    def(TODO_UNDO, &["ctrl+z"], Scope::Todo),
    def(TODO_REDO, &["ctrl+shift+z"], Scope::Todo),
];

pub fn definition(id: &str) -> Option<&'static Definition> {
    DEFINITIONS.iter().find(|d| d.documents(id))
}

/// `main.clearPicks` -> `shortcut_main_clear_picks`.
pub fn config_key(id: &str) -> String {
    let mut key = String::with_capacity(CONFIG_KEY_PREFIX.len() + id.len() + 4);
    key.push_str(CONFIG_KEY_PREFIX);
    for ch in id.chars() {
        if ch == '.' {
            key.push('_');
        } else if ch.is_ascii_uppercase() {
            key.push('_');
            key.push(ch.to_ascii_lowercase());
        } else {
            key.push(ch);
        }
    }
    key
}

/// US-layout pairs: both shells report Shift+, as `<` with Shift dropped.
const SHIFTED: &[(char, char)] = &[
    ('`', '~'),
    ('1', '!'),
    ('2', '@'),
    ('3', '#'),
    ('4', '$'),
    ('5', '%'),
    ('6', '^'),
    ('7', '&'),
    ('8', '*'),
    ('9', '('),
    ('0', ')'),
    ('-', '_'),
    ('=', '+'),
    ('[', '{'),
    (']', '}'),
    ('\\', '|'),
    (';', ':'),
    ('\'', '"'),
    (',', '<'),
    ('.', '>'),
    ('/', '?'),
];

fn single(key: &str) -> Option<char> {
    let mut chars = key.chars();
    chars.next().filter(|_| chars.next().is_none())
}

fn shifted(key: &str) -> Option<char> {
    let base = single(key)?;
    SHIFTED.iter().find(|(b, _)| *b == base).map(|(_, s)| *s)
}

fn unshifted(key: &str) -> Option<char> {
    let symbol = single(key)?;
    SHIFTED.iter().find(|(_, s)| *s == symbol).map(|(b, _)| *b)
}

/// A keystroke as a config value, for the recorder; core judges it.
pub fn spec(ks: &Keystroke) -> String {
    let m = ks.modifiers;
    let (key, shift) = match unshifted(&ks.key) {
        Some(base) => (base.to_string(), true),
        None => (ks.key.clone(), m.shift),
    };
    [
        (m.control, "ctrl"),
        (m.alt, "alt"),
        (shift, "shift"),
        (m.platform, "win"),
    ]
    .iter()
    .filter(|(on, _)| *on)
    .map(|(_, name)| *name)
    .chain(std::iter::once(key.as_str()))
    .collect::<Vec<_>>()
    .join(CHORD_JOIN)
}

/// A chord in gpui's spelling.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chord {
    ctrl: bool,
    alt: bool,
    shift: bool,
    win: bool,
    key: String,
}

impl Chord {
    fn from_hotkey(hotkey: &Hotkey) -> Self {
        let has = |m: Modifier| hotkey.modifiers.contains(&m);
        Self {
            ctrl: has(Modifier::Control),
            alt: has(Modifier::Option),
            shift: has(Modifier::Shift),
            win: has(Modifier::Command),
            key: gpui_key(&hotkey.key),
        }
    }

    pub fn matches(&self, ks: &Keystroke) -> bool {
        let m = ks.modifiers;
        if m.control != self.ctrl || m.alt != self.alt || m.platform != self.win {
            return false;
        }
        if m.shift == self.shift && ks.key == self.key {
            return true;
        }
        self.shift && !m.shift && shifted(&self.key).is_some_and(|s| single(&ks.key) == Some(s))
    }
}

/// `KeyK` is `k`, `ArrowUp` is `up`, `F3` is `f3`.
fn gpui_key(key: &Key) -> String {
    match key.character {
        Some(c) => c.to_string(),
        None => key
            .code
            .strip_prefix(ARROW_PREFIX)
            .unwrap_or(&key.code)
            .to_lowercase(),
    }
}

struct Bound {
    chord: Chord,
    spec: String,
    display: String,
}

pub struct Bindings {
    /// Per definition, in table order.
    bound: Vec<Vec<Bound>>,
    /// The `shortcut_*` keys as read, so a preview can lay more over them.
    overrides: HashMap<String, String>,
    /// The launcher key's display, read once rather than per render.
    pub launcher: String,
}

impl Bindings {
    pub fn from_config() -> Self {
        let overrides = config::get_config()
            .entries
            .into_iter()
            .filter(|e| e.key.starts_with(CONFIG_KEY_PREFIX) && !e.value.is_empty())
            .map(|e| (e.key, e.value))
            .collect();
        Self::from_overrides(overrides, linows_backend::hotkey::configured().display)
    }

    fn from_overrides(overrides: HashMap<String, String>, launcher: String) -> Self {
        let bound = DEFINITIONS
            .iter()
            .map(|d| {
                let key = config_key(d.id);
                let value = overrides.get(&key);
                let mut bound: Vec<Bound> = value
                    .map(|v| specs_in(v).filter_map(resolve).collect())
                    .unwrap_or_default();
                // A chord core rejects falls back rather than going dead.
                if bound.is_empty() {
                    if let Some(value) = value {
                        eprintln!("look: {key}={value} is not a chord Look can bind");
                    }
                    bound = d.defaults.iter().filter_map(|s| resolve(s)).collect();
                }
                bound
            })
            .collect();
        Self {
            bound,
            overrides,
            launcher,
        }
    }

    /// This table with `pending` (by catalog id) laid over it.
    pub fn with(&self, pending: &HashMap<&'static str, String>) -> Self {
        let mut overrides = self.overrides.clone();
        for (id, value) in pending {
            overrides.insert(config_key(id), value.clone());
        }
        Self::from_overrides(overrides, self.launcher.clone())
    }

    fn index(id: &str) -> Option<usize> {
        DEFINITIONS.iter().position(|d| d.documents(id))
    }

    /// The first definition in `scope` whose chord the keystroke is.
    pub fn hit(&self, ks: &Keystroke, scope: Scope) -> Option<&'static str> {
        DEFINITIONS
            .iter()
            .zip(&self.bound)
            .filter(|(d, _)| d.scope == scope)
            .find(|(_, bound)| bound.iter().any(|b| b.chord.matches(ks)))
            .map(|(d, _)| d.id)
    }

    /// Every chord the row answers to; None for a row with no binding.
    pub fn display(&self, id: &str) -> Option<String> {
        let i = Self::index(id)?;
        Some(
            self.bound[i]
                .iter()
                .map(|b| b.display.as_str())
                .collect::<Vec<_>>()
                .join(DISPLAY_SEPARATOR),
        )
    }

    pub fn specs(&self, id: &str) -> Vec<&str> {
        Self::index(id).map_or_else(Vec::new, |i| {
            self.bound[i].iter().map(|b| b.spec.as_str()).collect()
        })
    }

    pub fn is_default(&self, id: &str) -> bool {
        let Some(i) = Self::index(id) else {
            return true;
        };
        let defaults: Vec<String> = DEFINITIONS[i]
            .defaults
            .iter()
            .map(|s| HotkeyCheck::local(s).spec)
            .collect();
        self.specs(id) == defaults
    }

    /// Chords claimed twice where both are live. Reported, not refused.
    pub fn conflicts(&self) -> Vec<String> {
        let mut owners: Vec<(&str, &str, Vec<usize>)> = Vec::new();
        for (i, bound) in self.bound.iter().enumerate() {
            for b in bound {
                match owners.iter_mut().find(|(spec, _, _)| *spec == b.spec) {
                    Some((_, _, who)) => who.push(i),
                    None => owners.push((&b.spec, &b.display, vec![i])),
                }
            }
        }
        owners
            .into_iter()
            .filter(|(_, _, who)| {
                who.iter().enumerate().any(|(n, a)| {
                    who[n + 1..]
                        .iter()
                        .any(|b| DEFINITIONS[*a].scope.overlaps(DEFINITIONS[*b].scope))
                })
            })
            .map(|(_, display, who)| {
                let names: Vec<&str> = who.iter().map(|i| DEFINITIONS[*i].id).collect();
                format!("{display} is bound to {}", names.join(" and "))
            })
            .collect()
    }
}

pub fn specs_in(value: &str) -> impl Iterator<Item = &str> {
    value
        .split(SPEC_SPLIT)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn resolve(spec: &str) -> Option<Bound> {
    let check = HotkeyCheck::local(spec);
    let hotkey = check.hotkey?;
    Some(Bound {
        chord: Chord::from_hotkey(&hotkey),
        spec: check.spec,
        display: check.display?,
    })
}

static BINDINGS: LazyLock<RwLock<Arc<Bindings>>> =
    LazyLock::new(|| RwLock::new(Arc::new(Bindings::from_config())));
/// Bumped on every load, so a cached hint knows to respell itself.
static REVISION: AtomicU64 = AtomicU64::new(0);

pub fn load() {
    *BINDINGS.write().unwrap_or_else(|p| p.into_inner()) = Arc::new(Bindings::from_config());
    REVISION.fetch_add(1, Ordering::Relaxed);
}

pub fn get() -> Arc<Bindings> {
    Arc::clone(&BINDINGS.read().unwrap_or_else(|p| p.into_inner()))
}

pub fn revision() -> u64 {
    REVISION.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ks(s: &str) -> Keystroke {
        Keystroke::parse(s).unwrap_or_else(|e| panic!("{s}: {e:?}"))
    }

    fn defaults() -> Bindings {
        Bindings::from_overrides(HashMap::new(), String::new())
    }

    impl Bindings {
        fn matches(&self, id: &str, ks: &Keystroke) -> bool {
            DEFINITIONS
                .iter()
                .find(|d| d.documents(id))
                .is_some_and(|d| self.hit(ks, d.scope) == Some(d.id))
        }
    }

    #[test]
    fn config_keys_follow_the_macos_spelling() {
        assert_eq!(config_key(CLEAR_PICKS), "shortcut_main_clear_picks");
        assert_eq!(
            config_key(QUICK_TOGGLE),
            "shortcut_main_quick_action_toggle"
        );
        assert_eq!(config_key(RELOAD_CONFIG), "shortcut_view_reload_config");
        assert_eq!(config_key(TODO_REDO), "shortcut_todo_redo");
    }

    #[test]
    fn every_default_parses_and_ids_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for d in DEFINITIONS {
            assert!(seen.insert(d.id), "duplicate {}", d.id);
            for spec in d.defaults {
                assert!(HotkeyCheck::local(spec).hotkey.is_some(), "{spec}");
            }
            assert!(
                crate::shortcuts::entry(d.id).is_some(),
                "{} not in catalog",
                d.id
            );
            for m in d.mirrors {
                assert!(crate::shortcuts::entry(m).is_some(), "{m} not in catalog");
            }
        }
        assert!(defaults().conflicts().is_empty());
    }

    #[test]
    fn defaults_match_their_keystrokes() {
        let b = defaults();
        assert!(b.matches(COPY, &ks("ctrl-c")));
        assert!(b.matches(PROCESS_COPY_PID, &ks("ctrl-c")));
        assert!(!b.matches(COPY, &ks("ctrl-shift-c")));
        assert!(b.matches(ACTIONS, &ks("ctrl-j")));
        assert!(b.matches(OPEN_PICKED, &ks("shift-enter")));
        assert!(b.matches(WEB_SEARCH, &ks("ctrl-enter")));
        assert!(b.matches(TODO_REDO, &ks("ctrl-shift-z")));
        assert!(!b.matches(TODO_UNDO, &ks("ctrl-shift-z")));
        assert_eq!(
            b.hit(&ks("ctrl-shift-c"), Scope::AppWide),
            Some(SWITCH_LAYOUT)
        );
        assert_eq!(b.hit(&ks("ctrl-shift-c"), Scope::Launcher), None);
        assert_eq!(b.hit(&ks("ctrl-d"), Scope::Launcher), Some(TRASH));
    }

    #[test]
    fn shifted_symbols_meet_their_chord_either_way() {
        let b = defaults();
        // The compositor's spelling, Shift folded into the symbol.
        assert!(b.matches(SETTINGS, &ks("ctrl-<")));
        assert!(b.matches(RELOAD_CONFIG, &ks("ctrl-:")));
        assert!(b.matches(COMMAND_MODE, &ks("ctrl-?")));
        // The socket's spelling, Shift kept on the base key.
        assert!(b.matches(SETTINGS, &ks("ctrl-shift-,")));
        assert!(!b.matches(SETTINGS, &ks("ctrl-,")));
        assert_eq!(spec(&ks("ctrl-<")), "ctrl+shift+,");
        assert_eq!(spec(&ks("ctrl-shift-k")), "ctrl+shift+k");
        assert_eq!(spec(&ks("alt-up")), "alt+up");
        assert_eq!(spec(&ks("cmd-space")), "win+space");
    }

    #[test]
    fn overrides_rebind_and_bad_ones_keep_the_default() {
        let b = Bindings::from_overrides(
            HashMap::from([
                (config_key(COPY), "ctrl+shift+c | alt+c".to_string()),
                (config_key(TRASH), "shift+k".to_string()),
            ]),
            String::new(),
        );
        assert!(b.matches(COPY, &ks("alt-c")));
        assert!(!b.matches(COPY, &ks("ctrl-c")));
        assert!(!b.is_default(COPY));
        assert_eq!(b.display(COPY).as_deref(), Some("Ctrl+Shift+C / Alt+C"));
        assert!(b.matches(TRASH, &ks("ctrl-d")));
        assert!(b.is_default(TRASH));
        assert_eq!(b.display("nothing.here"), None);
    }

    #[test]
    fn a_preview_lays_pending_over_the_file_and_reports_conflicts() {
        let file = Bindings::from_overrides(
            HashMap::from([(config_key(COPY), "alt+c".to_string())]),
            String::new(),
        );
        let preview = file.with(&HashMap::from([(PICK, "alt+c".to_string())]));
        // The first row in the table wins the chord; the other is shadowed.
        assert_eq!(preview.hit(&ks("alt-c"), Scope::Launcher), Some(COPY));
        assert_eq!(preview.specs(PICK), vec!["alt+c"]);
        assert_eq!(preview.conflicts().len(), 1);
        assert!(preview.conflicts()[0].starts_with("Alt+C is bound to"));
        // /todo's Ctrl+Z is not the launcher's, so no line for it.
        let todo = file.with(&HashMap::from([(COPY, "ctrl+z".to_string())]));
        assert!(todo.conflicts().is_empty());
        // Across scopes an app-wide chord collides with everything.
        let wide = file.with(&HashMap::from([(HELP, "ctrl+z".to_string())]));
        assert_eq!(wide.conflicts().len(), 1);
    }
}
