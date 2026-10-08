//! `/pomo`: the timer card with its ring, dial or bare digits, the controls,
//! the plan as an editable list, and the background music at the foot. The
//! state is the process-wide one in `pomo.rs`; this panel ticks it while
//! it is open and fades to the ring alone after a few idle seconds.

use std::time::{Duration, Instant};

use gpui::{
    Animation, AnimationExt, AnyElement, Bounds, Context, Corners, Div, Entity, FontWeight,
    PathBuilder, Pixels, Rgba, ScrollHandle, SharedString, Stateful, Task, TextAlign, TextRun,
    Transformation, Window, canvas, div, fill, point, prelude::*, px, radians, svg,
};

use super::{Commands, KeyOutcome};
use crate::banner;
use crate::bg;
use crate::glyphs;
use crate::launcher::Launcher;
use crate::launchpad::Tone;
use crate::motion;
use crate::pick;
use crate::pomo::{self, Kind, STYLES, Style};
use crate::search::{SearchInput, search_field};
use crate::theme::{self, Theme};

pub const ID: &str = "pomo";
const TICK: Duration = Duration::from_millis(500);
/// Running and untouched this long, the panel fades to the ring.
const IDLE_FADE: Duration = Duration::from_secs(5);
const CANVAS: f32 = 180.0;
const CANVAS_IDLE: f32 = 240.0;
const RING_INSET: f32 = 12.0;
const RING_W: f32 = 6.0;
const TRACK_GREY: f32 = 0.5;
const RING_TRACK_ALPHA: f32 = 0.15;
const TICK_TRACK_ALPHA: f32 = 0.3;

/// The grey the unfilled ring and the dial's ticks share.
fn track(alpha: f32) -> gpui::Hsla {
    theme::hsla_of(Rgba::new(TRACK_GREY, TRACK_GREY, TRACK_GREY, alpha))
}
const DIAL_INSET: f32 = 10.0;
const DIAL_TICK_MAJOR: f32 = 12.0;
const DIAL_TICK_MINOR: f32 = 6.0;
const DIAL_HUB: f32 = 4.0;
const DIAL_NEEDLE_INSET: f32 = 20.0;
const DIAL_NEEDLE_W: f32 = 2.5;
const RING_TEXT_SCALE: f32 = 0.22;
const MINIMAL_TEXT_SCALE: f32 = 0.32;
const MINIMAL_TEXT_Y: f32 = 0.42;
const MINIMAL_BAR_Y: f32 = 0.68;
const MINIMAL_BAR_W: f32 = 0.8;
const MINIMAL_BAR_H: f32 = 4.0;
const LINE_HEIGHT: f32 = 1.2;
const CARD_PADDING_TOP: f32 = 16.0;
const CARD_PADDING_BOTTOM: f32 = 8.0;
const CARD_GAP: f32 = 8.0;
const CONTROLS_GAP: f32 = 8.0;
const CONTROLS_PADDING_X: f32 = 12.0;
const CONTROLS_PADDING_Y: f32 = 4.0;
const BUTTON_PADDING_X: f32 = 14.0;
const BUTTON_PADDING_Y: f32 = 5.0;
const GEAR_PADDING_X: f32 = 8.0;
const GEAR_ICON: f32 = 16.0;
const SETTINGS_PADDING_X: f32 = 12.0;
const SETTINGS_PADDING_Y: f32 = 6.0;
const SETTINGS_MARGIN_X: f32 = 8.0;
const STYLE_PADDING_X: f32 = 10.0;
const STYLE_PADDING_Y: f32 = 3.0;
const SESSIONS_PADDING_X: f32 = 12.0;
const SESSIONS_PADDING_TOP: f32 = 4.0;
const SESSIONS_HEADER_GAP: f32 = 6.0;
const SESSIONS_HEADER_PADDING_Y: f32 = 4.0;
const SESSIONS_LIST_MAX_H: f32 = 140.0;
const SESSION_ROW_GAP: f32 = 6.0;
const SESSION_ROW_PADDING_X: f32 = 6.0;
const SESSION_ROW_PADDING_Y: f32 = 4.0;
const SESSION_DOT: f32 = 8.0;
const SESSION_MINUTES_W: f32 = 36.0;
const SESSION_FIELD_PADDING_X: f32 = 4.0;
const SESSION_FIELD_PADDING_Y: f32 = 2.0;
const SESSION_KIND_PADDING_X: f32 = 6.0;
const SESSION_PAST_OPACITY: f32 = 0.45;
const ADD_PADDING_X: f32 = 10.0;
const ADD_PADDING_Y: f32 = 3.0;
const ADD_GAP: f32 = 8.0;
const CHEVRON: f32 = 12.0;
const QUARTER_TURN: f32 = std::f32::consts::FRAC_PI_2;
const MUSIC_PADDING: f32 = 10.0;
const MUSIC_MARGIN_X: f32 = 8.0;
const MUSIC_MARGIN_BOTTOM: f32 = 8.0;
const MUSIC_ROW_GAP: f32 = 8.0;
const MUSIC_ROW_PADDING_Y: f32 = 4.0;
const MUSIC_ICON: f32 = 14.0;
const MUSIC_BUTTON_PADDING_X: f32 = 5.0;
const MUSIC_BUTTON_PADDING_Y: f32 = 2.0;
const MUSIC_BUTTON_GAP: f32 = 2.0;
const FOLDER_ROW_GAP: f32 = 6.0;
const START: &str = "Start (Space)";
const PAUSE: &str = "Pause (Space)";
const RESUME: &str = "Resume (Space)";
const SKIP: &str = "Skip";
const RESET: &str = "Reset (R)";
const TIMER_STYLE: &str = "Timer style:";
const SESSION_LIST: &str = "Session List";
const ADD_FOCUS: &str = "+ Focus";
const ADD_BREAK: &str = "+ Break";
const REMOVE: &str = "\u{d7}";
const NO_FOLDER: &str = "Pick a folder to enable music";
const NO_TRACKS: &str = "(no audio files)";
const PRESS_PLAY: &str = "(press play)";
const CHOOSE: &str = "Choose\u{2026}";
const CLEAR: &str = "Clear";
const CHOOSE_PROMPT: &str = "Choose";
const NO_PICKER: &str = "No folder dialog";
const NAME_BLANK: &str = "A session needs a name";
const MINUTES_RANGE: &str = "Minutes: 1 to 120, whole";

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Name,
    Minutes,
}

