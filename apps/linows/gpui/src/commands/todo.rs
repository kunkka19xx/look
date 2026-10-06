//! `/todo`: a card per day with its tasks, an add row and a rename field,
//! undo and redo over the whole set, Save writing it back, and the Stats
//! page with its donuts, streak, trend, year heatmap and insights. The data
//! is the process-wide one in `todo.rs`; this is its screen. Mirrors
//! `screens/commands/todo.js`, which mirrors macOS.

use std::time::Duration;

use chrono::NaiveDate;
use gpui::{
    Anchor, AnchoredPositionMode, Bounds, ClickEvent, Context, Div, Edges, Entity, FontWeight,
    HitboxBehavior, Hsla, MouseMoveEvent, PathBuilder, Pixels, Point, ScrollHandle, SharedString,
    Stateful, Task, Window, anchored, canvas, deferred, div, fill, point, prelude::*, px, size,
    svg,
};

use super::draw::{Type, paint_circle, paint_dot, paint_text};
use super::{Commands, KeyOutcome};
use crate::bg;
use crate::glyphs;
use crate::launcher::Launcher;
use crate::search::{SearchInput, search_field};
use crate::theme::{self, Theme};
use crate::todo::{
    self, EXTENSION_WINDOW_DAYS, FUTURE_GROUP_LIMIT, HEATMAP_WEEKS, Period, TREND_DAYS,
    UNFINISHED_LIMIT,
};

pub const ID: &str = "todo";
const PLACEHOLDER: &str = "Search tasks & dates";
const STATS_SUBTITLE: &str = "Analytics & trends";
const TASKS_TAB: &str = "Tasks";
const STATS_TAB: &str = "Stats";
const ADD_DATE: &str = "Add date";
const SAVE: &str = "Save";
const SAVED: &str = "Saved";
const SAVE_FAILED: &str = "Save failed";
const RETRY: &str = "Retry";
const ADD_TASK: &str = "Add task";
const ADD_PLACEHOLDER: &str = "Task name, then \u{21b5}";
const ADD_HINT: &str = "\u{21b5} add \u{b7} esc";
const EXTENDED: &str = "EXTENDED";
const OVERDUE: &str = "OVERDUE";
const NO_TASKS: &str = "No tasks yet";
const NO_MATCH: &str = "No matching tasks or dates";
const TODAY: &str = "Today";
const REMOVE_MARK: &str = "\u{d7}";
const SAVE_TOAST: Duration = Duration::from_millis(1600);

const TOOLBAR_PADDING_X: f32 = 10.0;
const TOOLBAR_PADDING_TOP: f32 = 6.0;
const TOOLBAR_PADDING_BOTTOM: f32 = 2.0;
const TOOLBAR_MARGIN_X: f32 = 8.0;
const TOOLBAR_GAP: f32 = 8.0;
const COUNT_GAP: f32 = 6.0;
const COUNT_ICON: f32 = 14.0;
const SEGMENT_PADDING: f32 = 2.0;
const SEGMENT_BUTTON_PADDING_X: f32 = 10.0;
const SEGMENT_BUTTON_PADDING_Y: f32 = 3.0;
const BUTTON_PADDING_X: f32 = 10.0;
const BUTTON_PADDING_Y: f32 = 4.0;
const BUTTON_GAP: f32 = 5.0;
const BUTTON_ICON: f32 = 13.0;
const DISABLED_OPACITY: f32 = 0.5;
const DIRTY_DOT: f32 = 6.0;
const CARD_GAP: f32 = 10.0;
const CARD_PADDING_X: f32 = 12.0;
const CARD_PADDING_TOP: f32 = 8.0;
const CARD_PADDING_BOTTOM: f32 = 10.0;
const PAST_OPACITY: f32 = 0.75;
const HEADER_GAP: f32 = 7.0;
const HEADER_PADDING_TOP: f32 = 2.0;
const HEADER_PADDING_BOTTOM: f32 = 6.0;
const RING_SIZE: f32 = 18.0;
const RING_R: f32 = 7.0;
const RING_W: f32 = 2.5;
const ICON_BUTTON_PADDING_X: f32 = 5.0;
const ICON_BUTTON_PADDING_Y: f32 = 3.0;
const ICON_BUTTON_ICON: f32 = 14.0;
const ROW_GAP: f32 = 9.0;
const ROW_PADDING_X: f32 = 2.0;
const ROW_PADDING_Y: f32 = 4.0;
const CHECKBOX: f32 = 16.0;
const CHECKBOX_BORDER: f32 = 1.5;
const CHECK_ICON: f32 = 11.0;
const LOCKED_OPACITY: f32 = 0.55;
const TAG_SIZE: f32 = 9.0;
const TAG_PADDING_X: f32 = 6.0;
const TAG_PADDING_Y: f32 = 1.0;
const TAG_WASH: f32 = 0.14;
const REMOVE_SIZE: f32 = 13.0;
const REMOVE_PADDING_X: f32 = 5.0;
const ADD_GAP: f32 = 8.0;
const ADD_MARGIN_TOP: f32 = 4.0;
const ADD_PADDING_X: f32 = 6.0;
const ADD_PADDING_Y: f32 = 4.0;
const ADD_ICON: f32 = 13.0;
const NOTE_PADDING_Y: f32 = 10.0;
const TOAST_BOTTOM: f32 = 8.0;
const TOAST_PADDING_X: f32 = 12.0;
const TOAST_PADDING_Y: f32 = 7.0;
const TOAST_GAP: f32 = 6.0;
const TOAST_SIZE: f32 = 12.0;
const TOAST_ICON: f32 = 13.0;
const STATS_GAP: f32 = 8.0;
const STAT_CARD_PADDING_X: f32 = 14.0;
const STAT_CARD_PADDING_Y: f32 = 12.0;
const STAT_CELL_GAP: f32 = 8.0;
const DONUT_SIZE: f32 = 68.0;
const DONUT_R: f32 = 26.0;
const DONUT_W: f32 = 5.0;
const DONUT_DONE_SIZE: f32 = 14.0;
const DONUT_TOTAL_SIZE: f32 = 10.0;
const DONUT_DONE_Y: f32 = 27.0;
const DONUT_LINE_Y: f32 = 35.0;
const DONUT_LINE_HALF: f32 = 8.0;
const DONUT_TOTAL_Y: f32 = 44.0;
const STREAK_GAP: f32 = 12.0;
const STREAK_FLAME: f32 = 24.0;
const STREAK_RIGHT_GAP: f32 = 6.0;
const STREAK_DOTS: usize = 7;
const STREAK_DOT: f32 = 6.0;
const STREAK_DOT_GAP: f32 = 4.0;
const SECTION_GAP: f32 = 6.0;
const SECTION_MARGIN_TOP: f32 = 4.0;
const SECTION_ICON: f32 = 12.0;
const TREND_H: f32 = 96.0;
const TREND_PAD_X: f32 = 8.0;
const TREND_PAD_Y: f32 = 10.0;
const TREND_LINE_W: f32 = 1.5;
const TREND_DOT_R: f32 = 2.0;
const TREND_LAST_DOT_R: f32 = 3.5;
const HEAT_GAP: f32 = 3.0;
const HEAT_LABEL_W: f32 = 18.0;
const HEAT_CELL_MIN: f32 = 6.0;
const HEAT_CELL: f32 = 10.0;
const HEAT_RADIUS: f32 = 2.0;
const HEAT_LABEL_SIZE: f32 = 8.0;
const HEAT_ROW_LABELS: [&str; 7] = ["", "M", "", "W", "", "F", ""];
/// The five-step accent ramp the heatmap and its legend share.
const HEAT_OPACITY: [f32; 5] = [0.12, 0.28, 0.5, 0.74, 1.0];
const HEAT_TIP_GAP: f32 = 8.0;
const HEAT_TIP_MARGIN: f32 = 4.0;
const HEAT_TIP_PADDING_X: f32 = 8.0;
const HEAT_TIP_PADDING_Y: f32 = 4.0;
const INSIGHT_GAP: f32 = 4.0;
const INSIGHT_PADDING_Y: f32 = 4.0;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Page {
    #[default]
    Tasks,
    Stats,
}

