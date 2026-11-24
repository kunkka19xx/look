//! The Christmas decorations over any theme: the core's snow and bokeh, a
//! faded figure behind the results, and the snowman in the search bar.

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, OnceLock};
use std::time::{Duration, Instant};

use gpui::{
    Animation, AnimationExt, App, AppContext, Bounds, ContentMask, Corners, FontWeight, GlyphId,
    Hsla, IntoElement, PaintQuad, ParentElement, Pixels, Point, RenderImage, Rgba, SharedString,
    Size, Styled, Window, canvas, div, point, px, size, svg,
};
use image::RgbaImage;
use linows_backend::look_snow::{self, CRYSTALS, bokeh};

use crate::bgimage;
use crate::commands::draw::{self, Type};
use crate::glyphs;
use crate::motion;
use crate::theme::{self, Theme};

static EPOCH: LazyLock<Instant> = LazyLock::new(Instant::now);
/// Each frame redraws the whole launcher, so the scene runs at the macOS
/// rate rather than the display's.
const FRAME_INTERVAL: Duration = Duration::from_millis(16);
static FRAME_PENDING: AtomicBool = AtomicBool::new(false);

const SNOWMAN: &str = "☃️";
/// A font family whose name carries this draws colour emoji.
const EMOJI_FAMILY_MARK: &str = "Emoji";
/// What a character no installed font draws is shaped as.
const MISSING_GLYPH: GlyphId = GlyphId(0);
/// Without a font for the crystals they fall as dots, this share of a
/// crystal's radius: a bit larger than the nearest plain flake.
const CRYSTAL_DOT_SHARE: f32 = 0.35;
/// Any size shapes the same glyphs.
const PROBE_SIZE: f32 = 16.0;

/// Glyph size as a share of the frame's height, and its bounds.
const SANTA_SIZING: (f32, f32, f32) = (0.12, 40.0, 64.0);
const FIGURE_OPACITY: f32 = 0.55;
const FIGURE_CORNER_INSET: f32 = 12.0;
/// Santa stands behind his gift, up and to the left, in glyph sizes.
const SANTA_RISE: f32 = 0.5;
const SANTA_LEAN: f32 = 0.35;
const SANTA_RED: Rgba = Rgba::new(0.86, 0.22, 0.22, 1.0);
const SANTA_SKIN: Rgba = Rgba::new(0.96, 0.78, 0.65, 1.0);
const SANTA_WHITE: Rgba = Rgba::new(0.97, 0.96, 0.94, 1.0);
const RIBBON_GOLD: Rgba = Rgba::new(0.98, 0.8, 0.36, 1.0);
/// Layers, back to front.
const SANTA: [(&str, Rgba); 3] = [
    (glyphs::SANTA_HAT, SANTA_RED),
    (glyphs::SANTA_FACE, SANTA_SKIN),
    (glyphs::SANTA_TRIM, SANTA_WHITE),
];
const GIFT: [(&str, Rgba); 2] = [
    (glyphs::GIFT_BOX, SANTA_RED),
    (glyphs::GIFT_RIBBON, RIBBON_GOLD),
];
/// The lights hold still here, wider than the core asks.
const BOKEH_SECONDS: f64 = 0.0;
const GLOW_SPREAD: f32 = 1.4;
const CHANNEL_MAX: f32 = 255.0;

/// Whether an installed family draws colour emoji; without one the glass
/// stays.
pub fn emoji_available(cx: &App) -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        cx.text_system()
            .all_font_names()
            .iter()
            .any(|family| family.contains(EMOJI_FAMILY_MARK))
    })
}

/// The snowman in the glass's box, zooming and shaking as the bar spawns.
pub fn snowman_icon() -> impl IntoElement {
    div()
        .flex_shrink_0()
        .size(px(theme::SNOWMAN_BOX))
        .flex()
        .items_center()
        .justify_center()
        .child(div().relative().child(SNOWMAN).with_animation(
            "snowman",
            Animation::new(motion::dur(motion::SNOWMAN_MS)),
            |el, t| {
                el.text_size(px(theme::SNOWMAN_SIZE * motion::snowman_zoom(t)))
                    .left(px(motion::snowman_shake(t)))
            },
        ))
}

