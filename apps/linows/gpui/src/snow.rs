//! The Christmas decorations over any theme: the core's snow and bokeh, a
//! faded figure behind the results, and the snowman in the search bar.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, OnceLock};
use std::time::{Duration, Instant};

use gpui::{
    Animation, AnimationExt, AnyElement, App, AppContext, Bounds, BoxShadow, ContentMask, Corners,
    FontWeight, Hsla, IntoElement, PaintQuad, ParentElement, Pixels, Point, Rgba, SharedString,
    Styled, Window, canvas, div, point, px, size,
};
use linows_backend::look_snow::{self, CRYSTALS, bokeh};

use crate::commands::draw::{self, Type};
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

/// Glyph size as a share of the frame's height, and its bounds.
const TREE_SIZING: (f32, f32, f32) = (0.24, 96.0, 170.0);
const SANTA_SIZING: (f32, f32, f32) = (0.12, 40.0, 64.0);
const FIGURE_OPACITY: f32 = 0.55;
const FIGURE_CORNER_INSET: f32 = 12.0;
/// Santa stands behind his gift, up and to the left, in glyph sizes.
const SANTA_RISE: f32 = 0.5;
const SANTA_LEAN: f32 = 0.35;
/// Of a glow's radius, the lit disc and the blur around it.
const GLOW_CORE_SHARE: f32 = 0.55;
const GLOW_BLUR_SHARE: f32 = 0.9;

#[derive(Clone, Copy)]
pub enum Figure {
    Tree,
    Santa,
}

/// Whether an installed family draws colour emoji; without one the glass
/// stays and no figure stands.
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

/// `figure` at the bottom of a frame `frame_height` tall.
pub fn figure(figure: Figure, frame_height: f32) -> AnyElement {
    let sized = |(share, min, max): (f32, f32, f32)| (frame_height * share).clamp(min, max);
    let el = div().absolute().bottom_0().opacity(FIGURE_OPACITY);
    match figure {
        Figure::Tree => {
            let size = sized(TREE_SIZING);
            el.left_0()
                .right_0()
                .flex()
                .justify_center()
                .text_size(px(size))
                .line_height(px(size))
                .child("🎄")
        }
        Figure::Santa => {
            let size = sized(SANTA_SIZING);
            el.right(px(FIGURE_CORNER_INSET))
                .w(px(size * (1.0 + SANTA_LEAN)))
                .h(px(size * (1.0 + SANTA_RISE)))
                .text_size(px(size))
                .line_height(px(size))
                .child(div().absolute().left_0().top_0().child("🎅"))
                .child(div().absolute().right_0().bottom_0().child("🎁"))
        }
    }
    .into_any_element()
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
        for flake in flakes {
            let mut fill = base;
            fill.alpha *= flake.alpha;
            let centre = bounds.origin + point(px(flake.x), px(flake.y));
            match flake.crystal.and_then(|glyph| CRYSTALS.get(glyph)) {
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

/// This card's slice of one frame-sized bokeh field, as the background
/// picture is sliced. None unless decorations are on over a dark theme.
pub fn bokeh_layer(th: &Theme) -> Option<impl IntoElement> {
    let frame_size = th.bokeh().then(|| th.frame_size())?;
    Some(scene_canvas(move |bounds, seconds, window, _| {
        let frame = theme::place_frame(frame_size, window.viewport_size());
        let (w, h) = (frame.size.width, frame.size.height);
        for glow in bokeh::bokeh(f32::from(w).into(), f32::from(h).into(), seconds) {
            paint_glow(frame.origin, &glow, bounds, window);
        }
    }))
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

/// A glow as a lit disc's blurred shadow, gpui having no radial gradient.
/// Skipped when it misses `visible`, as most do for any one card.
fn paint_glow(
    origin: Point<Pixels>,
    glow: &bokeh::Glow,
    visible: Bounds<Pixels>,
    window: &mut Window,
) {
    let centre = origin + point(px(glow.center[0]), px(glow.center[1]));
    let reach = px(glow.radius);
    let extent = Bounds::new(centre - point(reach, reach), size(reach * 2.0, reach * 2.0));
    if !extent.intersects(&visible) {
        return;
    }
    let [r, g, b] = glow.colour;
    let core = px(glow.radius * GLOW_CORE_SHARE);
    let shadow = BoxShadow {
        color: theme::hsla_of(Rgba::new(r, g, b, glow.alpha)).into(),
        offset: point(px(0.0), px(0.0)),
        blur_radius: px(glow.radius * GLOW_BLUR_SHARE),
        spread_radius: px(0.0),
        inset: false,
    };
    let disc = Bounds::new(centre - point(core, core), size(core * 2.0, core * 2.0));
    window.paint_drop_shadows(disc, Corners::all(core), 0.0, &[shadow]);
}