/// One session row's field, open for typing.
pub struct Editing {
    index: usize,
    field: Field,
    input: Entity<SearchInput>,
}

#[derive(Default)]
pub struct Panel {
    settings_open: bool,
    sessions_open: bool,
    idle: bool,
    last_activity: Option<Instant>,
    editing: Option<Editing>,
    /// The field whose entry was refused, and a count keying its flash.
    rejected: Option<(Field, usize)>,
    reject_seq: u64,
    sessions_scroll: ScrollHandle,
    _tick: Option<Task<()>>,
}

impl Panel {
    pub fn enter(&mut self, cx: &mut Context<Launcher>) {
        self.idle = false;
        self.editing = None;
        self.last_activity = Some(Instant::now());
        // The folder's tracks, read once.
        bg::fetch(cx, || pomo::lock().restore_music(), |_, (), _| {});
        self._tick = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                if this
                    .update(cx, |this, cx| {
                        let panel = &mut this.commands.pomo;
                        let running = pomo::lock().status().running;
                        if running
                            && !panel.idle
                            && panel.editing.is_none()
                            && panel
                                .last_activity
                                .is_some_and(|at| at.elapsed() >= IDLE_FADE)
                        {
                            panel.idle = true;
                        }
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        }));
    }

    pub fn leave(&mut self) {
        self._tick = None;
        self.editing = None;
        self.idle = false;
    }

    /// Faded to the ring: the frame hides its sidebar too.
    pub fn is_idle(&self) -> bool {
        self.idle
    }

    /// The field being typed into, which the launcher's keys edit.
    pub fn editing_field(&self) -> Option<Entity<SearchInput>> {
        self.editing.as_ref().map(|e| e.input.clone())
    }

    fn activity(&mut self) {
        self.last_activity = Some(Instant::now());
        self.idle = false;
    }

    /// Escape with a field open commits it instead of leaving. True when
    /// that happened.
    pub fn dismiss(&mut self, cx: &mut Context<Launcher>) -> bool {
        if self.editing.is_some() {
            self.commit(cx);
            return true;
        }
        if self.idle {
            self.activity();
            cx.notify();
            return true;
        }
        false
    }

    pub fn key(&mut self, key: &str, cx: &mut Context<Launcher>) -> KeyOutcome {
        self.activity();
        if self.editing.is_some() {
            if key == "enter" || key == "tab" {
                self.commit(cx);
                return KeyOutcome::Consumed;
            }
            cx.notify();
            return KeyOutcome::Pass;
        }
        match key {
            "space" => {
                pomo::lock().toggle();
            }
            "r" => pomo::lock().reset(),
            "p" => {
                bg::fetch(cx, || pomo::lock().music_toggle(), |_, (), _| {});
            }
            _ => {
                cx.notify();
                return KeyOutcome::Pass;
            }
        }
        cx.notify();
        KeyOutcome::Consumed
    }

    fn open_field(&mut self, index: usize, field: Field, cx: &mut Context<Launcher>) {
        let text = {
            let pomo = pomo::lock();
            let Some(session) = pomo.sessions.get(index) else {
                return;
            };
            match field {
                Field::Name => session.name.clone(),
                Field::Minutes => session.minutes.to_string(),
            }
        };
        let input = cx.new(SearchInput::new);
        input.update(cx, |input, cx| {
            input.set_text(&text, cx);
            input.select_all(cx);
        });
        self.editing = Some(Editing {
            index,
            field,
            input,
        });
        self.activity();
        cx.notify();
    }

    /// Apply the open field, or say why it could not be.
    fn commit(&mut self, cx: &mut Context<Launcher>) {
        let Some(editing) = self.editing.take() else {
            return;
        };
        let text = editing.input.read(cx).committed();
        let (ok, complaint) = match editing.field {
            Field::Name => (pomo::lock().rename(editing.index, &text), NAME_BLANK),
            Field::Minutes => (
                pomo::lock().set_minutes(editing.index, &text),
                MINUTES_RANGE,
            ),
        };
        if !ok {
            self.rejected = Some((editing.field, editing.index));
            self.reject_seq += 1;
            let launcher = cx.entity();
            cx.defer(move |cx| {
                launcher.update(cx, |this, cx| {
                    this.banner
                        .show(complaint.into(), Tone::Info, banner::MEDIUM, cx)
                });
            });
        }
        cx.notify();
    }

    fn choose_folder(&mut self, cx: &mut Context<Launcher>) {
        let picked = pick::folder(CHOOSE_PROMPT, cx);
        cx.spawn(async move |this, cx| {
            let folder = match picked.await {
                Ok(Some(folder)) => folder.to_string_lossy().into_owned(),
                Ok(None) => return,
                Err(err) => {
                    let _ = this.update(cx, |this, cx| {
                        this.banner.show(
                            format!("{NO_PICKER}: {err}"),
                            Tone::Error,
                            banner::SHORT,
                            cx,
                        );
                    });
                    return;
                }
            };
            cx.background_executor()
                .spawn(async move { pomo::lock().set_music_folder(folder) })
                .await;
            let _ = this.update(cx, |_, cx| cx.notify());
        })
        .detach();
    }
}