/// Santa and his gift in the bottom right corner of a frame `frame_height`
/// tall.
pub fn santa(frame_height: f32) -> impl IntoElement {
    let (share, min, max) = SANTA_SIZING;
    let size = (frame_height * share).clamp(min, max);
    div()
        .absolute()
        .opacity(FIGURE_OPACITY)
        .right(px(FIGURE_CORNER_INSET))
        .bottom(px(FIGURE_CORNER_INSET))
        .w(px(size * (1.0 + SANTA_LEAN)))
        .h(px(size * (1.0 + SANTA_RISE)))
        .child(layered(&SANTA, size).left_0().top_0())
        .child(layered(&GIFT, size).right_0().bottom_0())
}

/// One glyph of `size` from its coloured `layers`.
fn layered(layers: &[(&'static str, Rgba)], size: f32) -> gpui::Div {
    div()
        .absolute()
        .size(px(size))
        .children(layers.iter().map(|(glyph, colour)| {
            svg()
                .path(*glyph)
                .absolute()
                .size(px(size))
                .text_color(*colour)
        }))
}

/// Snow over the whole frame: `amount` times the default, dots in `colour`,
/// crystals set in `family`.
pub fn layer(colour: Rgba, family: SharedString, amount: f32) -> impl IntoElement {
    let base = theme::hsla_of(colour);
    scene_canvas(move |bounds, seconds, window, cx| {
        let (w, h) = (bounds.size.width, bounds.size.height);
        let flakes = look_snow::frame(
            f32::from(w).into(),
            f32::from(h).into(),
            seconds,
            amount.into(),
        );
        let drawable = crystals_drawable(&family, window);
        for flake in flakes {
            let mut fill = base;
            fill.alpha *= flake.alpha;
            let centre = bounds.origin + point(px(flake.x), px(flake.y));
            match flake.crystal.and_then(|glyph| CRYSTALS.get(glyph)) {
                Some(_) if !drawable => {
                    paint_flake(centre, flake.radius * CRYSTAL_DOT_SHARE, fill, window);
                }
                Some(glyph) => {
                    let style = Type {
                        size: flake.radius * 2.0,
                        family: &family,
                        weight: FontWeight::NORMAL,
                        colour: fill,
                        spacing: 0.0,
                    };
                    draw::paint_text(window, cx, glyph, &style, centre.x.into(), centre.y.into());
                }
                None => paint_flake(centre, flake.radius, fill, window),
            }
        }
    })
}

thread_local! {
    static CRYSTAL_FONT: RefCell<Option<(SharedString, bool)>> = const { RefCell::new(None) };
}

/// Whether `family` or a fallback of it draws every crystal, shaped once
/// per family.
fn crystals_drawable(family: &SharedString, window: &Window) -> bool {
    CRYSTAL_FONT.with_borrow_mut(|cached| {
        if let Some((_, drawable)) = cached.as_ref().filter(|(of, _)| of == family) {
            return *drawable;
        }
        let style = Type {
            size: PROBE_SIZE,
            family,
            weight: FontWeight::NORMAL,
            colour: Hsla::default(),
            spacing: 0.0,
        };
        let line = draw::shape(window, &CRYSTALS.concat(), &style);
        let mut glyphs = line
            .paint_fragments
            .iter()
            .flat_map(|run| run.glyphs.iter());
        let drawable =
            glyphs.clone().next().is_some() && glyphs.all(|glyph| glyph.id != MISSING_GLYPH);
        *cached = Some((family.clone(), drawable));
        drawable
    })
}

/// This card's slice of one frame-sized bokeh field, as the background
/// picture is sliced. None unless decorations are on over a dark theme.
pub fn bokeh_layer(radius: f32, th: &Theme) -> Option<impl IntoElement> {
    let frame_size = th.bokeh().then(|| th.frame_size())?;
    let corners = Corners::all(px(th.inner_radius(radius)));
    Some(
        canvas(
            |_, _, _| {},
            move |bounds: Bounds<Pixels>, (), window, _| {
                let frame = theme::place_frame(frame_size, window.viewport_size());
                let field = bokeh_field(frame.size, window);
                let _ = window.paint_image(bounds, frame, corners, 0.0, field, 0, false);
            },
        )
        .absolute()
        .inset_0(),
    )
}

/// A frame's size in logical pixels and its lights.
type BokehField = ((u32, u32), Arc<RenderImage>);

thread_local! {
    static BOKEH_FIELD: RefCell<Option<BokehField>> = const { RefCell::new(None) };
}

/// The lights for `frame`, painted once per size.
fn bokeh_field(frame: Size<Pixels>, window: &mut Window) -> Arc<RenderImage> {
    let key = (
        f32::from(frame.width).round() as u32,
        f32::from(frame.height).round() as u32,
    );
    BOKEH_FIELD.with_borrow_mut(|cached| {
        if let Some((_, field)) = cached.as_ref().filter(|(of, _)| *of == key) {
            return field.clone();
        }
        let field = bgimage::frame(paint_bokeh(key.0, key.1));
        if let Some((_, stale)) = cached.replace((key, field.clone())) {
            let _ = window.drop_image(stale);
        }
        field
    })
}

/// BGRA with straight alpha, a logical pixel each.
fn paint_bokeh(width: u32, height: u32) -> RgbaImage {
    let mut field = RgbaImage::new(width, height);
    for glow in bokeh::bokeh(width.into(), height.into(), BOKEH_SECONDS) {
        let [x, y] = glow.center;
        let reach = glow.radius * GLOW_SPREAD;
        let span = |centre: f32, limit: u32| {
            let from = (centre - reach).floor().max(0.0) as u32;
            let to = ((centre + reach).ceil().max(0.0) as u32).min(limit);
            from..to
        };
        let [r, g, b] = glow.colour;
        for py in span(y, height) {
            for px in span(x, width) {
                let off = ((px as f32 + 0.5 - x).hypot(py as f32 + 0.5 - y) / reach).min(1.0);
                let alpha = glow.alpha * glow_falloff(off);
                let under = field.get_pixel_mut(px, py);
                let [ub, ug, ur, ua] = under.0.map(|c| f32::from(c) / CHANNEL_MAX);
                let kept = ua * (1.0 - alpha);
                let out = alpha + kept;
                if out <= 0.0 {
                    continue;
                }
                let mix = |over: f32, under: f32| (over * alpha + under * kept) / out;
                under.0 = [mix(b, ub), mix(g, ug), mix(r, ur), out]
                    .map(|c| (c * CHANNEL_MAX).round() as u8);
            }
        }
    }
    field
}

/// A glow's light at `off` its centre, 0 to 1 of its reach.
fn glow_falloff(off: f32) -> f32 {
    let inside = 1.0 - off * off;
    inside * inside
}

/// A pointer-transparent canvas over its parent, painting at the scene's
/// clock inside its bounds; one still frame with motion off.
fn scene_canvas(
    paint: impl Fn(Bounds<Pixels>, f64, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds: Bounds<Pixels>, (), window, cx| {
            let still = cx.reduce_motion();
            let seconds = if still {
                0.0
            } else {
                EPOCH.elapsed().as_secs_f64()
            };
            let mask = ContentMask {
                bounds,
                ..ContentMask::default()
            };
            window.with_content_mask(Some(mask), |window| paint(bounds, seconds, window, cx));
            if !still {
                next_frame(window, cx);
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

/// Redraws the window one interval from now, once however many layers ask.
/// A hidden window paints nothing, so the chain stops with it.
fn next_frame(window: &mut Window, cx: &mut App) {
    if FRAME_PENDING.swap(true, Ordering::AcqRel) {
        return;
    }
    let handle = window.window_handle();
    cx.spawn(async move |cx| {
        cx.background_executor().timer(FRAME_INTERVAL).await;
        FRAME_PENDING.store(false, Ordering::Release);
        let _ = cx.update_window(handle, |_, window, _| window.refresh());
    })
    .detach();
}

/// A rounded quad: cheaper than a path for hundreds of flakes.
fn paint_flake(centre: Point<Pixels>, radius: f32, fill: Hsla, window: &mut Window) {
    let radius = px(radius);
    let bounds = Bounds::new(
        centre - point(radius, radius),
        size(radius * 2.0, radius * 2.0),
    );
    let mut quad = PaintQuad::new(bounds).background(fill);
    quad.corner_radii = Corners::all(radius);
    window.paint_quad(quad);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_glow_fades_from_its_centre_to_nothing_at_its_rim() {
        assert_eq!(glow_falloff(0.0), 1.0);
        assert!(glow_falloff(0.5) < 1.0 && glow_falloff(0.5) > glow_falloff(0.9));
        assert_eq!(glow_falloff(1.0), 0.0);
    }

    #[test]
    fn the_field_is_lit_and_stays_see_through() {
        let field = paint_bokeh(840, 560);
        assert!(field.pixels().any(|p| p.0[3] > 0));
        assert!(field.pixels().any(|p| p.0[3] == 0));
    }
}
