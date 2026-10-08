//! Single-line query field. The entity owns the text and speaks
//! EntityInputHandler, which is how Wayland text-input-v3 (fcitx5) reaches it:
//! preedit arrives as marked text, a commit as a plain replace.

use std::ops::Range;
use std::time::Instant;

use gpui::{
    App, AppContext, Bounds, Context, Element, ElementInputHandler, Entity, EntityInputHandler,
    EventEmitter, FocusHandle, GlobalElementId, Hsla, InspectorElementId, IntoElement, LayoutId,
    PaintQuad, Pixels, ShapedLine, SharedString, TextAlign, TextRun, Transition, TransitionState,
    UTF16Selection, UnderlineStyle, Window, fill, point, px, size,
};

use crate::motion;
use crate::theme;

pub struct Changed;

pub struct SearchInput {
    pub focus_handle: FocusHandle,
    text: String,
    cursor: usize,
    /// The other end of the selection, when there is one; the cursor is the
    /// moving end.
    anchor: Option<usize>,
    marked: Option<Range<usize>>,
    caret: Bounds<Pixels>,
    /// The caret's x, gliding to each new column.
    caret_x: Entity<TransitionState<f32>>,
    /// When the caret last moved: solid for a while after, then blinking.
    caret_moved: Instant,
    /// When the field appeared, for the placeholder's slide in.
    shown: Instant,
}

impl EventEmitter<Changed> for SearchInput {}

impl SearchInput {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            text: String::new(),
            cursor: 0,
            anchor: None,
            marked: None,
            caret: Bounds::default(),
            caret_x: cx.new(|_| TransitionState::new(0.0)),
            caret_moved: Instant::now(),
            shown: Instant::now(),
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// The text without any preedit, for a field that commits its value.
    /// Search reads `text()` instead so results follow the preedit too.
    pub fn committed(&self) -> String {
        match &self.marked {
            Some(range) => {
                let mut s = self.text.clone();
                s.replace_range(range.clone(), "");
                s
            }
            None => self.text.clone(),
        }
    }

    /// The selected byte range, normalised, when it is not empty.
    pub fn selection(&self) -> Option<Range<usize>> {
        let anchor = self.anchor?;
        if anchor == self.cursor {
            return None;
        }
        Some(anchor.min(self.cursor)..anchor.max(self.cursor))
    }

    pub fn selected_text(&self) -> Option<String> {
        self.selection().map(|r| self.text[r].to_string())
    }

    /// Removes the selection, so a keystroke replaces it. `true` when there
    /// was one.
    fn delete_selection(&mut self) -> bool {
        let Some(range) = self.selection() else {
            self.anchor = None;
            return false;
        };
        self.text.replace_range(range.clone(), "");
        self.cursor = range.start;
        self.anchor = None;
        true
    }

    pub fn backspace(&mut self, cx: &mut Context<Self>) {
        if !self.delete_selection() {
            if self.cursor == 0 {
                return;
            }
            let prev = prev_char(&self.text, self.cursor);
            self.text.replace_range(prev..self.cursor, "");
            self.cursor = prev;
        }
        self.changed(cx);
    }

    pub fn delete(&mut self, cx: &mut Context<Self>) {
        if !self.delete_selection() {
            if self.cursor >= self.text.len() {
                return;
            }
            let next = next_char(&self.text, self.cursor);
            self.text.replace_range(self.cursor..next, "");
        }
        self.changed(cx);
    }

    /// Ctrl+Backspace, Ctrl+W: the word before the cursor.
    pub fn delete_word_back(&mut self, cx: &mut Context<Self>) {
        if !self.delete_selection() {
            let start = prev_word(&self.text, self.cursor);
            if start == self.cursor {
                return;
            }
            self.text.replace_range(start..self.cursor, "");
            self.cursor = start;
        }
        self.changed(cx);
    }

    /// Moves the cursor to `to`, extending the selection when `select`, or
    /// dropping it otherwise.
    fn move_to(&mut self, to: usize, select: bool, cx: &mut Context<Self>) {
        if select {
            self.anchor.get_or_insert(self.cursor);
        } else {
            self.anchor = None;
        }
        self.cursor = to.min(self.text.len());
        cx.notify();
    }