pub fn panel(frame: &Commands, th: &Theme, cx: &mut Context<Launcher>) -> Stateful<Div> {
    let state = &frame.pomo;
    let (status, session, count, style, kind) = {
        let pomo = pomo::lock();
        let session = pomo.active_session().cloned();
        (
            pomo.status(),
            session.clone(),
            pomo.sessions.len(),
            pomo.style,
            session.map(|s| s.kind),
        )
    };
    let subtitle = match (&session, status.running) {
        (Some(s), true) => format!("Running: {}", s.name),
        (Some(s), false) => format!("Paused: {}", s.name),
        (None, _) => format!("{count} sessions planned"),
    };
    let colour = match kind {
        None => th.accent,
        Some(Kind::Focus) => th.danger,
        Some(Kind::Break) => th.success,
    };
    let size = if state.idle { CANVAS_IDLE } else { CANVAS };
    let progress = if status.total == 0 || status.active.is_none() {
        0.0
    } else {
        1.0 - status.seconds_left as f32 / status.total as f32
    };
    let card = div()
        .pt(px(if state.idle { 0.0 } else { CARD_PADDING_TOP }))
        .pb(px(CARD_PADDING_BOTTOM))
        .flex()
        .flex_col()
        .items_center()
        .gap(px(CARD_GAP))
        .child(
            div()
                .min_h(px(th.font_size * LINE_HEIGHT))
                .font_weight(FontWeight::SEMIBOLD)
                .child(SharedString::from(
                    session.as_ref().map(|s| s.name.clone()).unwrap_or_default(),
                )),
        )
        .child(timer(
            style,
            size,
            progress,
            // Idle, the face shows the first session's length.
            pomo::format_time(if status.active.is_none() {
                status.total
            } else {
                status.seconds_left
            }),
            colour,
            th,
        ));

    let root = div()
        .id("pomo")
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .on_click(cx.listener(|this, _, _, cx| {
            this.commands.pomo.activity();
            cx.notify();
        }))
        .on_scroll_wheel(cx.listener(|this, _, _, cx| {
            if this.commands.pomo.idle {
                this.commands.pomo.activity();
                cx.notify();
            }
        }));
    if state.idle {
        return root.justify_center().child(card);
    }

    root.child(
        frame
            .bar(th)
            .child(div().flex_1().text_color(th.text_secondary).child(subtitle))
            .child(frame.pill(th)),
    )
    .child(card)
    .child(controls(&status, th, cx))
    .when(state.settings_open, |el| {
        el.child(settings_row(style, th, cx))
    })
    .child(div().flex_1())
    .child(sessions_block(state, &status, th, cx))
    .child(music_card(th, cx))
}

