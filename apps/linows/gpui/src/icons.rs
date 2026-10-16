//! Row icons, resolved by the backend and drawn off the UI thread at the
//! device pixels they fill: a vector icon stays sharp at any scale, and a
//! large bitmap costs no more than its slot. Drawn icons live for the
//! process in a gpui global, one copy per picture and size, so a summon
//! paints them in its first frame; at slot size a few hundred come to a few
//! MB. The list draws a glyph until the picture lands, and keeps the glyph
//! on a miss.

use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasher, RandomState};
use std::sync::Arc;

use base64::Engine;
use gpui::{App, Context, Global, Image, ImageFormat, RenderImage, WeakEntity};
use image::RgbaImage;
use image::imageops::FilterType;
use linows_backend::platform;
use resvg::{tiny_skia, usvg};

use crate::bgimage;
use crate::icon_cache;

/// A request at its device size.
type Key = (String, u32);

const DRAWN_ENTRIES: usize = 1024;

pub struct IconStore {
    /// The window's, so a size in points becomes the pixels it covers.
    scale: f32,
}

pub struct IconRequest {
    pub kind: String,
    pub path: String,
    pub id: Option<String>,
    /// The side the picture is drawn at, in points.
    pub size: f32,
}

/// Every icon drawn so far, across summons.
#[derive(Default)]
struct Drawn {
    /// What each request drew, `None` on a miss.
    requests: HashMap<Key, Option<Arc<RenderImage>>>,
    /// One copy per picture and size: every file of a type shares the
    /// type's icon.
    pictures: HashMap<(u64, u32), Arc<RenderImage>>,
    pending: HashSet<Key>,
    /// Stores that asked for a pending key, told when it lands: the one that
    /// started it may be gone with a hidden launcher.
    waiting: HashMap<Key, Vec<WeakEntity<IconStore>>>,
    hasher: RandomState,
}

impl Global for Drawn {}

impl IconStore {
    pub fn new(scale: f32) -> Self {
        Self { scale }
    }

    pub fn set_scale(&mut self, scale: f32) {
        self.scale = scale;
    }

    /// The picture if it is drawn; otherwise starts drawing it and notifies
    /// once it is, so the row re-renders with it.
    pub fn get(
        &mut self,
        request: IconRequest,
        cx: &mut Context<Self>,
    ) -> Option<Arc<RenderImage>> {
        let side = (request.size * self.scale).ceil().max(1.0) as u32;
        let key = (platform::icon_key(&request.kind, &request.path), side);
        let me = cx.weak_entity();
        let drawn = cx.default_global::<Drawn>();
        if let Some(known) = drawn.requests.get(&key) {
            return known.clone();
        }
        let waiting = drawn.waiting.entry(key.clone()).or_default();
        if !waiting
            .iter()
            .any(|store| store.entity_id() == me.entity_id())
        {
            waiting.push(me);
        }
        if !drawn.pending.insert(key.clone()) {
            return None;
        }
        let hasher = drawn.hasher.clone();
        let renderer = cx.svg_renderer();
        cx.spawn(async move |_, cx| {
            let picture = cx
                .background_executor()
                .spawn(async move {
                    let url = platform::get_icon(
                        icon_cache(),
                        &request.kind,
                        &request.path,
                        request.id.as_deref(),
                    )
                    .data_url?;
                    let image = draw(&url, side, renderer)?;
                    Some((hasher.hash_one(&url), image))
                })
                .await;
            // Kept even if this launcher hid meanwhile: the next one wants it.
            for store in cx.update(|cx| land(key, picture, cx)) {
                let _ = store.update(cx, |_, cx| cx.notify());
            }
        })
        .detach();
        None
    }
}

/// Stores the picture and returns the stores waiting for it.
fn land(
    key: Key,
    picture: Option<(u64, Arc<RenderImage>)>,
    cx: &mut App,
) -> Vec<WeakEntity<IconStore>> {
    let drawn = cx.default_global::<Drawn>();
    drawn.pending.remove(&key);
    let waiting = drawn.waiting.remove(&key).unwrap_or_default();
    let stale: Vec<_> = if drawn.requests.len() >= DRAWN_ENTRIES {
        drawn.requests.clear();
        drawn.pictures.drain().map(|(_, image)| image).collect()
    } else {
        Vec::new()
    };
    let image = picture.map(|(source, image)| {
        drawn
            .pictures
            .entry((source, key.1))
            .or_insert(image)
            .clone()
    });
    drawn.requests.insert(key, image);
    for image in stale {
        cx.drop_image(image, None);
    }
    waiting
}

/// The picture in `url` fitted to a `side`-pixel square, its aspect kept. A
/// bitmap is only ever shrunk; a GIF keeps its frames at their own size.
fn draw(url: &str, side: u32, renderer: gpui::SvgRenderer) -> Option<Arc<RenderImage>> {
    let image = decode_data_url(url)?;
    let mut rgba = match image.format {
        ImageFormat::Svg => draw_svg(&image.bytes, side)?,
        ImageFormat::Gif => return image.to_image_data(renderer).ok(),
        format => {
            let decoded =
                image::load_from_memory_with_format(&image.bytes, raster(format)?).ok()?;
            if decoded.width() > side || decoded.height() > side {
                decoded.resize(side, side, FilterType::CatmullRom)
            } else {
                decoded
            }
            .into_rgba8()
        }
    };
    bgimage::to_bgra(&mut rgba);
    Some(bgimage::frame(rgba))
}

fn raster(format: ImageFormat) -> Option<image::ImageFormat> {
    Some(match format {
        ImageFormat::Png => image::ImageFormat::Png,
        ImageFormat::Jpeg => image::ImageFormat::Jpeg,
        ImageFormat::Webp => image::ImageFormat::WebP,
        ImageFormat::Bmp => image::ImageFormat::Bmp,
        _ => return None,
    })
}

/// Drawn straight at `side` rather than gpui's twice-natural size, which
/// is 1024 px for Firefox's icon. Without a font database, text in an icon
/// is left out; icons ship their lettering as paths.
fn draw_svg(bytes: &[u8], side: u32) -> Option<RgbaImage> {
    let tree = usvg::Tree::from_data(bytes, &usvg::Options::default()).ok()?;
    let natural = tree.size();
    let scale = side as f32 / natural.width().max(natural.height());
    let fit = |v: f32| (v * scale).round().max(1.0) as u32;
    let (w, h) = (fit(natural.width()), fit(natural.height()));
    let mut pixmap = tiny_skia::Pixmap::new(w, h)?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    // tiny-skia keeps alpha premultiplied; gpui samples it straight.
    let straight = pixmap
        .pixels()
        .iter()
        .flat_map(|pixel| {
            let c = pixel.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect();
    RgbaImage::from_raw(w, h, straight)
}

/// `data:<mime>;base64,<payload>` into the bytes the image cache decodes.
pub fn decode_data_url(url: &str) -> Option<Image> {
    let rest = url.strip_prefix("data:")?;
    let (mime, payload) = rest.split_once(";base64,")?;
    let format = match mime {
        "image/png" => ImageFormat::Png,
        "image/jpeg" => ImageFormat::Jpeg,
        "image/gif" => ImageFormat::Gif,
        "image/webp" => ImageFormat::Webp,
        "image/svg+xml" => ImageFormat::Svg,
        "image/bmp" => ImageFormat::Bmp,
        _ => return None,
    };
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload)
        .ok()?;
    Some(Image::from_bytes(format, bytes))
}
