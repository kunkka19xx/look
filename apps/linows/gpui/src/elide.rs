//! One line of text that keeps both ends when it does not fit, the way
//! SwiftUI's `.truncationMode(.middle)` shows a path: the root and the
//! file name survive, the directories between give way to an ellipsis.

use gpui::{
    App, Bounds, Element, GlobalElementId, InspectorElementId, IntoElement, LayoutId, Pixels,
    ShapedLine, SharedString, TextRun, Window, relative,
};

const ELLIPSIS: char = '\u{2026}';

pub struct MiddleElided {
    text: SharedString,
}

pub fn middle_elided(text: impl Into<SharedString>) -> MiddleElided {
    MiddleElided { text: text.into() }
}

impl IntoElement for MiddleElided {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for MiddleElided {
    type RequestLayoutState = ();
    type PrepaintState = ShapedLine;

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
        style.size.width = relative(1.0).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        _cx: &mut App,
    ) -> ShapedLine {
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let shape = |text: SharedString| {
            let run = TextRun {
                len: text.len(),
                font: style.font(),
                color: style.color,
                background_color: None,
                underline: None,
                strikethrough: None,
                letter_spacing: None,
            };
            window
                .text_system()
                .shape_line(text, font_size, &[run], None)
        };
        let whole = shape(self.text.clone());
        if whole.width <= bounds.size.width {
            return whole;
        }
        // The most characters that fit around the ellipsis, split evenly;
        // each probe is one shaping, and the search is a handful of them.
        let chars: Vec<char> = self.text.chars().collect();
        let fits = |keep: usize| -> Option<ShapedLine> {
            let head = keep - keep / 2;
            let tail = keep / 2;
            let text: String = chars[..head]
                .iter()
                .chain(std::iter::once(&ELLIPSIS))
                .chain(chars[chars.len() - tail..].iter())
                .collect();
            let line = shape(text.into());
            (line.width <= bounds.size.width).then_some(line)
        };
        let (mut low, mut high) = (0, chars.len());
        let mut best = fits(0);
        while low < high {
            let mid = (low + high).div_ceil(2);
            match fits(mid) {
                Some(line) => {
                    best = Some(line);
                    low = mid;
                }
                None => high = mid - 1,
            }
        }
        best.unwrap_or(whole)
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        line: &mut ShapedLine,
        window: &mut Window,
        cx: &mut App,
    ) {
        let line_height = window.line_height();
        let align = window.text_style().text_align;
        line.paint(
            bounds.origin,
            line_height,
            align,
            Some(bounds.size.width),
            window,
            cx,
        )
        .ok();
    }
}
