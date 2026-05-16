//! Helpers shared across platforms.

use base64::Engine;
use std::fs;

/// Read an icon file from disk and return it as a `data:` URL string.
/// Supports PNG and SVG; returns None for empty files, XPM, or unknown types.
// Linux uses this from platform::linux::icons; Windows will reuse it in M2 for
// the cached-PNG path of the Shell icon pipeline.
#[allow(dead_code)]
pub(crate) fn read_icon_file(path: &str) -> Option<String> {
    let data = fs::read(path).ok()?;
    if data.is_empty() {
        return None;
    }

    let mime = if path.ends_with(".svg") {
        "image/svg+xml"
    } else if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".xpm") {
        return None;
    } else if data.starts_with(b"\x89PNG") {
        "image/png"
    } else if data.starts_with(b"<") || data.starts_with(b"<?xml") {
        "image/svg+xml"
    } else {
        return None;
    };

    let b64 = base64::engine::general_purpose::STANDARD.encode(&data);
    Some(format!("data:{mime};base64,{b64}"))
}