/// A box open on a card: the add row, or a task's name.
pub struct Field {
    day: NaiveDate,
    task: Option<String>,
    input: Entity<SearchInput>,
}

struct Toast {
    text: String,
    error: bool,
    _timer: Option<Task<()>>,
}

/// The heatmap cell under the pointer and the line its bubble shows.
struct HeatHover {
    tip: SharedString,
    cell: Bounds<Pixels>,
}

#[derive(Default)]
pub struct Panel {
    page: Page,
    field: Option<Field>,
    toast: Option<Toast>,
    saving: bool,
    save_queued: bool,
    days_scroll: ScrollHandle,
    heat_hover: Option<HeatHover>,
}

impl Panel {
    pub fn enter(&mut self, cx: &mut Context<Launcher>) {
        self.page = Page::Tasks;
        self.field = None;
        self.toast = None;
        todo::lock().ensure_today();
        // The store's rows, unless edits are waiting on Save.
        if !todo::lock().is_dirty() {
            bg::fetch(cx, linows_backend::todo::todo_list, |_, rows, _| {
                if let Ok(rows) = rows {
                    todo::lock().replace(rows);
                }
            });
        }
    }

    pub fn leave(&mut self) {
        self.field = None;
        self.toast = None;
        self.heat_hover = None;
    }

    pub fn editing_field(&self) -> Option<Entity<SearchInput>> {
        self.field.as_ref().map(|f| f.input.clone())
    }

    /// Escape with a box open closes it. True when that happened.
    pub fn dismiss(&mut self, cx: &mut Context<Launcher>) -> bool {
        let had = self.field.take().is_some();
        if had {
            cx.notify();
        }
        had
    }

    pub fn key(&mut self, ks: &gpui::Keystroke, cx: &mut Context<Launcher>) -> KeyOutcome {
        let ctrl = ks.modifiers.control;
        let shift = ks.modifiers.shift;
        let alt = ks.modifiers.alt;
        let key = ks.key.as_str();
        // Ctrl+Z and Ctrl+Shift+Z step the history, unless a box is being
        // typed in; Ctrl+N flips the page and Ctrl+S saves, the macOS pair.
        if ctrl && !alt && key == "z" {
            if self.field.is_some() {
                return KeyOutcome::Pass;
            }
            let stepped = if shift {
                todo::lock().redo()
            } else {
                todo::lock().undo()
            };
            if !stepped {
                return KeyOutcome::Pass;
            }
            cx.notify();
            return KeyOutcome::Consumed;
        }
        if ctrl && !shift && !alt && key == "n" {
            self.page = match self.page {
                Page::Tasks => Page::Stats,
                Page::Stats => Page::Tasks,
            };
            self.field = None;
            self.heat_hover = None;
            cx.notify();
            return KeyOutcome::Consumed;
        }
        if ctrl && !shift && !alt && key == "s" {
            self.save(cx);
            return KeyOutcome::Consumed;
        }
        if self.field.is_some() && key == "enter" {
            self.commit(cx);
            return KeyOutcome::Consumed;
        }
        KeyOutcome::Pass
    }

    fn open_add(&mut self, day: NaiveDate, cx: &mut Context<Launcher>) {
        self.open(day, None, "", cx);
    }

    fn open_rename(&mut self, day: NaiveDate, id: String, name: &str, cx: &mut Context<Launcher>) {
        self.open(day, Some(id), name, cx);
    }

    fn open(
        &mut self,
        day: NaiveDate,
        task: Option<String>,
        text: &str,
        cx: &mut Context<Launcher>,
    ) {
        let input = cx.new(SearchInput::new);
        input.update(cx, |input, cx| input.set_text(text, cx));
        self.field = Some(Field { day, task, input });
        cx.notify();
    }

    /// Enter: the add row keeps taking names until the day is full; a
    /// rename closes.
    fn commit(&mut self, cx: &mut Context<Launcher>) {
        let Some(field) = self.field.take() else {
            return;
        };
        let text = field.input.read(cx).committed();
        let mut todo = todo::lock();
        match &field.task {
            None => {
                if todo.add_task(field.day, &text) && todo.open_count(field.day) < UNFINISHED_LIMIT
                {
                    drop(todo);
                    self.open_add(field.day, cx);
                    return;
                }
            }
            Some(id) => todo.rename(field.day, id, &text),
        }
        drop(todo);
        cx.notify();
    }

