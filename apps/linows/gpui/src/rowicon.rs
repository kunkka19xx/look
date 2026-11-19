//! One renderer for list icons and preview headers, including decode failures.

use gpui::{
    AnyElement, App, Entity, ObjectFit, Rgba, SharedString, Window, div, img, prelude::*, px, svg,
};

use crate::icons::{IconRequest, IconStore};
use crate::imagecache::PreviewImages;
use crate::rows::{Icon, Row};

#[derive(Clone, Copy)]
pub struct IconStyle {
    pub size: f32,
    pub radius: f32,
    /// A picture on disk fills the tile and loses its overflow, a thumbnail;
    /// otherwise it is shown whole.
    pub crop: bool,
    pub glyph_size: f32,
    pub text_size: f32,
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
    match icon {
        Icon::Resolve { glyph } => match icons.update(cx, |store, cx| store.get(request(), cx)) {
            Some(image) => img(image).size(px(style.size)).into_any_element(),
            None => glyph_icon(glyph, style),
        },
        Icon::Glyph(glyph) => glyph_icon(glyph, style),
        Icon::File { path, fallback } => {
            let fallback = Fallback {
                icon: fallback.as_ref().clone(),
                request: request(),
                style,
                images: images.clone(),
                icons: icons.clone(),
            };
            img(path.clone())
                .image_cache(images)
                .size(px(style.size))
                .rounded(px(style.radius))
                .object_fit(if style.crop {
                    ObjectFit::Cover
                } else {
                    ObjectFit::Contain
                })
                .with_fallback(move || fallback.clone().into_any_element())
                .into_any_element()
        }
        Icon::Text(text) => div()
            .text_size(px(style.text_size))
            .child(SharedString::from(text.clone()))
            .into_any_element(),
    }
}

fn glyph_icon(glyph: &'static str, style: IconStyle) -> AnyElement {
    svg()
        .path(glyph)
        .size(px(style.glyph_size))
        .text_color(style.color)
        .into_any_element()
}

/// What stands in for a picture that did not load. It is built only then, so
/// a row whose picture loads never asks the icon store for the one behind it.
#[derive(Clone, IntoElement)]
struct Fallback {
    icon: Icon,
    request: IconRequest,
    style: IconStyle,
    images: Entity<PreviewImages>,
    icons: Entity<IconStore>,
}

impl RenderOnce for Fallback {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Self {
            icon,
            request,
            style,
            images,
            icons,
        } = self;
        div()
            .size(px(style.size))
            .flex()
            .items_center()
            .justify_center()
            .child(picture(&icon, || request, style, &images, &icons, cx))
    }
}