fn button(id: &'static str, label: &'static str, bg: Rgba, fg: Rgba, th: &Theme) -> Stateful<Div> {
    div()
        .id(id)
        .px(px(BUTTON_PADDING_X))
        .py(px(BUTTON_PADDING_Y))
        .rounded(px(th.chip_radius()))
        .bg(bg)
        .text_color(fg)
        .text_size(px(th.font_size - 1.0))
        .font_weight(FontWeight::SEMIBOLD)
        .cursor_pointer()
        .hover(|s| s.opacity(0.9))
        .child(label)
}

fn controls(status: &pomo::Status, th: &Theme, cx: &mut Context<Launcher>) -> Div {
    let (label, bg) = match (status.active, status.running) {
        (None, _) => (START, th.accent),
        (Some(_), true) => (PAUSE, th.warning),
        (Some(_), false) => (RESUME, th.success),
    };
    let toggle = button("pomo-toggle", label, bg, th.on_accent, th).on_click(cx.listener(
        |this, _, _, cx| {
            pomo::lock().toggle();
            this.commands.pomo.activity();
            cx.notify();
        },
    ));
    let gear = div()
        .id("pomo-gear")
        .px(px(GEAR_PADDING_X))
        .py(px(BUTTON_PADDING_Y))
        .rounded(px(th.chip_radius()))
        .bg(th.control_fill)
        .flex()
        .items_center()
        .cursor_pointer()
        .hover(|s| s.opacity(0.9))
        .on_click(cx.listener(|this, _, _, cx| {
            this.commands.pomo.settings_open = !this.commands.pomo.settings_open;
            this.commands.pomo.activity();
            cx.notify();
        }))
        .child(
            svg()
                .path(glyphs::SETTINGS)
                .size(px(GEAR_ICON))
                .text_color(th.text_secondary),
        );
    div()
        .px(px(CONTROLS_PADDING_X))
        .py(px(CONTROLS_PADDING_Y))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(CONTROLS_GAP))
        .child(toggle)
        .when(status.active.is_some(), |el| {
            el.child(
                button("pomo-skip", SKIP, th.control_fill, th.text, th).on_click(cx.listener(
                    |this, _, _, cx| {
                        pomo::lock().skip();
                        this.commands.pomo.activity();
                        cx.notify();
                    },
                )),
            )
            .child(
                button("pomo-reset", RESET, th.danger, th.on_accent, th).on_click(cx.listener(
                    |this, _, _, cx| {
                        pomo::lock().reset();
                        this.commands.pomo.activity();
                        cx.notify();
                    },
                )),
            )
        })
        .child(gear)
}

fn settings_row(current: Style, th: &Theme, cx: &mut Context<Launcher>) -> Div {
    div()
        .mx(px(SETTINGS_MARGIN_X))
        .px(px(SETTINGS_PADDING_X))
        .py(px(SETTINGS_PADDING_Y))
        .rounded(px(th.control_radius()))
        .bg(th.control_fill)
        .flex()
        .items_center()
        .gap(px(CONTROLS_GAP))
        .child(
            div()
                .text_size(px(th.font_size - 1.0))
                .text_color(th.text_secondary)
                .child(TIMER_STYLE),
        )
        .children(STYLES.iter().enumerate().map(|(i, style)| {
            let active = *style == current;
            let style = *style;
            div()
                .id(("pomo-style", i))
                .px(px(STYLE_PADDING_X))
                .py(px(STYLE_PADDING_Y))
                .rounded(px(th.chip_radius()))
                .text_size(px(th.font_size - 1.0))
                .map(|el| {
                    if active {
                        el.bg(th.accent).text_color(th.on_accent)
                    } else {
                        el.hover(|s| s.bg(th.selection_fill))
                    }
                })
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, _, cx| {
                    pomo::lock().set_style(style);
                    this.commands.pomo.activity();
                    cx.notify();
                }))
                .child(style.label())
        }))
}

