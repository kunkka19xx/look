//! The settings screen's widgets, built once and shared by its tabs: a
//! switch, a slider, a select, a segmented control, a text box, a button,
//! and the row and section they sit in. Sizes are the webview's
//! `settings.css`.

use std::rc::Rc;

use gpui::prelude::*;
use gpui::{
    AnyElement, App, Bounds, CursorStyle, Div, FontWeight, HitboxBehavior, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, SharedString, Stateful, Window, canvas,
    deferred, div, fill, point, px, relative, size,
};

use crate::theme::{self, Theme};

pub const LABEL_W: f32 = 140.0;
const ROW_GAP: f32 = 10.0;
const ROW_PADDING_Y: f32 = 4.0;
const VALUE_W: f32 = 42.0;
const SECTION_GAP: f32 = 8.0;
const SECTION_PADDING_TOP: f32 = 8.0;
const SECTION_PADDING_BOTTOM: f32 = 2.0;
const SECTION_ARROW: &str = "\u{25b6}";
const DIVIDER_MARGIN_Y: f32 = 6.0;
const TOGGLE_W: f32 = 36.0;
const TOGGLE_H: f32 = 20.0;
const KNOB: f32 = 16.0;
const KNOB_INSET: f32 = (TOGGLE_H - KNOB) / 2.0;
const TRACK_H: f32 = 3.0;
const THUMB: f32 = 12.0;
const SLIDER_H: f32 = 20.0;
const SELECT_MIN_W: f32 = 120.0;
const SELECT_PADDING_X: f32 = 10.0;
const SELECT_PADDING_Y: f32 = 4.0;
const SELECT_GAP: f32 = 8.0;
const SELECT_ARROW: &str = "\u{25be}";
const MENU_GAP: f32 = 4.0;
const MENU_PADDING: f32 = 4.0;
const MENU_ITEM_PADDING_X: f32 = 10.0;
const MENU_ITEM_PADDING_Y: f32 = 5.0;
const SEGMENT_PADDING: f32 = 2.0;
const SEGMENT_GAP: f32 = 2.0;
const SEGMENT_ITEM_PADDING_X: f32 = 14.0;
const SEGMENT_ITEM_PADDING_Y: f32 = 3.0;
/// Every item carries it, clear unless ringed, so a ring moves nothing.
const SEGMENT_RING: f32 = 1.0;
const BUTTON_PADDING_X: f32 = 14.0;
const BUTTON_PADDING_Y: f32 = 5.0;
const BOX_PADDING_X: f32 = 8.0;
const BOX_PADDING_Y: f32 = 4.0;

