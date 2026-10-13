//! The Shortcuts tab: the catalog by topic, a recorder on every bound row.
//! Only a rebind is written to the file. The macOS `ShortcutsSettingsTab`.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use gpui::prelude::*;
use gpui::{AnyElement, Context, Div, FontWeight, SharedString, div, px, svg};
use linows_backend::config::ConfigUpdate;
use linows_backend::hotkey::{self, LauncherHotkeyState};
use linows_backend::look_engine::hotkey::{HotkeyCheck, LAUNCHER_HOTKEY_CONFIG_KEY};

use super::{Field, FieldKind, Settings};
use crate::bindings::{self, Bindings};
use crate::commands::KeyOutcome;
use crate::controls;
use crate::glyphs;
use crate::launcher::Launcher;
use crate::search::{Changed, input_for, search_field};
use crate::shortcuts::{self, Entry, Group, Hint, Piece, Topic};
use crate::theme::Theme;

const LISTENING: &str = "Press a shortcut, Esc to cancel";
const UNKNOWN_KEY: &str = "That key cannot be used";
const RESET: &str = "Reset";
const RESET_ALL: &str = "Reset all";
const ALL_LABEL: &str = "All";
const CONFIGURABLE_LABEL: &str = "Configurable";
const SEARCH_PLACEHOLDER: &str = "Search shortcuts";
const NO_MATCH: &str = "No shortcut matches";
const NOTHING: &str = "Nothing to show";
const FOOTER_DOCS: &str = "Click a key capsule, press the chord you want, then Save Config. Enter, Esc, Tab, the arrows and the Y/N confirmations stay fixed.";
const FOOTER_PADDING_TOP: f32 = 10.0;
const NOTICE_PADDING_Y: f32 = 4.0;
const ROW_GAP: f32 = 10.0;
const LIST_PADDING_Y: f32 = 4.0;
const HEADER_GAP: f32 = 8.0;
const CAPSULE_GAP: f32 = 6.0;
const SEARCH_W: f32 = 230.0;
const CHEVRON: f32 = 14.0;
/// The Reset link's element id, beside the capsule of the same name.
const RESET_SLOT: usize = 1;
/// Modifier presses arrive as keystrokes of their own.
const MODIFIER_KEYS: [&str; 6] = [
    "control", "shift", "alt", "platform", "function", "capslock",
];

static TIPS: Hint = Hint::new(&[
    Piece::Key("Tips: t\"word", "web EN/VI/JA translation"),
    Piece::Key("/kill", "force quit apps"),
]);

#[derive(Default)]
pub(super) struct View {
    topic: Option<Topic>,
    collapsed: HashSet<&'static str>,
    query: String,
    configurable_only: bool,
}

impl View {
    fn needle(&self) -> String {
        self.query.trim().to_lowercase()
    }
}

pub(super) struct Recorder {
    pub state: Option<LauncherHotkeyState>,
    /// Not yet saved, by catalog id: a spec, or a `|` list for an in-app chord.
    pending: HashMap<&'static str, String>,
    /// The live table with `pending` over it.
    preview: Arc<Bindings>,
    /// The row listening, by catalog id.
    pub listening: Option<&'static str>,
    error: Option<String>,
}

impl Default for Recorder {
    fn default() -> Self {
        Self {
            state: None,
            pending: HashMap::new(),
            preview: bindings::get(),
            listening: None,
            error: None,
        }
    }
}

impl Recorder {
    fn is_hotkey(id: &str) -> bool {
        id == shortcuts::TOGGLE_LAUNCHER
    }

    fn configurable(&self, id: &str) -> bool {
        if Self::is_hotkey(id) {
            self.state.as_ref().is_some_and(|s| s.configurable)
        } else {
            bindings::definition(id).is_some()
        }
    }

    fn shown(&self, id: &str) -> String {
        if Self::is_hotkey(id) {
            return self
                .pending
                .get(id)
                .and_then(|spec| hotkey::hotkey_check(spec).display)
                .or_else(|| self.state.as_ref().map(|s| s.display.clone()))
                .unwrap_or_else(|| shortcuts::TOGGLE_LAUNCHER_KEYS.to_string());
        }
        self.preview
            .display(id)
            .unwrap_or_else(|| shortcuts::keys(id))
    }