    fn save(&mut self, cx: &mut Context<Launcher>) {
        if self.saving {
            // Folded into a follow-up pass rather than dropped.
            self.save_queued = true;
            return;
        }
        self.saving = true;
        self.save_queued = false;
        let (tasks, revision) = todo::lock().for_save();
        bg::fetch(
            cx,
            move || linows_backend::todo::todo_save(&tasks),
            move |this, outcome, cx| {
                let panel = &mut this.commands.todo;
                panel.saving = false;
                match outcome {
                    Ok(()) => {
                        todo::lock().mark_saved(revision);
                        panel.toast(SAVED.into(), false, Some(SAVE_TOAST), cx);
                    }
                    Err(err) => panel.toast(format!("{SAVE_FAILED}: {err}"), true, None, cx),
                }
                if panel.save_queued {
                    panel.save(cx);
                }
            },
        );
    }

    /// The capsule at the panel's foot. No `secs` keeps it, which is how a
    /// failure keeps its Retry.
    fn toast(
        &mut self,
        text: String,
        error: bool,
        secs: Option<Duration>,
        cx: &mut Context<Launcher>,
    ) {
        let timer = secs.map(|secs| {
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(secs).await;
                let _ = this.update(cx, |this, cx| {
                    this.commands.todo.toast = None;
                    cx.notify();
                });
            })
        });
        self.toast = Some(Toast {
            text,
            error,
            _timer: timer,
        });
        cx.notify();
    }
}

pub fn panel(frame: &Commands, th: &Theme, cx: &mut Context<Launcher>) -> Div {
    let state = &frame.todo;
    let root = div().relative().flex_1().min_h_0().flex().flex_col();
    match state.page {
        Page::Tasks => root
            .child(frame.input_bar(PLACEHOLDER, th))
            .child(toolbar(state, th, cx))
            .child(days(frame, th, cx))
            .children(state.toast.as_ref().map(|toast| toast_view(toast, th, cx))),
        Page::Stats => root
            .child(stats_bar(frame, th, cx))
            .child(stats(state, th, cx)),
    }
}

// --- Tasks page -----------------------------------------------------------------

fn toolbar(state: &Panel, th: &Theme, cx: &mut Context<Launcher>) -> Div {
    let (dirty, done, total, future) = {
        let todo = todo::lock();
        let (done, total) = todo.today_counts();
        (todo.is_dirty(), done, total, todo.future_count())
    };
    let left = FUTURE_GROUP_LIMIT.saturating_sub(future);
    let count = div()
        .flex()
        .items_center()
        .gap(px(COUNT_GAP))
        .text_size(px(th.font_size - 1.0))
        .text_color(th.text_secondary)
        .child(
            svg()
                .path(glyphs::LIST_CHECKS)
                .size(px(COUNT_ICON))
                .text_color(th.text_secondary),
        )
        .child(format!("{done}/{total} done today"));
    let add_date = button(
        "todo-add-date",
        glyphs::CALENDAR_PLUS,
        format!("{ADD_DATE} + {left}"),
        th,
    )
    .when(left == 0, |el| el.opacity(DISABLED_OPACITY))
    .when(left > 0, |el| {
        el.on_click(cx.listener(|this, _, _, cx| {
            if let Some(day) = todo::lock().add_date() {
                this.commands.todo.open_add(day, cx);
            }
        }))
    });
    let save = button("todo-save", glyphs::SAVE, SAVE.to_string(), th)
        .when(dirty, |el| {
            el.text_color(th.accent)
                .border_color(th.accent)
                .child(div().size(px(DIRTY_DOT)).rounded_full().bg(th.accent))
        })
        .on_click(cx.listener(|this, _, _, cx| this.commands.todo.save(cx)));
    div()
        .mx(px(TOOLBAR_MARGIN_X))
        .px(px(TOOLBAR_PADDING_X))
        .pt(px(TOOLBAR_PADDING_TOP))
        .pb(px(TOOLBAR_PADDING_BOTTOM))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(TOOLBAR_GAP))
        .child(count)
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(TOOLBAR_GAP))
                .child(segmented(state.page, th, cx))
                .child(add_date)
                .child(save),
        )
}

/// Tasks | Stats.
fn segmented(page: Page, th: &Theme, cx: &mut Context<Launcher>) -> Div {
    let tab = |id: &'static str, label: &'static str, target: Page| {
        let active = page == target;
        div()
            .id(id)
            .px(px(SEGMENT_BUTTON_PADDING_X))
            .py(px(SEGMENT_BUTTON_PADDING_Y))
            .rounded(px(th.chip_radius()))
            .text_size(px(th.font_size - 2.0))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(if active {
                th.on_accent
            } else {
                th.text_secondary
            })
            .when(active, |el| el.bg(th.accent))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                let panel = &mut this.commands.todo;
                panel.page = target;
                panel.field = None;
                panel.heat_hover = None;
                cx.notify();
            }))
            .child(label)
    };
    div()
        .p(px(SEGMENT_PADDING))
        .rounded(px(th.chip_radius()))
        .bg(th.control_fill)
        .flex()
        .child(tab("todo-tab-tasks", TASKS_TAB, Page::Tasks))
        .child(tab("todo-tab-stats", STATS_TAB, Page::Stats))
}