/// A section title: the arrow and the name, as the webview's header row.
pub fn section(title: &'static str, th: &Theme) -> Div {
    div()
        .pt(px(SECTION_PADDING_TOP))
        .pb(px(SECTION_PADDING_BOTTOM))
        .flex()
        .items_center()
        .gap(px(SECTION_GAP))
        .text_size(px(th.font_size - 1.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(th.text_secondary)
        .child(div().text_size(px(th.font_size - 2.0)).child(SECTION_ARROW))
        .child(title)
}

pub fn divider(th: &Theme) -> Div {
    div().my(px(DIVIDER_MARGIN_Y)).h(px(1.0)).bg(th.panel_fill)
}

/// A control's row: the label column, then whatever the caller adds.
pub fn row(label: impl Into<SharedString>, th: &Theme) -> Div {
    div()
        .py(px(ROW_PADDING_Y))
        .flex()
        .items_center()
        .gap(px(ROW_GAP))
        .child(
            div()
                .w(px(LABEL_W))
                .flex_shrink_0()
                .text_size(px(th.font_size - 1.0))
                .text_color(th.text_secondary)
                .child(label.into()),
        )
}

/// The small print after a control.
pub fn hint(text: impl Into<SharedString>, th: &Theme) -> Div {
    div()
        .text_size(px(th.font_size - 2.0))
        .text_color(th.text_muted)
        .whitespace_nowrap()
        .child(text.into())
}

/// A slider's reading, right-aligned in a fixed column.
pub fn value(text: String, th: &Theme) -> Div {
    div()
        .min_w(px(VALUE_W))
        .flex_shrink_0()
        .text_right()
        .font_family(th.mono_family.clone())
        .text_size(px(th.font_size - 1.0))
        .text_color(th.text_muted)
        .child(text)
}

/// The switch. The caller binds the click.
pub fn toggle(id: &'static str, on: bool, th: &Theme) -> Stateful<Div> {
    div()
        .id(id)
        .w(px(TOGGLE_W))
        .h(px(TOGGLE_H))
        .flex_shrink_0()
        .rounded_full()
        .bg(if on { th.accent } else { th.selection_fill })
        .cursor_pointer()
        .p(px(KNOB_INSET))
        .flex()
        .when(on, |el| el.justify_end())
        .child(div().size(px(KNOB)).rounded_full().bg(if on {
            th.on_accent
        } else {
            th.text_secondary
        }))
}

pub fn button(id: &'static str, label: &'static str, th: &Theme) -> Stateful<Div> {
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
        .child(label)
}

/// A value with its label, for selects and segments.
pub type Option_ = (&'static str, &'static str);

/// Side-by-side choices, one filled.
pub fn segmented(
    id: &'static str,
    options: &'static [Option_],
    active: &str,
    ringed: Option<&str>,
    th: &Theme,
    pick: impl Fn(&str, &mut Window, &mut App) + 'static,
) -> Div {
    let pick = Rc::new(pick);
    div()
        .p(px(SEGMENT_PADDING))
        .rounded(px(th.control_radius()))
        .bg(th.control_fill)
        .flex()
        .gap(px(SEGMENT_GAP))
        .children(options.iter().enumerate().map(|(i, (value, label))| {
            let on = *value == active;
            let ring = !on && ringed == Some(*value);
            let pick = pick.clone();
            div()
                .id((id, i))
                .px(px(SEGMENT_ITEM_PADDING_X - SEGMENT_RING))
                .py(px(SEGMENT_ITEM_PADDING_Y - SEGMENT_RING))
                .border(px(SEGMENT_RING))
                .border_color(if ring {
                    theme::hsla_of(th.accent)
                } else {
                    gpui::transparent_black()
                })
                .rounded(px(th.chip_radius()))
                .text_size(px(th.font_size - 2.0))
                .font_weight(FontWeight::SEMIBOLD)
                .cursor_pointer()
                .map(|el| {
                    if on {
                        el.bg(th.accent).text_color(th.on_accent)
                    } else {
                        el.text_color(th.text_secondary)
                            .hover(|s| s.bg(th.selection_fill))
                    }
                })
                .on_click(move |_, window, cx| pick(value, window, cx))
                .child(*label)
        }))
}

/// A dropdown: the button shows the active label, the list opens under it.
/// `open` is the caller's state, flipped by `on_open` and by a click
/// outside; `pick` gets the chosen value. The list is deferred, so the
/// scrolling body cannot clip it.
pub fn select(
    id: &'static str,
    options: &'static [Option_],
    active: &str,
    open: bool,
    th: &Theme,
    on_open: impl Fn(&bool, &mut Window, &mut App) + 'static,
    pick: impl Fn(&str, &mut Window, &mut App) + 'static,
) -> Div {
    let label = options
        .iter()
        .find(|(value, _)| *value == active)
        .map_or(active.to_string(), |(_, label)| (*label).to_string());
    let on_open = Rc::new(on_open);
    let pick = Rc::new(pick);
    let open_click = on_open.clone();
    let button = div()
        .id(id)
        .min_w(px(SELECT_MIN_W))
        .px(px(SELECT_PADDING_X))
        .py(px(SELECT_PADDING_Y))
        .rounded(px(th.control_radius()))
        .bg(th.control_fill)
        .text_size(px(th.font_size - 1.0))
        .cursor_pointer()
        .hover(|s| s.bg(th.selection_fill))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(SELECT_GAP))
        .on_click(move |_, window, cx| open_click(&!open, window, cx))
        .child(label)
        .child(div().text_color(th.text_muted).child(SELECT_ARROW));
    div().relative().child(button).when(open, |el| {
        el.child(deferred(
            div()
                .id(SharedString::from(format!("{id}-menu")))
                .occlude()
                .absolute()
                .top(relative(1.0))
                .mt(px(MENU_GAP))
                .left_0()
                .min_w(px(SELECT_MIN_W))
                .p(px(MENU_PADDING))
                .rounded(px(th.control_radius()))
                .bg(theme::opaque(th.card_face()))
                .border(px(1.0))
                .border_color(th.border)
                .shadow(th.card_shadow())
                .flex()
                .flex_col()
                .on_mouse_down_out(move |_, window, cx| on_open(&false, window, cx))
                .children(options.iter().enumerate().map(|(i, (value, label))| {
                    let on = *value == active;
                    let pick = pick.clone();
                    div()
                        .id((id, i))
                        .px(px(MENU_ITEM_PADDING_X))
                        .py(px(MENU_ITEM_PADDING_Y))
                        .rounded(px(th.chip_radius()))
                        .text_size(px(th.font_size - 1.0))
                        .whitespace_nowrap()
                        .cursor_pointer()
                        .map(|el| {
                            if on {
                                el.text_color(th.accent).font_weight(FontWeight::SEMIBOLD)
                            } else {
                                el.text_color(th.text)
                            }
                        })
                        .hover(|s| s.bg(th.selection_fill))
                        .on_click(move |_, window, cx| pick(value, window, cx))
                        .child(*label)
                })),
        ))
    })
}

/// A text box: the field while it edits, the text as a button otherwise.
/// The caller binds the click that starts editing.
pub fn text_box(
    id: &'static str,
    width: f32,
    content: AnyElement,
    editing: bool,
    th: &Theme,
) -> Stateful<Div> {
    div()
        .id(id)
        .w(px(width))
        .px(px(BOX_PADDING_X))
        .py(px(BOX_PADDING_Y))
        .rounded(px(th.control_radius()))
        .bg(th.control_fill)
        .border(px(1.0))
        .border_color(if editing { th.accent } else { th.border })
        .text_size(px(th.font_size - 1.0))
        .text_color(th.text)
        .cursor_text()
        .flex()
        .items_center()
        .child(content)
}

/// What a slider reports: the thumb went down, moved, or was let go.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SliderPhase {
    Start,
    Move,
    End,
}