fn sessions_block(
    state: &Panel,
    status: &pomo::Status,
    th: &Theme,
    cx: &mut Context<Launcher>,
) -> Div {
    let sessions = pomo::lock().sessions.clone();
    let add = |id: &'static str, label: &'static str, kind: Kind| {
        div()
            .id(id)
            .px(px(ADD_PADDING_X))
            .py(px(ADD_PADDING_Y))
            .rounded(px(th.chip_radius()))
            .border(px(1.0))
            .border_color(th.border)
            .text_size(px(th.font_size - 1.0))
            .text_color(th.text_secondary)
            .cursor_pointer()
            .hover(|s| s.bg(th.selection_fill))
            .on_click(cx.listener(move |this, _, _, cx| {
                // Inside the header, whose click folds the list.
                cx.stop_propagation();
                let index = pomo::lock().add_session(kind);
                // The new row opens, listed, named and in view.
                let panel = &mut this.commands.pomo;
                panel.sessions_open = true;
                panel.sessions_scroll.scroll_to_bottom();
                panel.open_field(index, Field::Name, cx);
            }))
            .child(label)
    };
    let open = state.sessions_open;
    let header = div()
        .id("pomo-sessions-toggle")
        .py(px(SESSIONS_HEADER_PADDING_Y))
        .flex()
        .items_center()
        .gap(px(SESSIONS_HEADER_GAP))
        .text_size(px(th.font_size - 1.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(th.text_secondary)
        .cursor_pointer()
        .on_click(cx.listener(|this, _, _, cx| {
            this.commands.pomo.sessions_open = !this.commands.pomo.sessions_open;
            this.commands.pomo.activity();
            cx.notify();
        }))
        // One chevron that turns a quarter, the webview's 0.2 s transition.
        .child(
            svg()
                .path(glyphs::CHEVRON_RIGHT)
                .size(px(CHEVRON))
                .text_color(th.text_secondary)
                .with_animation(
                    ("chevron", usize::from(open)),
                    Animation::new(motion::dur(motion::CHEVRON_MS)),
                    move |icon, t| {
                        let (from, to) = if open {
                            (0.0, QUARTER_TURN)
                        } else {
                            (QUARTER_TURN, 0.0)
                        };
                        let angle = from + (to - from) * motion::curve(t);
                        icon.with_transformation(Transformation::rotate(radians(angle)))
                    },
                ),
        )
        .child(format!("{SESSION_LIST} ({})", sessions.len()))
        .child(div().flex_1())
        .child(
            div()
                .flex()
                .gap(px(ADD_GAP))
                .child(add("pomo-add-focus", ADD_FOCUS, Kind::Focus))
                .child(add("pomo-add-break", ADD_BREAK, Kind::Break)),
        );

    let rows = sessions.iter().enumerate().map(|(i, session)| {
        let past = status.active.is_some_and(|active| i < active);
        let editing = state
            .editing
            .as_ref()
            .filter(|e| e.index == i)
            .map(|e| (e.field, e.input.clone()));
        let rejected = state.rejected;
        let reject_seq = state.reject_seq;
        let field = |which: Field, text: String, grow: bool| -> AnyElement {
            match &editing {
                Some((f, input)) if *f == which => div()
                    .when(grow, |el| el.flex_1().min_w_0())
                    .when(!grow, |el| el.w(px(SESSION_MINUTES_W)))
                    .px(px(SESSION_FIELD_PADDING_X))
                    .py(px(SESSION_FIELD_PADDING_Y))
                    .rounded(px(th.chip_radius() / 1.5))
                    .bg(th.control_fill)
                    .child(search_field(input.clone(), ""))
                    .into_any_element(),
                _ => div()
                    .id((
                        match which {
                            Field::Name => "pomo-name",
                            Field::Minutes => "pomo-minutes",
                        },
                        i,
                    ))
                    .when(grow, |el| el.flex_1().min_w_0().truncate())
                    .when(!grow, |el| {
                        el.w(px(SESSION_MINUTES_W))
                            .text_center()
                            .font_family(th.mono_family.clone())
                            .text_color(th.text_secondary)
                    })
                    .px(px(SESSION_FIELD_PADDING_X))
                    .py(px(SESSION_FIELD_PADDING_Y))
                    .rounded(px(th.chip_radius() / 1.5))
                    .cursor_pointer()
                    .hover(|s| s.bg(th.control_fill))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.commands.pomo.commit(cx);
                        this.commands.pomo.open_field(i, which, cx);
                    }))
                    .child(SharedString::from(text))
                    // A refused entry flashes the danger wash over the box,
                    // the webview's `cmd-pomo-field-reject`.
                    .map(|el| {
                        if rejected == Some((which, i)) {
                            let danger = th.danger;
                            el.with_animation(
                                ("reject", reject_seq),
                                Animation::new(motion::dur(motion::REJECT_MS)),
                                move |el, t| {
                                    let wash =
                                        motion::REJECT_WASH * motion::hump(t, motion::REJECT_PEAK);
                                    let mut flash = danger;
                                    flash.alpha = wash;
                                    el.bg(flash)
                                },
                            )
                            .into_any_element()
                        } else {
                            el.into_any_element()
                        }
                    }),
            }
        };
        div()
            .px(px(SESSION_ROW_PADDING_X))
            .py(px(SESSION_ROW_PADDING_Y))
            .rounded(px(th.chip_radius()))
            .when(past, |el| el.opacity(SESSION_PAST_OPACITY))
            .flex()
            .items_center()
            .gap(px(SESSION_ROW_GAP))
            .text_size(px(th.font_size - 1.0))
            .child(
                div()
                    .size(px(SESSION_DOT))
                    .flex_shrink_0()
                    .rounded_full()
                    .bg(match session.kind {
                        Kind::Focus => th.danger,
                        Kind::Break => th.success,
                    }),
            )
            .child(field(Field::Name, session.name.clone(), true))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(1.0))
                    .child(field(Field::Minutes, session.minutes.to_string(), false))
                    .child(
                        div()
                            .text_size(px(th.font_size - 2.0))
                            .text_color(th.text_muted)
                            .child("m"),
                    ),
            )
            .child(
                div()
                    .id(("pomo-kind", i))
                    .px(px(SESSION_KIND_PADDING_X))
                    .py(px(1.0))
                    .rounded(px(th.chip_radius() / 1.5))
                    .border(px(1.0))
                    .border_color(th.border)
                    .text_size(px(th.font_size - 2.0))
                    .text_color(th.text_secondary)
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        pomo::lock().flip_kind(i);
                        this.commands.pomo.activity();
                        cx.notify();
                    }))
                    .child(match session.kind {
                        Kind::Focus => "F",
                        Kind::Break => "B",
                    }),
            )
            .child(
                div()
                    .id(("pomo-remove", i))
                    .px(px(SESSION_FIELD_PADDING_X))
                    .text_size(px(th.font_size - 2.0))
                    .text_color(th.text_muted)
                    .cursor_pointer()
                    .hover(|s| s.text_color(th.danger))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        pomo::lock().remove_session(i);
                        this.commands.pomo.activity();
                        cx.notify();
                    }))
                    .child(REMOVE),
            )
    });

    div()
        .px(px(SESSIONS_PADDING_X))
        .pt(px(SESSIONS_PADDING_TOP))
        .flex_shrink_0()
        .flex()
        .flex_col()
        .child(header)
        .when(state.sessions_open, |el| {
            el.child(
                div()
                    .id("pomo-sessions")
                    .max_h(px(SESSIONS_LIST_MAX_H))
                    .overflow_y_scroll()
                    .track_scroll(&state.sessions_scroll)
                    .py(px(SESSIONS_HEADER_PADDING_Y))
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .children(rows),
            )
        })
}