/// A toolbar button: glyph, label, a hairline.
fn button(id: &'static str, glyph: &'static str, label: String, th: &Theme) -> Stateful<Div> {
    div()
        .id(id)
        .px(px(BUTTON_PADDING_X))
        .py(px(BUTTON_PADDING_Y))
        .rounded(px(th.chip_radius()))
        .border(px(1.0))
        .border_color(th.border)
        .bg(th.control_fill)
        .text_size(px(th.font_size - 2.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(th.text_secondary)
        .cursor_pointer()
        .hover(|s| s.text_color(th.text).bg(th.selection_fill))
        .flex()
        .items_center()
        .gap(px(BUTTON_GAP))
        .child(
            svg()
                .path(glyph)
                .size(px(BUTTON_ICON))
                .text_color(th.text_secondary),
        )
        .child(label)
}

fn days(frame: &Commands, th: &Theme, cx: &mut Context<Launcher>) -> Div {
    let state = &frame.todo;
    let needle = todo::fold_query(&frame.typed(cx));
    let searching = !needle.is_empty();
    let (cards, hidden_older) = todo::lock().visible(&needle);
    let empty = cards.is_empty();
    let cards: Vec<Div> = cards
        .into_iter()
        .map(|(day, rows)| day_card(state, day, rows, th, cx))
        .collect();
    let note = |text: String| {
        div()
            .py(px(NOTE_PADDING_Y))
            .text_size(px(th.font_size - 1.0))
            .text_color(th.text_muted)
            .text_center()
            .child(text)
    };
    frame.content().child(
        div()
            .id("todo-days")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&state.days_scroll)
            .flex()
            .flex_col()
            .gap(px(CARD_GAP))
            .children(cards)
            .when(empty, |el| {
                el.child(note(if searching { NO_MATCH } else { NO_TASKS }.into()))
            })
            .when(hidden_older > 0, |el| {
                let plural = if hidden_older == 1 { "" } else { "s" };
                el.child(note(format!(
                    "{hidden_older} older day{plural} hidden \u{b7} search to view"
                )))
            }),
    )
}

fn day_card(
    state: &Panel,
    day: NaiveDate,
    rows: Vec<todo::Task>,
    th: &Theme,
    cx: &mut Context<Launcher>,
) -> Div {
    let diff = todo::day_diff(day);
    let past = diff < 0;
    let is_today = diff == 0;
    // The ring shows the whole day even when the search thins the rows.
    let (done, total, open) = {
        let todo = todo::lock();
        let list = todo.days().get(&day);
        (
            list.map_or(0, |l| l.iter().filter(|t| t.done).count()),
            list.map_or(0, Vec::len),
            todo.open_count(day),
        )
    };
    let title = if is_today {
        TODAY.to_string()
    } else {
        todo::weekday(day).to_string()
    };
    let phrase = todo::relative_phrase(diff).filter(|_| !is_today);
    let icon_button = |id: &'static str, glyph: &'static str| {
        div()
            .id(id)
            .px(px(ICON_BUTTON_PADDING_X))
            .py(px(ICON_BUTTON_PADDING_Y))
            .rounded(px(th.chip_radius()))
            .text_color(th.text_secondary)
            .cursor_pointer()
            .hover(|s| s.text_color(th.text).bg(th.selection_fill))
            .child(
                svg()
                    .path(glyph)
                    .size(px(ICON_BUTTON_ICON))
                    .text_color(th.text_secondary),
            )
    };
    let header = div()
        .pt(px(HEADER_PADDING_TOP))
        .pb(px(HEADER_PADDING_BOTTOM))
        .flex()
        .items_center()
        .gap(px(HEADER_GAP))
        .child(ring(RING_SIZE, RING_R, RING_W, fraction(done, total), th))
        .child(
            div()
                .font_weight(FontWeight::BOLD)
                .text_color(if is_today { th.accent } else { th.text })
                .child(title),
        )
        .child(
            div()
                .text_size(px(th.font_size - 1.0))
                .text_color(th.text_secondary)
                .child(todo::month_day(day)),
        )
        .children(phrase.map(|phrase| {
            div()
                .text_size(px(th.font_size - 1.0))
                .text_color(th.text_secondary)
                .child(format!("\u{b7} {phrase}"))
        }))
        .child(div().flex_1())
        .when(!past, |el| {
            el.child(
                icon_button("todo-complete-all", glyphs::LIST_CHECKS).on_click(cx.listener(
                    move |_, _, _, cx| {
                        todo::lock().complete_all(day);
                        cx.notify();
                    },
                )),
            )
            .child(
                icon_button("todo-clear-all", glyphs::TRASH).on_click(cx.listener(
                    move |_, _, _, cx| {
                        todo::lock().clear_all(day);
                        cx.notify();
                    },
                )),
            )
        });
    let rows: Vec<Stateful<Div>> = rows
        .iter()
        .map(|task| task_row(state, day, task, past, th, cx))
        .collect();
    div()
        .px(px(CARD_PADDING_X))
        .pt(px(CARD_PADDING_TOP))
        .pb(px(CARD_PADDING_BOTTOM))
        .rounded(px(th.bar_radius()))
        .bg(th.panel_fill)
        .border(px(1.0))
        .border_color(th.border)
        .when(past, |el| el.opacity(PAST_OPACITY))
        .flex()
        .flex_col()
        .child(header)
        .children(rows)
        .when(!past, |el| el.child(add_row(state, day, open, th, cx)))
}

fn task_row(
    state: &Panel,
    day: NaiveDate,
    task: &todo::Task,
    past: bool,
    th: &Theme,
    cx: &mut Context<Launcher>,
) -> Stateful<Div> {
    let late = -todo::day_diff(day);
    let locked = late > EXTENSION_WINDOW_DAYS;
    let extended = !task.done && late >= 1 && !locked;
    let overdue = !task.done && locked;
    let editing = state
        .field
        .as_ref()
        .filter(|f| f.day == day && f.task.as_deref() == Some(task.id.as_str()))
        .map(|f| f.input.clone());
    let id = task.id.clone();
    let checkbox = div()
        .id(SharedString::from(format!("todo-check-{id}")))
        .size(px(CHECKBOX))
        .flex_shrink_0()
        .rounded(px(th.chip_radius()))
        .border(px(CHECKBOX_BORDER))
        .border_color(if task.done { th.accent } else { th.text_muted })
        .when(task.done, |el| el.bg(th.accent))
        .when(locked, |el| el.opacity(LOCKED_OPACITY))
        .when(!locked, |el| {
            let id = id.clone();
            el.cursor_pointer()
                .on_click(cx.listener(move |_, _, _, cx| {
                    todo::lock().toggle(day, &id);
                    cx.notify();
                }))
        })
        .flex()
        .items_center()
        .justify_center()
        .when(task.done, |el| {
            el.child(
                svg()
                    .path(glyphs::CHECK)
                    .size(px(CHECK_ICON))
                    .text_color(th.on_accent),
            )
        });
    let name: gpui::AnyElement = match editing {
        Some(input) => div()
            .flex_1()
            .min_w_0()
            .child(search_field(input, ""))
            .into_any_element(),
        None => {
            let id = id.clone();
            let text = task.name.clone();
            div()
                .id(SharedString::from(format!("todo-name-{id}")))
                .flex_1()
                .min_w_0()
                .truncate()
                .when(task.done, |el| el.line_through().text_color(th.text_muted))
                .when(!past, |el| {
                    el.on_click(cx.listener(move |this, ev: &ClickEvent, _, cx| {
                        if ev.click_count() >= 2 {
                            this.commands.todo.open_rename(day, id.clone(), &text, cx);
                        }
                    }))
                })
                .child(task.name.clone())
                .into_any_element()
        }
    };
    let tag = |text: &'static str, colour: gpui::Rgba| {
        div()
            .flex_shrink_0()
            .px(px(TAG_PADDING_X))
            .py(px(TAG_PADDING_Y))
            .rounded_full()
            .bg(gpui::Rgba::new(
                colour.color.red,
                colour.color.green,
                colour.color.blue,
                TAG_WASH,
            ))
            .font_family(th.mono_family.clone())
            .text_size(px(TAG_SIZE))
            .font_weight(FontWeight::BOLD)
            .text_color(colour)
            .child(text)
    };
    let remove_id = id.clone();
    div()
        .id(SharedString::from(format!("todo-task-{id}")))
        .group("todo-task")
        .px(px(ROW_PADDING_X))
        .py(px(ROW_PADDING_Y))
        .rounded(px(th.chip_radius()))
        .hover(|s| s.bg(th.selection_fill))
        .flex()
        .items_center()
        .gap(px(ROW_GAP))
        .child(checkbox)
        .child(name)
        .when(extended, |el| el.child(tag(EXTENDED, th.warning)))
        .when(overdue, |el| el.child(tag(OVERDUE, th.danger)))
        .child(
            div()
                .id(SharedString::from(format!("todo-remove-{id}")))
                .px(px(REMOVE_PADDING_X))
                .text_size(px(REMOVE_SIZE))
                .text_color(th.text_secondary)
                .opacity(0.0)
                .group_hover("todo-task", |s| s.opacity(1.0))
                .hover(|s| s.text_color(th.danger))
                .cursor_pointer()
                .on_click(cx.listener(move |_, _, _, cx| {
                    todo::lock().remove(day, &remove_id);
                    cx.notify();
                }))
                .child(REMOVE_MARK),
        )
}