    fn unsaved(&self, id: &str) -> bool {
        if Self::is_hotkey(id) {
            return match (self.pending.get(id), &self.state) {
                (Some(spec), Some(s)) => {
                    hotkey::hotkey_check(spec).display.as_deref() != Some(s.display.as_str())
                }
                (Some(_), None) => true,
                (None, _) => false,
            };
        }
        self.pending.contains_key(id) && self.preview.specs(id) != bindings::get().specs(id)
    }

    fn unsaved_count(&self) -> usize {
        self.pending.keys().filter(|id| self.unsaved(id)).count()
    }

    fn at_default(&self, id: &str) -> bool {
        if Self::is_hotkey(id) {
            return match &self.state {
                Some(s) => self.shown(id) == s.default_display.clone().unwrap_or_default(),
                None => true,
            };
        }
        self.preview.is_default(id)
    }

    fn configurable_ids(&self) -> impl Iterator<Item = &'static str> + '_ {
        std::iter::once(shortcuts::TOGGLE_LAUNCHER)
            .chain(bindings::DEFINITIONS.iter().map(|d| d.id))
            .filter(|id| self.configurable(id))
    }

    fn set(&mut self, id: &'static str, value: String) {
        self.pending.insert(id, value);
        self.refresh();
    }

    /// After a change here, or after the file was read again.
    pub fn refresh(&mut self) {
        self.preview = Arc::new(bindings::get().with(&self.pending));
    }

    pub fn stop(&mut self) {
        if self.listening.is_some_and(Self::is_hotkey) {
            crate::hotkey::set_active(true);
        }
        self.listening = None;
        self.error = None;
    }

    pub fn discard(&mut self) {
        self.stop();
        self.pending.clear();
        self.refresh();
    }

    /// The lines to write and the ones to drop: a default leaves no line.
    /// `pending` stays until `saved`, so a failed write can be retried.
    pub fn writes(&self) -> (Vec<ConfigUpdate>, Vec<String>) {
        let mut updates = Vec::new();
        let mut removals = Vec::new();
        for (&id, value) in &self.pending {
            if Self::is_hotkey(id) {
                updates.push(ConfigUpdate {
                    key: LAUNCHER_HOTKEY_CONFIG_KEY.to_string(),
                    value: value.clone(),
                });
            } else if self.preview.is_default(id) {
                removals.push(bindings::config_key(id));
            } else {
                updates.push(ConfigUpdate {
                    key: bindings::config_key(id),
                    value: value.clone(),
                });
            }
        }
        (updates, removals)
    }

    pub fn saved(&mut self) {
        self.pending.clear();
        self.refresh();
    }
}

/// A group after the topic, the search and the filter.
struct Visible {
    group: &'static Group,
    entries: Vec<&'static Entry>,
}

impl Settings {
    pub(super) fn tips() -> SharedString {
        TIPS.text()
    }

    /// Esc alone stops, a modifier alone waits, anything else is checked.
    pub(super) fn record_key(
        &mut self,
        ks: &gpui::Keystroke,
        cx: &mut Context<Launcher>,
    ) -> KeyOutcome {
        let Some(id) = self.recorder.listening else {
            return KeyOutcome::Pass;
        };
        let key = ks.key.as_str();
        if MODIFIER_KEYS.contains(&key) {
            return KeyOutcome::Consumed;
        }
        let m = ks.modifiers;
        if key == "escape" && !(m.control || m.alt || m.shift || m.platform) {
            self.recorder.stop();
            cx.notify();
            return KeyOutcome::Consumed;
        }
        let spec = bindings::spec(ks);
        let check = if Recorder::is_hotkey(id) {
            hotkey::hotkey_check(&spec)
        } else {
            HotkeyCheck::local(&spec)
        };
        match &check.error {
            Some(error) => self.recorder.error = Some(error.clone()),
            None if check.display.is_none() => self.recorder.error = Some(UNKNOWN_KEY.into()),
            None => {
                self.recorder.set(id, check.spec);
                self.recorder.stop();
            }
        }
        cx.notify();
        KeyOutcome::Consumed
    }