fn music_card(th: &Theme, cx: &mut Context<Launcher>) -> Div {
    let (folder, tracks, playing, track) = {
        let pomo = pomo::lock();
        (
            pomo.music_folder.clone(),
            pomo.tracks_len(),
            pomo.music_playing(),
            pomo.track_name(),
        )
    };
    let title = match (&folder, tracks, track) {
        (None, _, _) => NO_FOLDER.to_string(),
        (Some(_), 0, _) => NO_TRACKS.to_string(),
        (Some(_), _, Some(name)) => name,
        (Some(_), _, None) => PRESS_PLAY.to_string(),
    };
    let transport = |id: &'static str, glyph: &'static str, command: &'static str| {
        div()
            .id(id)
            .px(px(MUSIC_BUTTON_PADDING_X))
            .py(px(MUSIC_BUTTON_PADDING_Y))
            .rounded(px(th.chip_radius() / 1.5))
            .text_color(th.text_secondary)
            .cursor_pointer()
            .hover(|s| s.bg(th.selection_fill))
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                bg::fetch(
                    cx,
                    move || pomo::lock().music_command(command),
                    |_, (), _| {},
                );
                this.commands.pomo.activity();
            }))
            .child(
                svg()
                    .path(glyph)
                    .size(px(MUSIC_ICON))
                    .text_color(th.text_secondary),
            )
    };
    let link = |id: &'static str, label: &'static str, colour: Rgba| {
        div()
            .id(id)
            .flex_shrink_0()
            .text_size(px(th.font_size - 2.0))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(colour)
            .cursor_pointer()
            .hover(|s| s.opacity(0.8))
            .child(label)
    };
    div()
        .mx(px(MUSIC_MARGIN_X))
        .mb(px(MUSIC_MARGIN_BOTTOM))
        .p(px(MUSIC_PADDING))
        .rounded(px(th.control_radius()))
        .bg(th.control_fill)
        .flex_shrink_0()
        .flex()
        .flex_col()
        .child(
            div()
                .py(px(MUSIC_ROW_PADDING_Y))
                .flex()
                .items_center()
                .gap(px(MUSIC_ROW_GAP))
                .text_size(px(th.font_size - 1.0))
                .child(
                    svg()
                        .path(glyphs::MUSIC)
                        .size(px(MUSIC_ICON))
                        .text_color(th.accent),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .font_weight(FontWeight::SEMIBOLD)
                        .truncate()
                        .child(title),
                )
                .when(folder.is_some() && tracks > 0, |el| {
                    el.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(MUSIC_BUTTON_GAP))
                            .child(transport("pomo-music-prev", glyphs::SKIP_BACK, "previous"))
                            .child(transport(
                                "pomo-music-toggle",
                                if playing { glyphs::PAUSE } else { glyphs::PLAY },
                                "playpause",
                            ))
                            .child(transport("pomo-music-next", glyphs::SKIP_FORWARD, "next")),
                    )
                }),
        )
        .child(
            div()
                .py(px(MUSIC_ROW_PADDING_Y))
                .flex()
                .items_center()
                .gap(px(FOLDER_ROW_GAP))
                .text_size(px(th.font_size - 2.0))
                .child(
                    svg()
                        .path(glyphs::FOLDER)
                        .size(px(MUSIC_ICON))
                        .text_color(th.accent),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_color(th.text_muted)
                        .truncate()
                        .child(SharedString::from(folder.clone().unwrap_or_default())),
                )
                .child(
                    link("pomo-music-choose", CHOOSE, th.accent).on_click(cx.listener(
                        |this, _, _, cx| {
                            cx.stop_propagation();
                            this.commands.pomo.activity();
                            this.commands.pomo.choose_folder(cx);
                        },
                    )),
                )
                .when(folder.is_some(), |el| {
                    el.child(
                        link("pomo-music-clear", CLEAR, th.danger).on_click(cx.listener(
                            |this, _, _, cx| {
                                cx.stop_propagation();
                                bg::fetch(cx, || pomo::lock().clear_music_folder(), |_, (), _| {});
                                this.commands.pomo.activity();
                            },
                        )),
                    )
                }),
        )
}

