//! Ctrl+H: the help screen, the catalog by topic in the command screen's
//! frame. The macOS `LauncherHelpScreenView`: the topic capsules ride in
//! the title row, and "All" keeps the whole catalog in one scroll.

use std::sync::LazyLock;

use gpui::prelude::*;
use gpui::{Context, Div, FocusHandle, FontWeight, ScrollHandle, div, px};

use crate::commands::KeyOutcome;
use crate::launcher::Launcher;
use crate::shortcuts::{self, Piece, Topic};
use crate::theme::Theme;
use crate::update::Update;

const TITLE: &str = "Help";
const CLOSE_HINT: &str = "Ctrl+H to close";
const SUBTITLE: &str = "Quick guide for app list, clipboard search, and command flow.";
const ALL_LABEL: &str = "All";

const PADDING_X: f32 = 14.0;
const PADDING_Y: f32 = 10.0;
const TITLE_SIZE_STEP: f32 = 3.0;
const TITLE_ROW_GAP: f32 = 12.0;
const CAPSULE_GAP: f32 = 6.0;
const CAPSULE_PADDING_X: f32 = 10.0;
const CAPSULE_PADDING_Y: f32 = 4.0;
const SELECTED_CAPSULE_ALPHA: f32 = 0.22;
const SUBTITLE_PADDING_Y: f32 = 6.0;
const BODY_PADDING_BOTTOM: f32 = 14.0;

static HINT: LazyLock<String> = LazyLock::new(|| {
    shortcuts::hint(&[
        Piece::Id(shortcuts::HELP, "Close help"),
        Piece::Key("Tab", "Next topic"),
        Piece::Id(shortcuts::BACK, "Close"),
    ])
});

pub struct Help {
    pub open: bool,
    /// `None` is every topic.
    topic: Option<Topic>,
    scroll: ScrollHandle,
    /// Holds the keys while the screen is up, so typing cannot reach the
    /// hidden search field.
    focus: FocusHandle,
}

impl Help {
    pub fn new(cx: &mut Context<Launcher>) -> Self {
        Self {
            open: false,
            topic: None,
            scroll: ScrollHandle::new(),
            focus: cx.focus_handle(),
        }
    }

    pub fn toggle(&mut self) {
        self.open = !self.open;
        self.topic = None;
        self.scroll.set_offset(Default::default());
    }

    pub fn hint(&self) -> &'static str {
        &HINT
    }

    pub fn focus_handle(&self) -> FocusHandle {
        self.focus.clone()
    }

    pub fn handle_key(&mut self, ks: &gpui::Keystroke, cx: &mut Context<Launcher>) -> KeyOutcome {
        match ks.key.as_str() {
            "escape" => KeyOutcome::Exit,
            "tab" => {
                self.step_topic(ks.modifiers.shift);
                cx.notify();
                KeyOutcome::Consumed
            }
            _ => KeyOutcome::Consumed,
        }
    }

    /// All, then each topic, round and round.
    fn step_topic(&mut self, back: bool) {
        let order: Vec<Option<Topic>> = std::iter::once(None)
            .chain(Topic::ALL.into_iter().map(Some))
            .collect();
        let at = order.iter().position(|t| *t == self.topic).unwrap_or(0);
        let n = order.len();
        self.topic = order[if back { (at + n - 1) % n } else { (at + 1) % n }];
        self.scroll.set_offset(Default::default());
    }

    fn pick(&mut self, topic: Option<Topic>, cx: &mut Context<Launcher>) {
        self.topic = topic;
        self.scroll.set_offset(Default::default());
        cx.notify();
    }

    pub fn render(&self, th: &Theme, update: &Update, cx: &mut Context<Launcher>) -> Div {
        let groups: Vec<&shortcuts::Group> = match self.topic {
            None => shortcuts::groups().iter().collect(),
            Some(topic) => shortcuts::groups_for(topic).collect(),
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(self.title_row(th, cx))
            .child(div().px(px(PADDING_X)).child(update.view(th, cx)))
            .child(
                div()
                    .id("help-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .px(px(PADDING_X))
                    .pb(px(BODY_PADDING_BOTTOM))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .py(px(SUBTITLE_PADDING_Y))
                            .text_color(th.text_secondary)
                            .child(SUBTITLE),
                    )
                    .children(groups.into_iter().map(|g| shortcuts::group_view(g, th))),
            )
    }

    fn title_row(&self, th: &Theme, cx: &mut Context<Launcher>) -> Div {
        let capsule = |i: usize, label: &'static str, topic: Option<Topic>| {
            let on = topic == self.topic;
            div()
                .id(("help-topic", i))
                .px(px(CAPSULE_PADDING_X))
                .py(px(CAPSULE_PADDING_Y))
                .rounded_full()
                .text_size(px(th.font_size - 1.0))
                .cursor_pointer()
                .map(|el| {
                    if on {
                        let mut wash = th.accent;
                        wash.alpha = SELECTED_CAPSULE_ALPHA;
                        el.bg(wash)
                            .text_color(th.text)
                            .font_weight(FontWeight::SEMIBOLD)
                    } else {
                        el.bg(th.control_fill)
                            .text_color(th.text_muted)
                            .hover(|s| s.text_color(th.text))
                    }
                })
                .on_click(cx.listener(move |this, _, _, cx| this.help.pick(topic, cx)))
                .child(label)
        };
        div()
            .px(px(PADDING_X))
            .py(px(PADDING_Y))
            .flex()
            .items_center()
            .gap(px(TITLE_ROW_GAP))
            .child(
                div()
                    .text_size(px(th.font_size + TITLE_SIZE_STEP))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(TITLE),
            )
            .child(
                div()
                    .flex()
                    .gap(px(CAPSULE_GAP))
                    .child(capsule(0, ALL_LABEL, None))
                    .children(
                        Topic::ALL
                            .into_iter()
                            .enumerate()
                            .map(|(i, t)| capsule(i + 1, t.label(), Some(t))),
                    ),
            )
            .child(div().flex_1())
            .child(
                div()
                    .text_size(px(th.font_size - 1.0))
                    .text_color(th.text_muted)
                    .child(CLOSE_HINT),
            )
    }
}
