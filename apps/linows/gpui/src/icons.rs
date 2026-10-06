//! Row icons, resolved by the backend off the UI thread and kept for the
//! process. The list draws a glyph until the picture lands, and keeps the
//! glyph on a miss.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use base64::Engine;
use gpui::{Context, Image, ImageFormat};
use linows_backend::platform;

use crate::icon_cache;

pub struct IconStore {
    images: HashMap<String, Option<Arc<Image>>>,
    pending: HashSet<String>,
}

pub struct IconRequest {
    pub kind: String,
    pub path: String,
    pub id: Option<String>,
}

impl IconRequest {
    fn key(&self) -> String {
        format!("{}:{}", self.kind, self.path)
    }
}

impl IconStore {
    pub fn new() -> Self {
        Self {
            images: HashMap::new(),
            pending: HashSet::new(),
        }
    }

    /// The picture if it is known; otherwise starts resolving it and
    /// notifies once it is, so the row re-renders with it.
    pub fn get(&mut self, request: IconRequest, cx: &mut Context<Self>) -> Option<Arc<Image>> {
        let key = request.key();
        if let Some(known) = self.images.get(&key) {
            return known.clone();
        }
        if !self.pending.insert(key.clone()) {
            return None;
        }
        cx.spawn(async move |this, cx| {
            let image = cx
                .background_executor()
                .spawn(async move {
                    let url = platform::get_icon(
                        icon_cache(),
                        &request.kind,
                        &request.path,
                        request.id.as_deref(),
                    )
                    .data_url?;
                    decode_data_url(&url).map(Arc::new)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.pending.remove(&key);
                this.images.insert(key, image);
                cx.notify();
            });
        })
        .detach();
        None
    }
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
