//! The background picture (`ui_bg_image`): one image laid over the window
//! the way the webview's `.launcher-window::before` is, and every card
//! shows its own slice of it, so the floating tiles read as one picture
//! cut apart by the gaps. The webview computed the slices per tile from a
//! ResizeObserver; here `paint_image` takes the card as the clip and the
//! whole picture as the placement.
//!
//! The blur is done once, off the UI thread, into a cached bitmap: gpui has
//! no image filter, and a filter run every frame was the webview's cost.
//! The picture is first scaled to the size it is drawn at, so the blur
//! runs over the pixels that show. Opacity is the element's, applied by
//! the card's layer.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use gpui::{
    Bounds, Corners, Pixels, RenderImage, Size, Window, canvas, div, point, prelude::*, px, size,
};
use image::imageops::FilterType;

use crate::theme::{BgLayout, Theme};

/// Blur steps on the slider, so a drag does not rebuild the bitmap per
/// hundredth of a pixel.
const BLUR_STEPS_PER_PX: f32 = 10.0;

/// Everything the bitmap depends on.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Key {
    path: PathBuf,
    layout: BgLayout,
    blur_steps: u32,
    /// The window, in logical pixels, rounded.
    viewport: (u32, u32),
    /// Device pixels per logical pixel, in hundredths.
    scale: u32,
}

impl Key {
    /// `None` when the theme has no picture.
    pub fn of(th: &Theme, window: &Window) -> Option<Self> {
        let path = th.bg_image.clone()?;
        let viewport = window.viewport_size();
        Some(Self {
            path,
            layout: th.bg_layout,
            blur_steps: (th.bg_blur.max(0.0) * BLUR_STEPS_PER_PX).round() as u32,
            viewport: (
                f32::from(viewport.width).round() as u32,
                f32::from(viewport.height).round() as u32,
            ),
            scale: (window.scale_factor() * 100.0).round() as u32,
        })
    }

    fn blur_px(&self) -> f32 {
        self.blur_steps as f32 / BLUR_STEPS_PER_PX
    }

    fn scale_factor(&self) -> f32 {
        self.scale as f32 / 100.0
    }

    fn viewport(&self) -> Size<Pixels> {
        size(px(self.viewport.0 as f32), px(self.viewport.1 as f32))
    }
}

/// A decoded, blurred picture and the size it is drawn at when the layout
/// keeps its natural size.
pub struct Backdrop {
    key: Key,
    image: Arc<RenderImage>,
    natural: Size<Pixels>,
}

static CACHE: Mutex<Option<Arc<Backdrop>>> = Mutex::new(None);

fn cache() -> std::sync::MutexGuard<'static, Option<Arc<Backdrop>>> {
    CACHE.lock().unwrap_or_else(|p| p.into_inner())
}

/// The cached picture for `key`. While the exact one is still being built,
/// the last one for the same file stands in, so a slider drag does not
/// blank the cards between steps.
pub fn ready(key: &Key) -> Option<Arc<Backdrop>> {
    cache().as_ref().filter(|b| b.key.path == key.path).cloned()
}

pub fn is_exact(key: &Key) -> bool {
    cache().as_ref().is_some_and(|b| b.key == *key)
}

pub fn store(backdrop: Backdrop) {
    *cache() = Some(Arc::new(backdrop));
}

/// Decode, scale to the drawn size, blur. Blocking: run it on the
/// background executor.
pub fn load(key: Key) -> Result<Backdrop, String> {
    let decoded = image::ImageReader::open(&key.path)
        .and_then(|reader| reader.with_guessed_format())
        .map_err(|err| err.to_string())?
        .decode()
        .map_err(|err| err.to_string())?;
    let natural = size(px(decoded.width() as f32), px(decoded.height() as f32));
    let mut rgba = decoded.into_rgba8();

    let drawn = drawn_rect(key.layout, key.viewport(), natural);
    let device = |v: Pixels| (f32::from(v) * key.scale_factor()).round().max(1.0) as u32;
    let (target_w, target_h) = match key.layout {
        BgLayout::Fill | BgLayout::Stretch => (device(drawn.size.width), device(drawn.size.height)),
        BgLayout::Center | BgLayout::Duplicate => (device(natural.width), device(natural.height)),
    };
    if rgba.width() > target_w || rgba.height() > target_h {
        rgba = image::imageops::resize(&rgba, target_w, target_h, FilterType::Triangle);
    }
    // The slider's pixels are logical and on the drawn picture; the bitmap
    // may be a different size.
    let sigma = key.blur_px() * rgba.width() as f32 / f32::from(drawn.size.width).max(1.0);
    if sigma > 0.0 {
        rgba = image::imageops::blur(&rgba, sigma);
    }
    // gpui samples BGRA.
    for pixel in rgba.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    Ok(Backdrop {
        image: Arc::new(RenderImage::new(vec![image::Frame::new(rgba)])),
        natural,
        key,
    })
}

