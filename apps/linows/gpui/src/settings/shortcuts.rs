//! The Shortcuts tab: the catalog flat, with the launcher key's recorder on
//! its row. Only Windows rebinds the key; on Linux the compositor holds it
//! and the row reads as documentation.

use std::sync::LazyLock;

use gpui::prelude::*;
use gpui::{Context, Div, FontWeight, div, px};
use linows_backend::hotkey::{self, LauncherHotkeyState};
use linows_backend::look_engine::hotkey::HotkeyCheck;

use super::Settings;
use crate::commands::KeyOutcome;
use crate::launcher::Launcher;
use crate::shortcuts::{self, Piece};
use crate::theme::Theme;

pub(super) const HOTKEY_KEY: &str = "launcher_hotkey";
const LISTENING: &str = "Press a shortcut, Esc to cancel";
const UNKNOWN_KEY: &str = "That key cannot be used";
const RESET: &str = "Reset";
const FOOTER_DOCS: &str = "This panel is intended as living documentation. We can add command and workflow docs here as features grow.";
const FOOTER_PADDING_TOP: f32 = 10.0;
const NOTICE_PADDING_Y: f32 = 4.0;
const ROW_GAP: f32 = 10.0;
const LIST_PADDING_Y: f32 = 4.0;
/// Modifier presses arrive as keystrokes of their own; they are not a chord.
const MODIFIER_KEYS: [&str; 6] = [
    "control", "shift", "alt", "platform", "function", "capslock",
];

static TIPS: LazyLock<String> = LazyLock::new(|| {
    shortcuts::hint(&[
        Piece::Key("Tips: t\"word", "web EN/VI/JA translation"),
        Piece::Key("/kill", "force quit apps"),
    ])
});

/// What the recorder is doing: nothing, or listening with the last refusal.
#[derive(Default)]
pub(super) struct Recorder {
    pub state: Option<LauncherHotkeyState>,
    /// A chord recorded and not yet saved.
    pub pending: Option<HotkeyCheck>,
    pub listening: bool,
    error: Option<String>,
}

impl Recorder {
    fn shown(&self) -> String {
        self.pending
            .as_ref()
            .and_then(|p| p.display.clone())
            .or_else(|| self.state.as_ref().map(|s| s.display.clone()))
            .unwrap_or_else(|| shortcuts::TOGGLE_LAUNCHER_KEYS.to_string())
    }

    fn configurable(&self) -> bool {
        self.state.as_ref().is_some_and(|s| s.configurable)
    }

    /// Changed from what the file holds.
    pub fn unsaved(&self) -> bool {
        match (&self.pending, &self.state) {
            (Some(p), Some(s)) => p.display.as_deref() != Some(s.display.as_str()),
            (Some(_), None) => true,
            (None, _) => false,
        }
    }

    fn at_default(&self) -> bool {
        match &self.state {
            Some(s) => self.shown() == s.default_display.clone().unwrap_or_default(),
            None => true,
        }
    }

    pub fn stop(&mut self) {
        self.listening = false;
        self.error = None;
    }

    pub fn discard(&mut self) {
        self.stop();
        self.pending = None;
    }
}

impl Settings {
    pub(super) fn tips() -> &'static str {
        &TIPS
    }

    /// A keystroke while the recorder listens: Esc alone stops it, a
    /// modifier alone is waited through, anything else is checked.
    pub(super) fn record_key(
        &mut self,
        ks: &gpui::Keystroke,
        cx: &mut Context<Launcher>,
    ) -> KeyOutcome {
        let key = ks.key.as_str();
        if MODIFIER_KEYS.contains(&key) {
            return KeyOutcome::Consumed;
        }
        let m = ks.modifiers;
        let mods = [
            (m.control, "ctrl"),
            (m.alt, "alt"),
            (m.shift, "shift"),
            (m.platform, "win"),
        ];
        if key == "escape" && !mods.iter().any(|(on, _)| *on) {
            self.recorder.stop();
            cx.notify();
            return KeyOutcome::Consumed;
        }
        let spec = mods
            .iter()
            .filter(|(on, _)| *on)
            .map(|(_, name)| *name)
            .chain(std::iter::once(key))
            .collect::<Vec<_>>()
            .join("+");
        let check = hotkey::hotkey_check(&spec);
        match &check.error {
            Some(error) => self.recorder.error = Some(error.clone()),
            None if check.display.is_none() => self.recorder.error = Some(UNKNOWN_KEY.into()),
            None => {
                self.recorder.pending = Some(check);
                self.recorder.stop();
            }
        }
        cx.notify();
        KeyOutcome::Consumed
    }

    fn toggle_listening(&mut self, cx: &mut Context<Launcher>) {
        if !self.recorder.configurable() {
            return;
        }
        if self.recorder.listening {
            self.recorder.stop();
        } else {
            self.recorder.listening = true;
            self.recorder.error = None;
        }
        cx.notify();
    }

    fn reset_hotkey(&mut self, cx: &mut Context<Launcher>) {
        if let Some(state) = &self.recorder.state {
            self.recorder.pending = Some(hotkey::hotkey_check(&state.default_spec));
        }
        cx.notify();
    }

    pub(super) fn shortcuts(&self, th: &Theme, cx: &mut Context<Launcher>) -> Div {
        let unsaved = usize::from(self.recorder.unsaved());
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
        let footer = |text: &'static str| {
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
            .children(notice)
            .children(shortcuts::groups().iter().map(|group| {
                let rows = group.entries.iter().map(|e| {
                    if e.id == shortcuts::TOGGLE_LAUNCHER {
                        self.hotkey_row(&e.action, th, cx)
                    } else {
                        shortcuts::row(e.keys.clone(), e.action.clone(), th)
                    }
                });
                div()
                    .flex()
                    .flex_col()
                    .child(shortcuts::section_title(group.title, th))
                    .children(rows)
            }))
            .child(footer(FOOTER_DOCS))
            .child(footer(Self::tips()))
    }

    /// The launcher key: a capsule that records on click where the key is
    /// rebindable, Reset while it is off its default, the refusal after it.
    fn hotkey_row(&self, action: &str, th: &Theme, cx: &mut Context<Launcher>) -> Div {
        let r = &self.recorder;
        let configurable = r.configurable();
        let active = r.listening || r.unsaved();
        let label = if r.listening {
            LISTENING.to_string()
        } else {
            r.shown()
        };
        let capsule = div()
            .id("settings-hotkey")
            .child(
                shortcuts::key_cap(label, th)
                    .when(active, |el| el.border(px(1.0)).border_color(th.accent)),
            )
            .when(configurable, |el| {
                el.cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| this.settings.toggle_listening(cx)))
            });
        let reset = (configurable && !r.listening && !r.at_default()).then(|| {
            div()
                .id("settings-hotkey-reset")
                .text_size(px(th.font_size - 2.0))
                .text_color(th.text_secondary)
                .cursor_pointer()
                .hover(|s| s.text_color(th.text))
                .on_click(cx.listener(|this, _, _, cx| this.settings.reset_hotkey(cx)))
                .child(RESET)
        });
        let error = r.listening.then(|| r.error.clone()).flatten().map(|error| {
            div()
                .text_size(px(th.font_size - 2.0))
                .text_color(th.danger)
                .child(error)
        });
        div()
            .py(px(3.0))
            .flex()
            .items_baseline()
            .gap(px(ROW_GAP))
            .child(capsule)
            .children(reset)
            .child(
                div()
                    .min_w_0()
                    .text_size(px(th.font_size - 1.0))
                    .text_color(th.text_secondary)
                    .child(action.to_string()),
            )
            .children(error)
    }
}