pub struct SliderEvent {
    pub value: f32,
    pub phase: SliderPhase,
}

/// A slider's range, in the control's own units.
#[derive(Clone, Copy)]
pub struct Range {
    pub min: f32,
    pub max: f32,
    pub step: f32,
}

impl Range {
    /// The value at `x` across a track of `width`, on the step grid.
    fn at(&self, x: f32, width: f32) -> f32 {
        let share = if width > 0.0 {
            (x / width).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let raw = self.min + share * (self.max - self.min);
        let stepped = ((raw - self.min) / self.step).round() * self.step + self.min;
        stepped.clamp(self.min, self.max)
    }

    fn share(&self, value: f32) -> f32 {
        if self.max <= self.min {
            return 0.0;
        }
        ((value - self.min) / (self.max - self.min)).clamp(0.0, 1.0)
    }
}

/// The track and thumb, painted; the drag lives on the window's mouse
/// events so it keeps following a pointer that leaves the track.
/// `dragging` is the caller's state, set on `Start` and cleared on `End`.
pub fn slider(
    value: f32,
    range: Range,
    dragging: bool,
    th: &Theme,
    report: impl Fn(&SliderEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let report = Rc::new(report);
    let track = theme::hsla_of(th.selection_fill);
    let filled = theme::hsla_of(th.accent);
    let thumb = theme::hsla_of(if dragging { th.text } else { th.text_secondary });
    canvas(
        move |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
        move |bounds: Bounds<Pixels>, hitbox, window, _| {
            let width = f32::from(bounds.size.width);
            let left = bounds.origin.x;
            let mid_y = bounds.origin.y + bounds.size.height / 2.0;
            let share = range.share(value);
            let thumb_x = left + px((width - THUMB) * share);
            let track_bounds = Bounds::new(
                point(left, mid_y - px(TRACK_H / 2.0)),
                size(bounds.size.width, px(TRACK_H)),
            );
            let mut quad = fill(track_bounds, track);
            quad.corner_radii = gpui::Corners::all(px(TRACK_H / 2.0));
            window.paint_quad(quad);
            let mut quad = fill(
                Bounds::new(
                    track_bounds.origin,
                    size(thumb_x - left + px(THUMB / 2.0), px(TRACK_H)),
                ),
                filled,
            );
            quad.corner_radii = gpui::Corners::all(px(TRACK_H / 2.0));
            window.paint_quad(quad);
            let mut quad = fill(
                Bounds::new(
                    point(thumb_x, mid_y - px(THUMB / 2.0)),
                    size(px(THUMB), px(THUMB)),
                ),
                thumb,
            );
            quad.corner_radii = gpui::Corners::all(px(THUMB / 2.0));
            window.paint_quad(quad);
            window.set_cursor_style(CursorStyle::PointingHand, &hitbox);

            // The thumb is THUMB wide, so the usable track is the bounds less
            // one thumb, and a press lands the thumb's centre under the pointer.
            let value_at =
                move |x: Pixels| range.at(f32::from(x - left) - THUMB / 2.0, width - THUMB);
            let down = report.clone();
            let pressed = hitbox.clone();
            window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                if phase.bubble() && event.button == MouseButton::Left && pressed.is_hovered(window)
                {
                    down(
                        &SliderEvent {
                            value: value_at(event.position.x),
                            phase: SliderPhase::Start,
                        },
                        window,
                        cx,
                    );
                }
            });
            if dragging {
                let moved = report.clone();
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                    if phase.bubble() {
                        moved(
                            &SliderEvent {
                                value: value_at(event.position.x),
                                phase: SliderPhase::Move,
                            },
                            window,
                            cx,
                        );
                    }
                });
            }
            // The release is taken while dragging, and on the track in the
            // frame of the press itself: a quick click has no later frame,
            // and a press left armed would follow the pointer to the next
            // control.
            let up = report.clone();
            window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                if phase.bubble()
                    && event.button == MouseButton::Left
                    && (dragging || hitbox.is_hovered(window))
                {
                    up(
                        &SliderEvent {
                            value: value_at(event.position.x),
                            phase: SliderPhase::End,
                        },
                        window,
                        cx,
                    );
                }
            });
        },
    )
    .flex_1()
    .h(px(SLIDER_H))
}