/// The card's foot: the open box, the cap notice, or the dashed add button.
fn add_row(
    state: &Panel,
    day: NaiveDate,
    open: usize,
    th: &Theme,
    cx: &mut Context<Launcher>,
) -> Div {
    let base = div()
        .mt(px(ADD_MARGIN_TOP))
        .px(px(ADD_PADDING_X))
        .py(px(ADD_PADDING_Y))
        .rounded(px(th.chip_radius()))
        .border(px(1.0))
        .border_dashed()
        .border_color(th.border)
        .text_size(px(th.font_size - 1.0))
        .text_color(th.text_muted)
        .flex()
        .items_center()
        .gap(px(ADD_GAP));
    let adding = state
        .field
        .as_ref()
        .filter(|f| f.day == day && f.task.is_none())
        .map(|f| f.input.clone());
    if let Some(input) = adding {
        return base
            .border_color(th.accent)
            .text_color(th.accent)
            .child(
                svg()
                    .path(glyphs::PLUS)
                    .size(px(ADD_ICON))
                    .text_color(th.accent),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(th.font_size))
                    .text_color(th.text)
                    .child(search_field(input, ADD_PLACEHOLDER)),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .text_size(px(th.font_size - 3.0))
                    .text_color(th.text_muted)
                    .child(ADD_HINT),
            );
    }
    if open >= UNFINISHED_LIMIT {
        return base.child(format!(
            "{UNFINISHED_LIMIT} unfinished \u{b7} complete one to add more"
        ));
    }
    base.child(
        div()
            .id(SharedString::from(format!("todo-add-{day}")))
            .flex_1()
            .cursor_pointer()
            .hover(|s| s.text_color(th.text))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.commands.todo.open_add(day, cx);
            }))
            .flex()
            .items_center()
            .gap(px(ADD_GAP))
            .child(
                svg()
                    .path(glyphs::PLUS)
                    .size(px(ADD_ICON))
                    .text_color(th.text_muted),
            )
            .child(ADD_TASK),
    )
}

fn toast_view(toast: &Toast, th: &Theme, cx: &mut Context<Launcher>) -> Div {
    let capsule = div()
        .id("todo-toast")
        .px(px(TOAST_PADDING_X))
        .py(px(TOAST_PADDING_Y))
        .rounded_full()
        .bg(if toast.error { th.danger } else { th.accent })
        .text_color(th.on_accent)
        .text_size(px(TOAST_SIZE))
        .font_weight(FontWeight::SEMIBOLD)
        .flex()
        .items_center()
        .gap(px(TOAST_GAP))
        .when(!toast.error, |el| {
            el.child(
                svg()
                    .path(glyphs::CHECK)
                    .size(px(TOAST_ICON))
                    .text_color(th.on_accent),
            )
        })
        .child(toast.text.clone())
        .when(toast.error, |el| {
            el.cursor_pointer()
                .on_click(cx.listener(|this, _, _, cx| this.commands.todo.save(cx)))
                .child(div().text_decoration_1().child(RETRY))
        });
    div()
        .absolute()
        .bottom(px(TOAST_BOTTOM))
        .left_0()
        .right_0()
        .flex()
        .justify_center()
        .child(capsule)
}

// --- Stats page -----------------------------------------------------------------

/// The stats page's header: the chart glyph, the subtitle, the tabs, the pill.
fn stats_bar(frame: &Commands, th: &Theme, cx: &mut Context<Launcher>) -> Div {
    div()
        .mt(px(super::BAR_MARGIN_TOP))
        .mx(px(super::BAR_MARGIN_X))
        .px(px(super::BAR_PADDING_X))
        .py(px(super::BAR_PADDING_Y))
        .rounded(px(th.bar_radius()))
        .bg(th.control_fill)
        .flex()
        .items_center()
        .gap(px(super::BAR_GAP))
        .child(
            svg()
                .path(glyphs::BAR_CHART)
                .size(px(super::BAR_ICON))
                .text_color(th.accent),
        )
        .child(
            div()
                .flex_1()
                .text_color(th.text_secondary)
                .child(STATS_SUBTITLE),
        )
        .child(segmented(frame.todo.page, th, cx))
        .child(frame.pill(th))
}