    pub fn left(&mut self, select: bool, cx: &mut Context<Self>) {
        // A plain arrow out of a selection lands on its edge, as every field does.
        let to = match (select, self.selection()) {
            (false, Some(range)) => range.start,
            _ => prev_char(&self.text, self.cursor),
        };
        self.move_to(to, select, cx);
    }

    pub fn right(&mut self, select: bool, cx: &mut Context<Self>) {
        let to = match (select, self.selection()) {
            (false, Some(range)) => range.end,
            _ => next_char(&self.text, self.cursor),
        };
        self.move_to(to, select, cx);
    }

    pub fn word_left(&mut self, select: bool, cx: &mut Context<Self>) {
        self.move_to(prev_word(&self.text, self.cursor), select, cx);
    }

    pub fn word_right(&mut self, select: bool, cx: &mut Context<Self>) {
        self.move_to(next_word(&self.text, self.cursor), select, cx);
    }

    pub fn home(&mut self, select: bool, cx: &mut Context<Self>) {
        self.move_to(0, select, cx);
    }

    pub fn end(&mut self, select: bool, cx: &mut Context<Self>) {
        self.move_to(self.text.len(), select, cx);
    }

    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        self.anchor = Some(0);
        self.cursor = self.text.len();
        cx.notify();
    }

    /// Typed or pasted text at the cursor, replacing any selection.
    pub fn insert(&mut self, text: &str, cx: &mut Context<Self>) {
        self.delete_selection();
        self.text.insert_str(self.cursor, text);
        self.cursor += text.len();
        self.changed(cx);
    }

    /// Harness entry: replaces the text as one commit, no keyboard involved.
    pub fn set_text(&mut self, text: &str, cx: &mut Context<Self>) {
        self.text = text.to_owned();
        self.cursor = self.text.len();
        self.anchor = None;
        self.marked = None;
        self.changed(cx);
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.text.clear();
        self.cursor = 0;
        self.anchor = None;
        self.marked = None;
        self.changed(cx);
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        cx.emit(Changed);
        cx.notify();
    }

    /// A change only when the text differs from `before`: an IME commit
    /// that lands the same text as its preedit just drops the underline,
    /// and a search for it would put the selection back at the top after
    /// the key that committed it moved it.
    fn changed_from(&mut self, before: &str, cx: &mut Context<Self>) {
        if self.text == before {
            cx.notify();
        } else {
            self.changed(cx);
        }
    }

    fn splice(&mut self, range: Range<usize>, new_text: &str) -> usize {
        let range = range.start.min(self.text.len())..range.end.min(self.text.len());
        self.text.replace_range(range.clone(), new_text);
        range.start
    }

    /// The range an edit replaces, by the platform's order of preference.
    fn target(&self, range_utf16: Option<Range<usize>>) -> Range<usize> {
        if range_utf16.is_none()
            && self.marked.is_none()
            && let Some(selection) = self.selection()
        {
            return selection;
        }
        range_utf16
            .map(|r| from_utf16(&self.text, &r))
            .or_else(|| self.marked.clone())
            .unwrap_or(self.cursor..self.cursor)
    }
}

