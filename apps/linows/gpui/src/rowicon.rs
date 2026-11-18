//! One renderer for list icons and preview headers, including decode failures.

use std::sync::Arc;

use gpui::{
    AnyElement, App, Entity, ObjectFit, RenderImage, Rgba, SharedString, div, img, prelude::*, px,
    svg,
};

use crate::icons::{IconRequest, IconStore};
use crate::imagecache::PreviewImages;
use crate::rows::{Icon, Row};

/// Emoji fills most of its tile while leaving room for its font's bearings.
const TEXT_ICON_SCALE: f32 = 0.8;

#[derive(Clone, Copy)]
pub struct IconStyle {
    pub size: f32,
    pub radius: f32,
    pub glyph_size: f32,
    pub color: Rgba,
}

pub fn request(row: &Row, size: f32) -> IconRequest {
    IconRequest {
        kind: row.icon_kind().to_string(),
        path: row.path.clone(),
        id: Some(row.id.clone()),
        size,
    }
}

pub fn picture(
    icon: &Icon,
    request: impl FnOnce() -> IconRequest,
    style: IconStyle,
    images: &Entity<PreviewImages>,
    icons: &Entity<IconStore>,
    cx: &mut App,
) -> AnyElement {
    let resolved = if needs_resolution(icon) {
        icons.update(cx, |store, cx| store.get(request(), cx))
    } else {
        None
    };
    draw(icon, style, images, resolved)
}

fn needs_resolution(icon: &Icon) -> bool {
    match icon {
        Icon::Resolve { .. } => true,
        Icon::File { fallback, .. } => needs_resolution(fallback),
        Icon::Glyph(_) | Icon::Text(_) => false,
    }
}

fn draw(
    icon: &Icon,
    style: IconStyle,
    images: &Entity<PreviewImages>,
    resolved: Option<Arc<RenderImage>>,
) -> AnyElement {
    match (icon, resolved) {
        (Icon::Resolve { .. }, Some(image)) => img(image).size(px(style.size)).into_any_element(),
        (Icon::Resolve { glyph } | Icon::Glyph(glyph), _) => svg()
            .path(*glyph)
            .size(px(style.glyph_size))
            .text_color(style.color)
            .into_any_element(),
        (Icon::File { path, fallback }, resolved) => {
            let fallback = fallback.clone();
            let fallback_images = images.clone();
            img(path.clone())
                .image_cache(images)
                .size(px(style.size))
                .rounded(px(style.radius))
                .object_fit(ObjectFit::Contain)
                .with_fallback(move || {
                    div()
                        .size(px(style.size))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(draw(&fallback, style, &fallback_images, resolved.clone()))
                        .into_any_element()
                })
                .into_any_element()
        }
        (Icon::Text(text), _) => div()
            .text_size(px(style.size * TEXT_ICON_SCALE))
            .child(SharedString::from(text.clone()))
            .into_any_element(),
    }
}