/// The timer face: a ring, a dial or digits over a bar, drawn each frame
/// from the numbers.
fn timer(
    style: Style,
    size: f32,
    progress: f32,
    time: String,
    colour: Rgba,
    th: &Theme,
) -> impl IntoElement {
    let text_colour = theme::hsla_of(th.text);
    let mono = th.mono_family.clone();
    canvas(
        move |_, _, _| (),
        move |bounds, (), window, cx| {
            let draw = Face {
                bounds,
                size,
                progress,
                colour: theme::hsla_of(colour),
                text_colour,
                mono,
            };
            match style {
                Style::Modern => draw.ring(&time, window, cx),
                Style::Vintage => draw.dial(window),
                Style::Minimal => draw.minimal(&time, window, cx),
            }
        },
    )
    .size(px(size))
}

struct Face {
    bounds: Bounds<Pixels>,
    size: f32,
    progress: f32,
    colour: gpui::Hsla,
    text_colour: gpui::Hsla,
    mono: String,
}

impl Face {
    fn centre(&self) -> (f32, f32) {
        let c = self.bounds.center();
        (f32::from(c.x), f32::from(c.y))
    }

    /// A full circle as two half arcs.
    fn circle(&self, builder: &mut PathBuilder, r: f32) {
        let (cx, cy) = self.centre();
        builder.move_to(point(px(cx), px(cy - r)));
        builder.arc_to(
            point(px(r), px(r)),
            px(0.0),
            false,
            true,
            point(px(cx), px(cy + r)),
        );
        builder.arc_to(
            point(px(r), px(r)),
            px(0.0),
            false,
            true,
            point(px(cx), px(cy - r)),
        );
    }

