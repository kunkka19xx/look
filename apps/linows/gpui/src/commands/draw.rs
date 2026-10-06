//! Canvas strokes the panels share: circles, dots and centred text.

use gpui::{
    App, FontWeight, Hsla, PathBuilder, Pixels, Point, SharedString, TextAlign, TextRun, Window,
    point, px,
};

/// Line height of canvas text, the webview's SVG default.
pub const LINE_HEIGHT: f32 = 1.2;

/// A circle of radius `r` as two half arcs, from the top.
pub fn circle_path(builder: &mut PathBuilder, centre: Point<Pixels>, r: f32) {
    let (cx, cy) = (f32::from(centre.x), f32::from(centre.y));
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

pub fn paint_circle(window: &mut Window, centre: Point<Pixels>, r: f32, width: f32, colour: Hsla) {
    let mut builder = PathBuilder::stroke(px(width));
    circle_path(&mut builder, centre, r);
    if let Ok(path) = builder.build() {
        window.paint_path(path, colour);
    }
}

/// `dash` is the SVG dash array: dash length, gap length.
pub fn paint_dashed_circle(
    window: &mut Window,
    centre: Point<Pixels>,
    r: f32,
    width: f32,
    dash: &[f32],
    colour: Hsla,
) {
    let dashes: Vec<Pixels> = dash.iter().map(|d| px(*d)).collect();
    let mut builder = PathBuilder::stroke(px(width)).dash_array(&dashes);
    circle_path(&mut builder, centre, r);
    if let Ok(path) = builder.build() {
        window.paint_path(path, colour);
    }
}

pub fn paint_dot(window: &mut Window, centre: Point<Pixels>, r: f32, colour: Hsla) {
    let mut builder = PathBuilder::fill();
    circle_path(&mut builder, centre, r);
    if let Ok(path) = builder.build() {
        window.paint_path(path, colour);
    }
}

/// How a line of canvas text is set.
pub struct Type<'a> {
    pub size: f32,
    pub family: &'a str,
    pub weight: FontWeight,
    pub colour: Hsla,
    /// Tracking in pixels, 0 for the font's own.
    pub spacing: f32,
}

/// `text` centred on (`centre_x`, `centre_y`).
pub fn paint_text(
    window: &mut Window,
    cx: &mut App,
    text: &str,
    style: &Type,
    centre_x: f32,
    centre_y: f32,
) {
    let mut font = window.text_style().font();
    font.family = style.family.to_string().into();
    font.weight = style.weight;
    let run = TextRun {
        len: text.len(),
        font,
        color: style.colour,
        background_color: None,
        underline: None,
        strikethrough: None,
        letter_spacing: (style.spacing != 0.0).then(|| px(style.spacing)),
    };
    let line = window.text_system().shape_line(
        SharedString::from(text.to_string()),
        px(style.size),
        &[run],
        None,
    );
    let line_height = px(style.size * LINE_HEIGHT);
    let origin = point(
        px(centre_x) - line.width() / 2.0,
        px(centre_y) - line_height / 2.0,
    );
    let _ = line.paint(origin, line_height, TextAlign::Left, None, window, cx);
}