    fn toggle_listening(&mut self, id: &'static str, cx: &mut Context<Launcher>) {
        if !self.recorder.configurable(id) {
            return;
        }
        let again = self.recorder.listening == Some(id);
        self.recorder.stop();
        if !again {
            if Recorder::is_hotkey(id) {
                crate::hotkey::set_active(false);
            }
            self.recorder.listening = Some(id);
        }
        cx.notify();
    }

    fn reset(&mut self, id: &'static str, cx: &mut Context<Launcher>) {
        if Recorder::is_hotkey(id) {
            if let Some(state) = &self.recorder.state {
                let spec = state.default_spec.clone();
                self.recorder.set(id, spec);
            }
        } else if let Some(definition) = bindings::definition(id) {
            self.recorder.set(
                definition.id,
                definition.defaults.join(bindings::SPEC_SEPARATOR),
            );
        }
        cx.notify();
    }

    fn reset_all(&mut self, cx: &mut Context<Launcher>) {
        let ids: Vec<&'static str> = self
            .recorder
            .configurable_ids()
            .filter(|id| !self.recorder.at_default(id))
            .collect();
        for id in ids {
            self.reset(id, cx);
        }
    }

    fn pick_topic(&mut self, topic: Option<Topic>, cx: &mut Context<Launcher>) {
        self.shortcuts_view.topic = topic;
        self.scroll.set_offset(Default::default());
        cx.notify();
    }

    fn toggle_group(&mut self, title: &'static str, cx: &mut Context<Launcher>) {
        let folded = &mut self.shortcuts_view.collapsed;
        if !folded.remove(title) {
            folded.insert(title);
        }
        cx.notify();
    }

    /// The text is mirrored into the view, so the filter outlives the box.
    fn open_search_field(&mut self, cx: &mut Context<Launcher>) {
        let input = input_for(cx);
        let query = self.shortcuts_view.query.clone();
        input.update(cx, |input, cx| input.set_text(&query, cx));
        cx.subscribe(&input, |this, input, _: &Changed, cx| {
            this.settings.shortcuts_view.query = input.read(cx).text().to_string();
            this.settings.scroll.set_offset(Default::default());
            cx.notify();
        })
        .detach();
        self.field = Some(Field {
            input,
            highlighted: None,
            kind: FieldKind::ShortcutSearch,
        });
        cx.notify();
    }

    fn close_search_field(&mut self, cx: &mut Context<Launcher>) {
        if self
            .field
            .as_ref()
            .is_some_and(|f| matches!(f.kind, FieldKind::ShortcutSearch))
        {
            self.field = None;
            cx.notify();
        }
    }