fn stats(state: &Panel, th: &Theme, cx: &mut Context<Launcher>) -> Stateful<Div> {
    let counts = todo::lock().counts();
    let trend = todo::trend(&counts);
    let week = todo::period_stat(&counts, Period::Week);
    let month = todo::period_stat(&counts, Period::Month);
    let streak = todo::streak_days(&counts);
    let today = todo::today();
    let card = |th: &Theme| {
        div()
            .px(px(STAT_CARD_PADDING_X))
            .py(px(STAT_CARD_PADDING_Y))
            .rounded(px(th.bar_radius()))
            .bg(th.panel_fill)
            .border(px(1.0))
            .border_color(th.border)
    };
    let separator = || div().w(px(1.0)).bg(th.border);
    let strip = card(th)
        .flex()
        .items_stretch()
        .justify_around()
        .child(donut("THIS WEEK", week, th))
        .child(separator())
        .child(donut("THIS MONTH", month, th))
        .child(separator())
        .child(streak_cell(streak, th));
    let axis = |text: String| {
        div()
            .font_family(th.mono_family.clone())
            .text_size(px(th.font_size - 4.0))
            .text_color(th.text_muted)
            .child(text)
    };
    let trend_card = card(th)
        .flex()
        .flex_col()
        .child(trend_chart(trend.clone(), th))
        .child(
            div()
                .flex()
                .justify_between()
                .child(axis(todo::month_day(
                    today - chrono::Days::new((TREND_DAYS - 1) as u64),
                )))
                .child(axis(todo::month_day(
                    today - chrono::Days::new((TREND_DAYS / 2) as u64),
                )))
                .child(axis(todo::month_day(today))),
        );
    let heat_card = card(th).child(heatmap(&counts, th, cx));
    let insights = card(th)
        .flex()
        .items_stretch()
        .children(insight_tiles(&trend, th));
    let content_padding = super::CONTENT_PADDING;
    // Taller than the panel at most sizes, so it scrolls, as the webview's
    // content box does.
    div()
        .id("todo-stats")
        .flex_1()
        .min_h_0()
        .mx(px(super::CONTENT_MARGIN))
        .mb(px(super::CONTENT_MARGIN))
        .p(px(content_padding))
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .gap(px(STATS_GAP))
        .child(strip)
        .child(section_title(
            glyphs::ACTIVITY,
            format!("COMPLETION TREND \u{b7} {TREND_DAYS} DAYS"),
            None,
            th,
        ))
        .child(trend_card)
        .child(section_title(
            glyphs::CALENDAR,
            "ACTIVITY \u{b7} LAST YEAR".into(),
            Some(heat_legend(th)),
            th,
        ))
        .child(heat_card)
        .children(state.heat_hover.as_ref().map(|hover| heat_tip(hover, th)))
        .child(section_title(
            glyphs::ZAP,
            format!("INSIGHTS \u{b7} LAST {TREND_DAYS} DAYS (TASKS)"),
            None,
            th,
        ))
        .child(insights)
}

fn section_title(glyph: &'static str, text: String, trailing: Option<Div>, th: &Theme) -> Div {
    div()
        .mt(px(SECTION_MARGIN_TOP))
        .flex()
        .items_center()
        .gap(px(SECTION_GAP))
        .font_family(th.mono_family.clone())
        .text_size(px(th.font_size - 3.0))
        .text_color(th.text_secondary)
        .child(
            svg()
                .path(glyph)
                .size(px(SECTION_ICON))
                .text_color(th.text_secondary),
        )
        .child(text)
        .children(trailing.map(|t| div().flex_1().flex().justify_end().child(t)))
}

fn stat_label(text: String, th: &Theme) -> Div {
    div()
        .font_family(th.mono_family.clone())
        .text_size(px(th.font_size - 3.0))
        .text_color(th.text_secondary)
        .child(text)
}

fn donut(label: &'static str, (done, total): (usize, usize), th: &Theme) -> Div {
    let f = fraction(done, total);
    let pct = (f * 100.0).round() as u32;
    let mono = th.mono_family.clone();
    let colours = (
        theme::hsla_of(th.border),
        theme::hsla_of(th.accent),
        theme::hsla_of(th.text),
        theme::hsla_of(th.text_secondary),
    );
    let face = canvas(
        move |_, _, _| (),
        move |bounds, (), window, cx| {
            let (track, accent, text, secondary) = colours;
            let centre = bounds.center();
            paint_ring(window, centre, DONUT_R, DONUT_W, f, track, accent);
            let top = f32::from(bounds.origin.y);
            let cx_ = f32::from(centre.x);
            paint_text(
                window,
                cx,
                &done.to_string(),
                &Type {
                    size: DONUT_DONE_SIZE,
                    family: &mono,
                    weight: FontWeight::BOLD,
                    colour: text,
                    spacing: 0.0,
                },
                cx_,
                top + DONUT_DONE_Y,
            );
            window.paint_quad(fill(
                Bounds::new(
                    point(px(cx_ - DONUT_LINE_HALF), px(top + DONUT_LINE_Y)),
                    size(px(DONUT_LINE_HALF * 2.0), px(1.0)),
                ),
                track,
            ));
            paint_text(
                window,
                cx,
                &total.to_string(),
                &Type {
                    size: DONUT_TOTAL_SIZE,
                    family: &mono,
                    weight: FontWeight::NORMAL,
                    colour: secondary,
                    spacing: 0.0,
                },
                cx_,
                top + DONUT_TOTAL_Y,
            );
        },
    )
    .size(px(DONUT_SIZE));
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(STAT_CELL_GAP))
        .child(
            stat_label(label.to_string(), th).flex().gap(px(4.0)).child(
                div()
                    .font_weight(FontWeight::BOLD)
                    .text_color(th.accent)
                    .child(format!("{pct}%")),
            ),
        )
        .child(face)
}

/// The flame, then the count with the last seven days as dots under it.
fn streak_cell(streak: usize, th: &Theme) -> Div {
    let filled = streak.min(STREAK_DOTS);
    let dots = (0..STREAK_DOTS).map(|i| {
        let lit = i >= STREAK_DOTS - filled;
        div()
            .size(px(STREAK_DOT))
            .rounded_full()
            .border(px(1.0))
            .border_color(if lit { th.accent } else { th.border })
            .when(lit, |el| el.bg(th.accent))
    });
    let plural = if streak == 1 { "" } else { "s" };
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(STAT_CELL_GAP))
        .child(stat_label("STREAK".into(), th))
        .child(
            div()
                .h(px(DONUT_SIZE))
                .flex()
                .items_center()
                .gap(px(STREAK_GAP))
                .child(
                    svg()
                        .path(glyphs::FLAME)
                        .size(px(STREAK_FLAME))
                        .text_color(th.warning),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_start()
                        .gap(px(STREAK_RIGHT_GAP))
                        .child(
                            div()
                                .flex()
                                .items_baseline()
                                .gap(px(4.0))
                                .text_size(px(th.font_size - 1.0))
                                .text_color(th.text_secondary)
                                .child(
                                    div()
                                        .text_size(px(th.font_size + 2.0))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(th.text)
                                        .child(streak.to_string()),
                                )
                                .child(format!("day{plural}")),
                        )
                        .child(div().flex().gap(px(STREAK_DOT_GAP)).children(dots)),
                ),
        )
}

