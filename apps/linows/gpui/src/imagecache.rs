//! The preview's pictures, a few at a time. gpui's asset store keeps every
//! decoded image for the life of the process and the browser's cache used
//! to evict for the webview, so a walk through a photo folder would pin one
//! RGBA texture per photo. This cache keeps the last few the panel showed
//! and releases the rest, store entry and GPU copy both; dropping it, which
//! a hide does with the launcher, releases them all.

use std::collections::VecDeque;
use std::sync::Arc;

use gpui::{
    App, Context, ImageCache, ImageCacheError, ImgResourceLoader, RenderImage, Resource, Window,
};

/// Current, previous, and two the user bounces between.
const PREVIEW_IMAGES: usize = 4;

pub struct PreviewImages {
    capacity: usize,
    /// Most recently shown last. The decoded image rides along once it has
    /// landed, so the texture can be dropped with the entry.
    recent: VecDeque<(Resource, Option<Arc<RenderImage>>)>,
}

impl PreviewImages {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self::with_capacity(PREVIEW_IMAGES, cx)
    }

    pub fn with_capacity(capacity: usize, cx: &mut Context<Self>) -> Self {
        cx.on_release(|cache, cx| {
            for (resource, image) in cache.recent.drain(..) {
                release(&resource, image, None, cx);
            }
        })
        .detach();
        Self {
            capacity,
            recent: VecDeque::with_capacity(capacity + 1),
        }
    }
}

fn release(
    resource: &Resource,
    image: Option<Arc<RenderImage>>,
    window: Option<&mut Window>,
    cx: &mut App,
) {
    cx.remove_asset::<ImgResourceLoader>(resource);
    if let Some(image) = image {
        cx.drop_image(image, window);
    }
}

impl ImageCache for PreviewImages {
    fn load(
        &mut self,
        resource: &Resource,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Result<Arc<RenderImage>, ImageCacheError>> {
        let result = window.use_asset::<ImgResourceLoader>(resource, cx);
        let prior = self
            .recent
            .iter()
            .position(|(r, _)| r == resource)
            .and_then(|i| self.recent.remove(i))
            .and_then(|(_, image)| image);
        let image = match &result {
            Some(Ok(image)) => Some(image.clone()),
            _ => prior,
        };
        self.recent.push_back((resource.clone(), image));
        while self.recent.len() > self.capacity {
            let (resource, image) = self.recent.pop_front().expect("over the cap");
            release(&resource, image, Some(window), cx);
        }
        result
    }
}