    /// A search covers the whole catalog, not only the chosen topic.
    fn visible_groups(&self, needle: &str) -> Vec<Visible> {
        let groups: Vec<&'static Group> = match (needle.is_empty(), self.shortcuts_view.topic) {
            (false, _) | (true, None) => shortcuts::groups().iter().collect(),
            (true, Some(topic)) => shortcuts::groups_for(topic).collect(),
        };
        groups
            .into_iter()
            .filter_map(|group| {
                let entries: Vec<&Entry> = group
                    .entries
                    .iter()
                    .filter(|e| self.shows(e, group, needle))
                    .collect();
                (!entries.is_empty()).then_some(Visible { group, entries })
            })
            .collect()
    }

    /// Matched against what the reader sees, plus the id.
    fn shows(&self, entry: &Entry, group: &Group, needle: &str) -> bool {
        let configurable = self.recorder.configurable(entry.id);
        if self.shortcuts_view.configurable_only && !configurable {
            return false;
        }
        if needle.is_empty() {
            return true;
        }
        let keys = if configurable {
            self.recorder.shown(entry.id)
        } else {
            shortcuts::keys(entry.id)
        };
        [entry.action.as_str(), &keys, group.title, entry.id]
            .iter()
            .any(|hay| hay.to_lowercase().contains(needle))
    }

    pub(super) fn shortcuts(&self, th: &Theme, cx: &mut Context<Launcher>) -> Div {
        let r = &self.recorder;
        let unsaved = r.unsaved_count();
        let notice = (unsaved > 0).then(|| {
            div()
                .py(px(NOTICE_PADDING_Y))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(th.accent)
                .text_size(px(th.font_size - 1.0))
                .child(format!(
                    "{unsaved} shortcut change{} pending. Save Config to apply.",
                    if unsaved == 1 { "" } else { "s" }
                ))
        });
        let conflicts = r.preview.conflicts();
        let conflict_lines = (!conflicts.is_empty()).then(|| {
            div()
                .py(px(NOTICE_PADDING_Y))
                .flex()
                .flex_col()
                .text_size(px(th.font_size - 1.0))
                .text_color(th.danger)
                .children(conflicts)
        });
        let needle = self.shortcuts_view.needle();
        let searching = !needle.is_empty();
        let visible = self.visible_groups(&needle);
        let empty = visible.is_empty().then(|| {
            div()
                .py(px(NOTICE_PADDING_Y))
                .text_size(px(th.font_size - 1.0))
                .text_color(th.text_secondary)
                .child(if searching {
                    format!("{NO_MATCH} \"{}\"", self.shortcuts_view.query.trim())
                } else {
                    NOTHING.to_string()
                })
        });
        let footer = |text: SharedString| {
            div()
                .pt(px(FOOTER_PADDING_TOP))
                .text_size(px(th.font_size - 1.0))
                .text_color(th.text_secondary)
                .child(text)
        };
        div()
            .py(px(LIST_PADDING_Y))
            .flex()
            .flex_col()
            .child(self.shortcuts_header(th, cx))
            .children(notice)
            .children(conflict_lines)
            .children(empty)
            .children(
                visible
                    .into_iter()
                    .enumerate()
                    .map(|(i, v)| self.section(i, v, searching, th, cx)),
            )
            .child(footer(FOOTER_DOCS.into()))
            .child(footer(Self::tips()))
    }

    /// One row in split; compact drops the search to a line of its own.
    fn shortcuts_header(&self, th: &Theme, cx: &mut Context<Launcher>) -> Div {
        let view = &self.shortcuts_view;
        let capsule = |i: usize, label: &'static str, topic: Option<Topic>| {
            shortcuts::filter_capsule(label, topic == view.topic, th)
                .id(("settings-shortcut-topic", i))
                .on_click(cx.listener(move |this, _, _, cx| this.settings.pick_topic(topic, cx)))
        };
        let topics = div()
            .flex()
            .flex_shrink_0()
            .gap(px(CAPSULE_GAP))
            .child(capsule(0, ALL_LABEL, None))
            .children(
                Topic::ALL
                    .into_iter()
                    .enumerate()
                    .map(|(i, t)| capsule(i + 1, t.label(), Some(t))),
            );
        let configurable =
            shortcuts::filter_capsule(CONFIGURABLE_LABEL, view.configurable_only, th)
                .id("settings-shortcut-configurable")
                .on_click(cx.listener(|this, _, _, cx| {
                    this.settings.shortcuts_view.configurable_only ^= true;
                    cx.notify();
                }));
        let reset_all = self
            .recorder
            .configurable_ids()
            .any(|id| !self.recorder.at_default(id))
            .then(|| {
                div()
                    .id("settings-shortcuts-reset-all")
                    .text_size(px(th.font_size - 1.0))
                    .text_color(th.text_secondary)
                    .cursor_pointer()
                    .hover(|s| s.text_color(th.text))
                    .on_click(cx.listener(|this, _, _, cx| this.settings.reset_all(cx)))
                    .child(RESET_ALL)
            });
        let wide = th.split();
        div()
            .flex()
            .flex_col()
            .gap(px(HEADER_GAP))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(HEADER_GAP))
                    .child(topics)
                    .child(configurable)
                    .when(wide, |el| el.child(self.search_box(th, cx)))
                    .child(div().flex_1())
                    .children(reset_all),
            )
            .when(!wide, |el| el.child(self.search_box(th, cx)))
    }

    fn search_box(&self, th: &Theme, cx: &mut Context<Launcher>) -> Div {
        let editing = self
            .field
            .as_ref()
            .filter(|f| matches!(f.kind, FieldKind::ShortcutSearch));
        let query = &self.shortcuts_view.query;
        let content: AnyElement = match editing {
            Some(field) => search_field(field.input.clone(), SEARCH_PLACEHOLDER).into_any_element(),
            None => div()
                .flex_1()
                .overflow_hidden()
                .whitespace_nowrap()
                .when(query.is_empty(), |el| el.text_color(th.text_muted))
                .child(if query.is_empty() {
                    SEARCH_PLACEHOLDER.to_string()
                } else {
                    query.clone()
                })
                .into_any_element(),
        };
        div().flex_shrink_0().child(
            controls::text_box(
                "settings-shortcut-search",
                SEARCH_W,
                content,
                editing.is_some(),
                th,
            )
            .when(editing.is_none(), |el| {
                el.on_click(cx.listener(|this, _, _, cx| this.settings.open_search_field(cx)))
            })
            .when(editing.is_some(), |el| {
                el.on_mouse_down_out(
                    cx.listener(|this, _, _, cx| this.settings.close_search_field(cx)),
                )
            }),
        )
    }

    /// A group under a header that folds it; a search never hides a hit.
    fn section(
        &self,
        index: usize,
        visible: Visible,
        searching: bool,
        th: &Theme,
        cx: &mut Context<Launcher>,
    ) -> Div {
        let title = visible.group.title;
        let open = searching || !self.shortcuts_view.collapsed.contains(title);
        let configurable = visible
            .entries
            .iter()
            .filter(|e| self.recorder.configurable(e.id))
            .count();
        let header = shortcuts::section_row(th)
            .id(("settings-shortcut-group", index))
            .when(!searching, |el| {
                el.cursor_pointer().on_click(
                    cx.listener(move |this, _, _, cx| this.settings.toggle_group(title, cx)),
                )
            })
            .child(
                svg()
                    .path(if open {
                        glyphs::CHEVRON_DOWN
                    } else {
                        glyphs::CHEVRON_RIGHT
                    })
                    .size(px(CHEVRON))
                    .text_color(th.text_secondary),
            )
            .child(shortcuts::section_label(title, th))
            .child(
                div()
                    .text_size(px(th.font_size - 1.0))
                    .text_color(th.text_muted)
                    .child(format!(
                        "{configurable} of {} configurable",
                        visible.entries.len()
                    )),
            );
        let rows = open.then(|| {
            visible.entries.into_iter().map(|e| {
                if self.recorder.configurable(e.id) {
                    self.recorder_row(e, th, cx)
                } else {
                    shortcuts::row_of(
                        shortcuts::key_cap(shortcuts::keys(e.id), th).text_color(th.text_muted),
                        e.action.clone(),
                        th,
                    )
                }
            })
        });
        div()
            .flex()
            .flex_col()
            .child(header)
            .children(rows.into_iter().flatten())
    }

    /// An outlined capsule that records on click, Reset off its default.
    fn recorder_row(&self, entry: &Entry, th: &Theme, cx: &mut Context<Launcher>) -> Div {
        let id = entry.id;
        let r = &self.recorder;
        let listening = r.listening == Some(id);
        let active = listening || r.unsaved(id);
        let label = if listening {
            LISTENING.to_string()
        } else {
            r.shown(id)
        };
        // The border is what says this capsule is an input.
        let capsule = shortcuts::key_cap(label, th)
            .id(SharedString::new_static(id))
            .border(px(shortcuts::KEY_BORDER))
            .border_color(if active { th.accent } else { th.border })
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| this.settings.toggle_listening(id, cx)));
        let reset = (!listening && !r.at_default(id)).then(|| {
            div()
                .id((SharedString::new_static(id), RESET_SLOT))
                .text_size(px(th.font_size - 2.0))
                .text_color(th.text_secondary)
                .cursor_pointer()
                .hover(|s| s.text_color(th.text))
                .on_click(cx.listener(move |this, _, _, cx| this.settings.reset(id, cx)))
                .child(RESET)
        });
        let error = listening.then(|| r.error.clone()).flatten().map(|error| {
            div()
                .text_size(px(th.font_size - 2.0))
                .text_color(th.danger)
                .child(error)
        });
        let keys = div()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(ROW_GAP))
                    .child(capsule)
                    .children(reset),
            )
            .children(error);
        shortcuts::row_of(keys, entry.action.clone(), th)
    }
}
