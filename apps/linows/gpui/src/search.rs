//! Single-line query field. The entity owns the text and speaks
//! EntityInputHandler, which is how Wayland text-input-v3 (fcitx5) reaches it:
//! preedit arrives as marked text, a commit as a plain replace.

use std::ops::Range;

use gpui::{
    App, Bounds, Context, Element, ElementInputHandler, Entity, EntityInputHandler, EventEmitter,
    FocusHandle, GlobalElementId, InspectorElementId, IntoElement, LayoutId, PaintQuad, Pixels,
    ShapedLine, SharedString, TextAlign, TextRun, UTF16Selection, UnderlineStyle, Window, fill,
    point, px, size,
};

use crate::theme;

pub struct Changed;

pub struct SearchInput {
    pub focus_handle: FocusHandle,
    text: String,
    cursor: usize,
    marked: Option<Range<usize>>,
    caret: Bounds<Pixels>,
}

impl EventEmitter<Changed> for SearchInput {}

impl SearchInput {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            text: String::new(),
            cursor: 0,
            marked: None,
            caret: Bounds::default(),
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// What a search should see: the committed text without any preedit.
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

    pub fn is_composing(&self) -> bool {
        self.marked.is_some()
    }

    pub fn backspace(&mut self, cx: &mut Context<Self>) {
        if self.cursor == 0 {
            return;
        }
        let prev = prev_char(&self.text, self.cursor);
        self.text.replace_range(prev..self.cursor, "");
        self.cursor = prev;
        self.changed(cx);
    }

    pub fn delete(&mut self, cx: &mut Context<Self>) {
        if self.cursor >= self.text.len() {
            return;
        }
        let next = next_char(&self.text, self.cursor);
        self.text.replace_range(self.cursor..next, "");
        self.changed(cx);
    }

    pub fn left(&mut self, cx: &mut Context<Self>) {
        self.cursor = prev_char(&self.text, self.cursor);
        cx.notify();
    }

    pub fn right(&mut self, cx: &mut Context<Self>) {
        self.cursor = next_char(&self.text, self.cursor);
        cx.notify();
    }

    pub fn home(&mut self, cx: &mut Context<Self>) {
        self.cursor = 0;
        cx.notify();
    }

    pub fn end(&mut self, cx: &mut Context<Self>) {
        self.cursor = self.text.len();
        cx.notify();
    }

    /// Harness entry: replaces the text as one commit, no keyboard involved.
    pub fn set_text(&mut self, text: &str, cx: &mut Context<Self>) {
        self.text = text.to_owned();
        self.cursor = self.text.len();
        self.marked = None;
        self.changed(cx);
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.text.clear();
        self.cursor = 0;
        self.marked = None;
        self.changed(cx);
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        cx.emit(Changed);
        cx.notify();
    }

    fn splice(&mut self, range: Range<usize>, new_text: &str) -> usize {
        let range = range.start.min(self.text.len())..range.end.min(self.text.len());
        self.text.replace_range(range.clone(), new_text);
        range.start
    }

    /// The range an edit replaces, by the platform's order of preference.
    fn target(&self, range_utf16: Option<Range<usize>>) -> Range<usize> {
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
        let cursor = offset_to_utf16(&self.text, self.cursor);
        Some(UTF16Selection {
            range: cursor..cursor,
            reversed: false,
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
        let start = self.splice(self.target(range_utf16), new_text);
        self.cursor = start + new_text.len();
        self.marked = None;
        self.changed(cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let start = self.splice(self.target(range_utf16), new_text);
        let end = start + new_text.len();
        self.marked = (!new_text.is_empty()).then_some(start..end);
        self.cursor = end;
        self.changed(cx);
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
    caret: Option<PaintQuad>,
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

        let run = |len: usize, color, underline: Option<UnderlineStyle>| TextRun {
            len,
            font: style.font(),
            color,
            background_color: None,
            underline,
            strikethrough: None,
            letter_spacing: None,
        };

        let line = if text.is_empty() {
            let placeholder = self.placeholder.clone();
            let runs = [run(placeholder.len(), theme::hsla_of(th.text_muted), None)];
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

        let caret = if input.focus_handle.is_focused(window) {
            let x = if text.is_empty() {
                px(0.0)
            } else {
                line.x_for_index(cursor)
            };
            let caret_h = font_size * theme::CARET_HEIGHT;
            let caret_bounds = Bounds::new(
                point(
                    bounds.left() + x,
                    bounds.top() + (line_height - caret_h) / 2.0,
                ),
                size(px(theme::CARET_WIDTH), caret_h),
            );
            self.input.update(cx, |input, _| input.caret = caret_bounds);
            Some(fill(caret_bounds, theme::hsla_of(th.accent)))
        } else {
            None
        };

        FieldPrepaint { line, caret }
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
        prepaint
            .line
            .paint(
                bounds.origin,
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