/// Where the picture goes in window coordinates: centred, scaled by the
/// layout. The webview's `drawnImageRect`.
fn drawn_rect(layout: BgLayout, viewport: Size<Pixels>, natural: Size<Pixels>) -> Bounds<Pixels> {
    let (w, h) = match layout {
        BgLayout::Stretch => (viewport.width, viewport.height),
        BgLayout::Fill => {
            let scale = (viewport.width / natural.width).max(viewport.height / natural.height);
            (natural.width * scale, natural.height * scale)
        }
        BgLayout::Center | BgLayout::Duplicate => (natural.width, natural.height),
    };
    Bounds::new(
        point((viewport.width - w) / 2.0, (viewport.height - h) / 2.0),
        size(w, h),
    )
}

/// The card's layer: its own slice of the picture, under its content.
/// `radius` is the card's outer corner; the layer sits inside the border,
/// so its own corner is that much tighter, or the corner shows a gap.
pub fn layer(backdrop: Arc<Backdrop>, radius: f32, th: &Theme) -> impl IntoElement {
    let opacity = th.bg_opacity.clamp(0.0, 1.0);
    let radius = (radius - th.border_thickness).max(0.0);
    div().absolute().inset_0().opacity(opacity).child(
        canvas(
            |_, _, _| (),
            move |bounds: Bounds<Pixels>, _, window, _| {
                let viewport = window.viewport_size();
                let drawn = drawn_rect(backdrop.key.layout, viewport, backdrop.natural);
                let corners = Corners::all(px(radius));
                let mut paint = |placement: Bounds<Pixels>| {
                    let _ = window.paint_image(
                        bounds,
                        placement,
                        corners,
                        backdrop.image.clone(),
                        0,
                        false,
                    );
                };
                if backdrop.key.layout != BgLayout::Duplicate {
                    paint(drawn);
                    return;
                }
                // Tiles anchored on the centred one, enough to cover the card.
                for placement in tiles(drawn, bounds) {
                    paint(placement);
                }
            },
        )
        .size_full(),
    )
}

/// Copies of `tile` on its own grid that touch `area`.
fn tiles(tile: Bounds<Pixels>, area: Bounds<Pixels>) -> Vec<Bounds<Pixels>> {
    let (w, h) = (tile.size.width, tile.size.height);
    if w <= px(0.0) || h <= px(0.0) {
        return Vec::new();
    }
    let first = |origin: Pixels, step: Pixels, edge: Pixels| {
        origin - step * ((origin - edge) / step).ceil()
    };
    let x0 = first(tile.origin.x, w, area.origin.x);
    let y0 = first(tile.origin.y, h, area.origin.y);
    let mut out = Vec::new();
    let mut y = y0;
    while y < area.origin.y + area.size.height {
        let mut x = x0;
        while x < area.origin.x + area.size.width {
            out.push(Bounds::new(point(x, y), tile.size));
            x += w;
        }
        y += h;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIEW: Size<Pixels> = Size {
        width: px(1000.0),
        height: px(600.0),
    };
    const WIDE: Size<Pixels> = Size {
        width: px(2000.0),
        height: px(500.0),
    };

    #[test]
    fn fill_covers_and_centres() {
        let r = drawn_rect(BgLayout::Fill, VIEW, WIDE);
        assert_eq!(r.size.height, px(600.0));
        assert_eq!(r.size.width, px(2400.0));
        assert_eq!(r.origin.x, px(-700.0));
        assert_eq!(r.origin.y, px(0.0));
    }

    #[test]
    fn stretch_is_the_viewport_and_center_keeps_size() {
        assert_eq!(drawn_rect(BgLayout::Stretch, VIEW, WIDE).size, VIEW);
        let r = drawn_rect(BgLayout::Center, VIEW, WIDE);
        assert_eq!(r.size, WIDE);
        assert_eq!(r.origin, point(px(-500.0), px(50.0)));
    }

    #[test]
    fn tiles_cover_the_area_from_the_anchor() {
        let tile = Bounds::new(point(px(10.0), px(10.0)), size(px(100.0), px(100.0)));
        let area = Bounds::new(point(px(0.0), px(0.0)), size(px(250.0), px(50.0)));
        let got = tiles(tile, area);
        // Four columns from -90 to 210, two rows from -90 to 10.
        assert_eq!(got.len(), 8);
        assert_eq!(got[0].origin, point(px(-90.0), px(-90.0)));
        assert_eq!(got[3].origin, point(px(210.0), px(-90.0)));
        assert_eq!(got[4].origin, point(px(-90.0), px(10.0)));
    }
}