impl EntityInputHandler for SearchInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = from_utf16(&self.text, &range_utf16);
        actual_range.replace(to_utf16(&self.text, &range));
        Some(self.text[range].to_owned())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let range = self
            .selection()
            .map(|r| to_utf16(&self.text, &r))
            .unwrap_or_else(|| {
                let cursor = offset_to_utf16(&self.text, self.cursor);
                cursor..cursor
            });
        Some(UTF16Selection {
            range,
            reversed: self.anchor.is_some_and(|a| a > self.cursor),
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked.as_ref().map(|r| to_utf16(&self.text, r))
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.marked = None;
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let before = self.text.clone();
        let start = self.splice(self.target(range_utf16), new_text);
        self.cursor = start + new_text.len();
        self.anchor = None;
        self.marked = None;
        self.changed_from(&before, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let before = self.text.clone();
        let start = self.splice(self.target(range_utf16), new_text);
        let end = start + new_text.len();
        self.anchor = None;
        self.marked = (!new_text.is_empty()).then_some(start..end);
        self.cursor = end;
        self.changed_from(&before, cx);
    }

    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        _bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        Some(self.caret)
    }

    fn character_index_for_point(
        &mut self,
        _point: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }

    fn set_selected_text_range(
        &mut self,
        range_utf16: Range<usize>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cursor = from_utf16(&self.text, &range_utf16).end;
        cx.notify();
    }

    fn text_length_utf16(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        Some(self.text.encode_utf16().count())
    }
}

/// The start of the word before `offset`: back over spaces, then over the word.
fn prev_word(s: &str, offset: usize) -> usize {
    let before = &s[..offset];
    let trimmed = before.trim_end();
    let start = trimmed
        .rfind(char::is_whitespace)
        .map_or(0, |i| i + s[i..].chars().next().map_or(1, char::len_utf8));
    start.min(offset)
}

/// The end of the word after `offset`: over the word, then over the spaces.
fn next_word(s: &str, offset: usize) -> usize {
    let after = &s[offset..];
    let word_end = after.find(char::is_whitespace).unwrap_or(after.len());
    let rest = &after[word_end..];
    let spaces = rest.len() - rest.trim_start().len();
    offset + word_end + spaces
}

fn prev_char(s: &str, offset: usize) -> usize {
    s[..offset].char_indices().next_back().map_or(0, |(i, _)| i)
}

fn next_char(s: &str, offset: usize) -> usize {
    s[offset..]
        .chars()
        .next()
        .map_or(offset, |c| offset + c.len_utf8())
}

fn offset_to_utf16(s: &str, offset: usize) -> usize {
    s[..offset.min(s.len())].encode_utf16().count()
}

fn offset_from_utf16(s: &str, offset: usize) -> usize {
    let mut units = 0;
    for (i, c) in s.char_indices() {
        if units >= offset {
            return i;
        }
        units += c.len_utf16();
    }
    s.len()
}

fn to_utf16(s: &str, range: &Range<usize>) -> Range<usize> {
    offset_to_utf16(s, range.start)..offset_to_utf16(s, range.end)
}

fn from_utf16(s: &str, range: &Range<usize>) -> Range<usize> {
    offset_from_utf16(s, range.start)..offset_from_utf16(s, range.end)
}

/// The field itself: one shaped line, a caret, and the input handler hookup.
pub struct SearchField {
    input: Entity<SearchInput>,
    placeholder: SharedString,
}

pub fn search_field(
    input: Entity<SearchInput>,
    placeholder: impl Into<SharedString>,
) -> SearchField {
    SearchField {
        input,
        placeholder: placeholder.into(),
    }
}

pub struct FieldPrepaint {
    line: ShapedLine,
    selection: Option<PaintQuad>,
    caret: Option<PaintQuad>,
    /// The placeholder's progress in, 1 once landed or when text shows.
    slide: f32,
}

impl IntoElement for SearchField {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for SearchField {
    type RequestLayoutState = ();
    type PrepaintState = FieldPrepaint;