    fn ring(&self, time: &str, window: &mut Window, cx: &mut gpui::App) {
        let (cx_, cy) = self.centre();
        let r = self.size / 2.0 - RING_INSET;
        let mut ring_track = PathBuilder::stroke(px(RING_W));
        self.circle(&mut ring_track, r);
        if let Ok(path) = ring_track.build() {
            window.paint_path(path, track(RING_TRACK_ALPHA));
        }
        if self.progress > 0.0 {
            let angle = std::f32::consts::TAU * self.progress.min(0.9999);
            let mut arc = PathBuilder::stroke(px(RING_W));
            arc.move_to(point(px(cx_), px(cy - r)));
            arc.arc_to(
                point(px(r), px(r)),
                px(0.0),
                angle > std::f32::consts::PI,
                true,
                point(px(cx_ + r * angle.sin()), px(cy - r * angle.cos())),
            );
            if let Ok(path) = arc.build() {
                window.paint_path(path, self.colour);
            }
        }
        self.text(time, self.size * RING_TEXT_SCALE, cy, window, cx);
    }

    fn dial(&self, window: &mut Window) {
        let (cx, cy) = self.centre();
        let r = self.size / 2.0 - DIAL_INSET;
        for (weight, major) in [(1.0, false), (2.0, true)] {
            let mut ticks = PathBuilder::stroke(px(weight));
            for i in 0..60 {
                if (i % 5 == 0) != major {
                    continue;
                }
                let angle = (i as f32 / 60.0) * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
                let inner = r - if major {
                    DIAL_TICK_MAJOR
                } else {
                    DIAL_TICK_MINOR
                };
                ticks.move_to(point(
                    px(cx + angle.cos() * inner),
                    px(cy + angle.sin() * inner),
                ));
                ticks.line_to(point(px(cx + angle.cos() * r), px(cy + angle.sin() * r)));
            }
            if let Ok(path) = ticks.build() {
                window.paint_path(path, track(TICK_TRACK_ALPHA));
            }
        }
        let mut hub = PathBuilder::fill();
        self.circle(&mut hub, DIAL_HUB);
        if let Ok(path) = hub.build() {
            window.paint_path(path, self.colour);
        }
        let angle = -std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * self.progress;
        let len = r - DIAL_NEEDLE_INSET;
        let mut needle = PathBuilder::stroke(px(DIAL_NEEDLE_W));
        needle.move_to(point(px(cx), px(cy)));
        needle.line_to(point(
            px(cx + angle.cos() * len),
            px(cy + angle.sin() * len),
        ));
        if let Ok(path) = needle.build() {
            window.paint_path(path, self.colour);
        }
    }

    fn minimal(&self, time: &str, window: &mut Window, cx: &mut gpui::App) {
        let top = f32::from(self.bounds.top());
        let left = f32::from(self.bounds.left());
        self.text(
            time,
            self.size * MINIMAL_TEXT_SCALE,
            top + self.size * MINIMAL_TEXT_Y,
            window,
            cx,
        );
        let bar_w = self.size * MINIMAL_BAR_W;
        let bar_x = left + (self.size - bar_w) / 2.0;
        let bar_y = top + self.size * MINIMAL_BAR_Y;
        let radii = Corners::all(px(MINIMAL_BAR_H / 2.0));
        let mut track_quad = fill(
            Bounds::new(
                point(px(bar_x), px(bar_y)),
                gpui::size(px(bar_w), px(MINIMAL_BAR_H)),
            ),
            track(RING_TRACK_ALPHA),
        );
        track_quad.corner_radii = radii;
        window.paint_quad(track_quad);
        if self.progress > 0.0 {
            let mut done = fill(
                Bounds::new(
                    point(px(bar_x), px(bar_y)),
                    gpui::size(px(bar_w * self.progress), px(MINIMAL_BAR_H)),
                ),
                self.colour,
            );
            done.corner_radii = radii;
            window.paint_quad(done);
        }
    }

    /// Bold mono digits centred on `cy`.
    fn text(&self, text: &str, font_size: f32, cy: f32, window: &mut Window, cx: &mut gpui::App) {
        let mut font = window.text_style().font();
        font.family = self.mono.clone().into();
        font.weight = FontWeight::BOLD;
        let run = TextRun {
            len: text.len(),
            font,
            color: self.text_colour,
            background_color: None,
            underline: None,
            strikethrough: None,
            letter_spacing: None,
        };
        let line = window.text_system().shape_line(
            SharedString::from(text.to_string()),
            px(font_size),
            &[run],
            None,
        );
        let line_height = px(font_size * LINE_HEIGHT);
        let (cx_, _) = self.centre();
        let origin = point(px(cx_) - line.width() / 2.0, px(cy) - line_height / 2.0);
        let _ = line.paint(origin, line_height, TextAlign::Left, None, window, cx);
    }
}