/// Completed per day over the month: a baseline, the line, a dot per day,
/// the last one larger.
fn trend_chart(data: Vec<usize>, th: &Theme) -> impl IntoElement {
    let accent = theme::hsla_of(th.accent);
    let track = theme::hsla_of(th.border);
    let face = theme::hsla_of(th.panel_fill);
    canvas(
        move |_, _, _| (),
        move |bounds, (), window, _| {
            let w = f32::from(bounds.size.width);
            let h = f32::from(bounds.size.height);
            let left = f32::from(bounds.origin.x);
            let top = f32::from(bounds.origin.y);
            let max = data
                .iter()
                .copied()
                .max()
                .unwrap_or(0)
                .max(UNFINISHED_LIMIT) as f32;
            let step = (w - TREND_PAD_X * 2.0) / (data.len().max(2) - 1) as f32;
            let x = |i: usize| left + TREND_PAD_X + i as f32 * step;
            let y = |v: usize| top + h - TREND_PAD_Y - (v as f32 / max) * (h - TREND_PAD_Y * 2.0);
            window.paint_quad(fill(
                Bounds::new(
                    point(px(left + TREND_PAD_X), px(y(0))),
                    size(px(w - TREND_PAD_X * 2.0), px(1.0)),
                ),
                track,
            ));
            let mut line = PathBuilder::stroke(px(TREND_LINE_W));
            for (i, v) in data.iter().enumerate() {
                let p = point(px(x(i)), px(y(*v)));
                if i == 0 {
                    line.move_to(p);
                } else {
                    line.line_to(p);
                }
            }
            if let Ok(path) = line.build() {
                window.paint_path(path, accent);
            }
            for (i, v) in data.iter().enumerate() {
                let last = i + 1 == data.len();
                let r = if last { TREND_LAST_DOT_R } else { TREND_DOT_R };
                let centre = point(px(x(i)), px(y(*v)));
                paint_dot(
                    window,
                    centre,
                    r,
                    if last || *v > 0 { accent } else { face },
                );
                if !last && *v == 0 {
                    paint_circle(window, centre, r, 1.0, accent);
                }
            }
        },
    )
    .w_full()
    .h(px(TREND_H))
}

/// Where the year grid's cells land: one column a week, Sundays on top.
/// Cells fill the width up to ten pixels; too narrow for six, the oldest
/// weeks drop off. The paint and its hover test share the arithmetic.
struct HeatGrid {
    left: f32,
    top: f32,
    cell: f32,
    weeks: usize,
    last_sunday: NaiveDate,
    today: NaiveDate,
}

impl HeatGrid {
    fn fit(bounds: Bounds<Pixels>, today: NaiveDate) -> Self {
        let w = f32::from(bounds.size.width);
        let mut weeks = HEATMAP_WEEKS;
        let mut cell = ((w - HEAT_LABEL_W - HEAT_GAP * weeks as f32) / weeks as f32).floor();
        if cell < HEAT_CELL_MIN {
            cell = HEAT_CELL_MIN;
            weeks = (((w - HEAT_LABEL_W) / (cell + HEAT_GAP)).floor() as usize).max(1);
        }
        let last_sunday = today
            - chrono::Days::new(chrono::Datelike::weekday(&today).num_days_from_sunday() as u64);
        Self {
            left: f32::from(bounds.origin.x),
            top: f32::from(bounds.origin.y),
            cell: cell.min(HEAT_CELL),
            weeks,
            last_sunday,
            today,
        }
    }

    fn stride(&self) -> f32 {
        self.cell + HEAT_GAP
    }

    fn day(&self, col: usize, row: usize) -> NaiveDate {
        let back = (self.weeks - 1 - col) as u64;
        self.last_sunday - chrono::Days::new(7 * back) + chrono::Days::new(row as u64)
    }

    fn cell_bounds(&self, col: usize, row: usize) -> Bounds<Pixels> {
        Bounds::new(
            point(
                px(self.left + HEAT_LABEL_W + col as f32 * self.stride()),
                px(self.top + row as f32 * self.stride()),
            ),
            size(px(self.cell), px(self.cell)),
        )
    }

    /// The painted cell under `at`; the gaps between cells count as none.
    fn hit(&self, at: Point<Pixels>) -> Option<(NaiveDate, Bounds<Pixels>)> {
        let x = f32::from(at.x) - self.left - HEAT_LABEL_W;
        let y = f32::from(at.y) - self.top;
        if x < 0.0 || y < 0.0 || x % self.stride() >= self.cell || y % self.stride() >= self.cell {
            return None;
        }
        let col = (x / self.stride()) as usize;
        let row = (y / self.stride()) as usize;
        if col >= self.weeks || row >= 7 {
            return None;
        }
        let day = self.day(col, row);
        (day <= self.today).then(|| (day, self.cell_bounds(col, row)))
    }
}

fn heatmap(counts: &todo::Counts, th: &Theme, cx: &mut Context<Launcher>) -> impl IntoElement {
    let accent = th.accent;
    let muted = theme::hsla_of(th.text_muted);
    let mono = th.mono_family.clone();
    let counts = counts.clone();
    let launcher = cx.weak_entity();
    let height = HEAT_CELL * 7.0 + HEAT_GAP * 6.0;
    canvas(
        move |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
        move |bounds, hitbox, window, cx| {
            let grid = HeatGrid::fit(bounds, todo::today());
            let stride = grid.stride();
            for (row, label) in HEAT_ROW_LABELS.iter().enumerate() {
                if !label.is_empty() {
                    paint_text(
                        window,
                        cx,
                        label,
                        &Type {
                            size: HEAT_LABEL_SIZE,
                            family: &mono,
                            weight: FontWeight::NORMAL,
                            colour: muted,
                            spacing: 0.0,
                        },
                        grid.left + HEAT_LABEL_W / 3.0,
                        grid.top + row as f32 * stride + grid.cell / 2.0,
                    );
                }
            }
            for col in 0..grid.weeks {
                for row in 0..7 {
                    let day = grid.day(col, row);
                    if day > grid.today {
                        continue;
                    }
                    let done = counts.get(&day).map_or(0, |c| c.0);
                    let colour = gpui::Rgba::new(
                        accent.color.red,
                        accent.color.green,
                        accent.color.blue,
                        HEAT_OPACITY[todo::heat_level(done)],
                    );
                    let mut quad = fill(grid.cell_bounds(col, row), theme::hsla_of(colour));
                    quad.corner_radii = gpui::Corners::all(px(HEAT_RADIUS));
                    window.paint_quad(quad);
                }
            }
            // The bubble follows the pointer from cell to cell; a move within
            // one cell, or outside the grid with no bubble up, changes nothing.
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                if !phase.bubble() {
                    return;
                }
                let hit = hitbox
                    .is_hovered(window)
                    .then(|| grid.hit(event.position))
                    .flatten();
                launcher
                    .update(cx, |this, cx| {
                        let panel = &mut this.commands.todo;
                        let unchanged = match (&panel.heat_hover, &hit) {
                            (None, None) => true,
                            (Some(shown), Some((_, cell))) => shown.cell == *cell,
                            _ => false,
                        };
                        if unchanged {
                            return;
                        }
                        panel.heat_hover = hit.map(|(day, cell)| HeatHover {
                            tip: heat_tip_text(day, counts.get(&day)),
                            cell,
                        });
                        cx.notify();
                    })
                    .ok();
            });
        },
    )
    .w_full()
    .h(px(height))
}