    fn id(&self) -> Option<gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = gpui::Style::default();
        style.size.width = gpui::relative(1.0).into();
        style.size.height = window.line_height().into();
        style.flex_grow = 1.0;
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> FieldPrepaint {
        let th = theme::get();
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line_height = window.line_height();
        let input = self.input.read(cx);
        let text = input.text.clone();
        let cursor = input.cursor;
        let marked = input.marked.clone();
        let selection = input.selection();

        let run = |len: usize, color, underline: Option<UnderlineStyle>| TextRun {
            len,
            font: style.font(),
            color,
            background_color: None,
            underline,
            strikethrough: None,
            letter_spacing: None,
        };

        // The placeholder slides in from the right behind the bar; a frame
        // is asked for until it has landed.
        let slide = if text.is_empty() && !cx.reduce_motion() {
            let elapsed = input.shown.elapsed().as_secs_f32();
            let delay = motion::secs(motion::SLIDE_DELAY_MS);
            let t = ((elapsed - delay) / (motion::secs(motion::SLIDE_MS))).clamp(0.0, 1.0);
            if t < 1.0 {
                window.request_animation_frame();
            }
            motion::curve(t)
        } else {
            1.0
        };
        let line = if text.is_empty() {
            let placeholder = self.placeholder.clone();
            let mut muted = theme::hsla_of(th.text_muted);
            muted.alpha *= slide;
            let runs = [run(placeholder.len(), muted, None)];
            window
                .text_system()
                .shape_line(placeholder, font_size, &runs, None)
        } else {
            let color = style.color;
            let runs: Vec<TextRun> = match &marked {
                Some(m) => {
                    let underline = UnderlineStyle {
                        thickness: px(1.0),
                        color: Some(theme::hsla_of(th.accent)),
                        wavy: false,
                    };
                    [
                        run(m.start, color, None),
                        run(m.len(), color, Some(underline)),
                        run(text.len() - m.end, color, None),
                    ]
                    .into_iter()
                    .filter(|r| r.len > 0)
                    .collect()
                }
                None => vec![run(text.len(), color, None)],
            };
            window
                .text_system()
                .shape_line(text.clone().into(), font_size, &runs, None)
        };

        let selection = selection.filter(|_| !text.is_empty()).map(|range| {
            let left = line.x_for_index(range.start);
            let right = line.x_for_index(range.end);
            fill(
                Bounds::new(
                    point(bounds.left() + left, bounds.top()),
                    size(right - left, line_height),
                ),
                theme::hsla_of(th.selection_fill),
            )
        });

        let caret = if input.focus_handle.is_focused(window) {
            let goal = if text.is_empty() {
                0.0
            } else {
                f32::from(line.x_for_index(cursor))
            };
            // The caret glides to its column on a `Transition` and stays
            // solid while it moves; idle, it blinks on its own frames, with
            // motion off too, as the CSS keeps it.
            let glide = Transition::new(
                input.caret_x.clone(),
                motion::transition(motion::CARET_GLIDE_MS, cx),
            )
            .with_easing(motion::curve);
            let moved = glide.update(cx, |x, _| *x = goal);
            if moved {
                self.input
                    .update(cx, |input, _| input.caret_moved = Instant::now());
            }
            let x = glide.evaluate(window, cx).round();
            let idle = self.input.read(cx).caret_moved.elapsed().as_secs_f32()
                - motion::secs(motion::CARET_SOLID_MS);
            let alpha = if idle <= 0.0 {
                1.0
            } else {
                motion::blink(idle / (motion::secs(motion::CARET_BLINK_MS)))
            };
            window.request_animation_frame();
            let caret_h = font_size * theme::CARET_HEIGHT;
            let caret_bounds = Bounds::new(
                point(
                    bounds.left() + px(x),
                    bounds.top() + (line_height - caret_h) / 2.0,
                ),
                size(px(theme::CARET_WIDTH), caret_h),
            );
            self.input.update(cx, |input, _| input.caret = caret_bounds);
            let mut colour: Hsla = theme::hsla_of(th.accent);
            colour.alpha *= alpha;
            Some(fill(caret_bounds, colour))
        } else {
            None
        };

        FieldPrepaint {
            line,
            selection,
            caret,
            slide,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        prepaint: &mut FieldPrepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        let line_height = window.line_height();
        if let Some(selection) = prepaint.selection.take() {
            window.paint_quad(selection);
        }
        let shift = motion::rise(0.0, motion::PLACEHOLDER_SHIFT, prepaint.slide);
        prepaint
            .line
            .paint(
                bounds.origin + point(px(shift), px(0.0)),
                line_height,
                TextAlign::Left,
                None,
                window,
                cx,
            )
            .ok();
        if let Some(caret) = prepaint.caret.take() {
            window.paint_quad(caret);
        }
    }
}