/// `Mon, Sep 7: 2/3 done`, or `no tasks` for an empty day.
fn heat_tip_text(day: NaiveDate, counts: Option<&(usize, usize)>) -> SharedString {
    let when = format!("{}, {}", todo::weekday(day), todo::month_day(day));
    match counts {
        Some((done, total)) if *total > 0 => format!("{when}: {done}/{total} done"),
        _ => format!("{when}: no tasks"),
    }
    .into()
}

/// The hover bubble above a heatmap cell. Deferred and window-anchored so
/// the scrolling stats column does not clip it.
fn heat_tip(hover: &HeatHover, th: &Theme) -> impl IntoElement {
    let cell = hover.cell;
    deferred(
        anchored()
            .position_mode(AnchoredPositionMode::Window)
            .anchor(Anchor::BottomCenter)
            .position(point(
                cell.origin.x + cell.size.width / 2.0,
                cell.origin.y - px(HEAT_TIP_GAP),
            ))
            .snap_to_window_with_margin(Edges::all(px(HEAT_TIP_MARGIN)))
            .child(
                div()
                    .px(px(HEAT_TIP_PADDING_X))
                    .py(px(HEAT_TIP_PADDING_Y))
                    .rounded(px(th.chip_radius()))
                    .bg(th.card_face())
                    .border(px(1.0))
                    .border_color(th.border)
                    .shadow(th.card_shadow())
                    .text_size(px(th.font_size - 2.0))
                    .text_color(th.text)
                    .whitespace_nowrap()
                    .child(hover.tip.clone()),
            ),
    )
}

/// Less, the five steps, More; right-aligned in the section title.
fn heat_legend(th: &Theme) -> Div {
    let swatches = HEAT_OPACITY.iter().map(|alpha| {
        div()
            .size(px(HEAT_CELL))
            .rounded(px(HEAT_RADIUS))
            .bg(gpui::Rgba::new(
                th.accent.color.red,
                th.accent.color.green,
                th.accent.color.blue,
                *alpha,
            ))
    });
    div()
        .flex()
        .items_center()
        .gap(px(HEAT_GAP))
        .text_size(px(th.font_size - 4.0))
        .text_color(th.text_muted)
        .child("Less")
        .children(swatches)
        .child("More")
}

fn insight_tiles(trend: &[usize], th: &Theme) -> Vec<Div> {
    let total: usize = trend.iter().sum();
    let days = trend.len();
    let tiles = [
        (
            format!("{:.1}", total as f32 / days.max(1) as f32),
            "AVG / DAY".to_string(),
        ),
        (
            trend.iter().max().copied().unwrap_or(0).to_string(),
            "BEST DAY".to_string(),
        ),
        (
            format!("{}/{days}", trend.iter().filter(|v| **v > 0).count()),
            "ACTIVE DAYS".to_string(),
        ),
        (total.to_string(), format!("DONE \u{b7} {days}D")),
    ];
    let mut out = Vec::new();
    for (i, (value, label)) in tiles.into_iter().enumerate() {
        if i > 0 {
            out.push(div().w(px(1.0)).bg(th.border));
        }
        out.push(
            div()
                .flex_1()
                .py(px(INSIGHT_PADDING_Y))
                .flex()
                .flex_col()
                .items_center()
                .gap(px(INSIGHT_GAP))
                .child(
                    div()
                        .font_family(th.mono_family.clone())
                        .text_size(px(th.font_size + 4.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(th.warning)
                        .child(value),
                )
                .child(
                    div()
                        .font_family(th.mono_family.clone())
                        .text_size(px(th.font_size - 4.0))
                        .text_color(th.text_secondary)
                        .child(label),
                ),
        );
    }
    out
}

// --- Drawing --------------------------------------------------------------------

fn fraction(done: usize, total: usize) -> f32 {
    if total == 0 {
        0.0
    } else {
        done as f32 / total as f32
    }
}

/// A day's progress ring at `size`: the track, then the arc.
fn ring(size_px: f32, r: f32, width: f32, f: f32, th: &Theme) -> impl IntoElement {
    let track = theme::hsla_of(th.border);
    let accent = theme::hsla_of(th.accent);
    canvas(
        move |_, _, _| (),
        move |bounds, (), window, _| {
            paint_ring(window, bounds.center(), r, width, f, track, accent);
        },
    )
    .size(px(size_px))
    .flex_shrink_0()
}

fn paint_ring(
    window: &mut Window,
    centre: Point<Pixels>,
    r: f32,
    width: f32,
    f: f32,
    track: Hsla,
    accent: Hsla,
) {
    paint_circle(window, centre, r, width, track);
    if f <= 0.0 {
        return;
    }
    let (cx, cy) = (f32::from(centre.x), f32::from(centre.y));
    let angle = std::f32::consts::TAU * f.min(0.9999);
    let mut arc = PathBuilder::stroke(px(width));
    arc.move_to(point(px(cx), px(cy - r)));
    arc.arc_to(
        point(px(r), px(r)),
        px(0.0),
        angle > std::f32::consts::PI,
        true,
        point(px(cx + r * angle.sin()), px(cy - r * angle.cos())),
    );
    if let Ok(path) = arc.build() {
        window.paint_path(path, accent);
    }
}
