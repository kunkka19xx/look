//! Helpers shared across platforms.

use base64::Engine;
use std::fs;

/// Read an icon file from disk and return it as a `data:` URL string.
/// PNG and SVG for themed and app icons, plus the formats a user may declare;
/// returns None for empty files, XPM, or unknown types.
// Linux uses this from platform::linux::icons; Windows will reuse it in M2 for
// the cached-PNG path of the Shell icon pipeline.
#[allow(dead_code)]
pub(crate) fn read_icon_file(path: &str) -> Option<String> {
    let data = fs::read(path).ok()?;
    if data.is_empty() {
        return None;
    }

    let lowered = path.to_lowercase();
    let mime = if lowered.ends_with(".svg") {
        "image/svg+xml"
    } else if lowered.ends_with(".png") {
        "image/png"
    } else if lowered.ends_with(".jpg") || lowered.ends_with(".jpeg") {
        "image/jpeg"
    } else if lowered.ends_with(".gif") {
        "image/gif"
    } else if lowered.ends_with(".webp") {
        "image/webp"
    } else if lowered.ends_with(".bmp") {
        "image/bmp"
    } else if lowered.ends_with(".ico") {
        "image/x-icon"
    } else if lowered.ends_with(".xpm") {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A declared icon is read as the file it names, whatever format it is in.
    #[test]
    fn a_declared_image_reads_as_its_own_data_url() {
        let dir = std::env::temp_dir().join(format!("look-icon-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("fixture dir");
        let png = dir.join("Favicon.PNG");
        std::fs::write(&png, b"\x89PNG\r\n\x1a\n rest").expect("fixture file");

        let url = read_icon_file(png.to_str().unwrap()).expect("data url");
        assert!(url.starts_with("data:image/png;base64,"), "{url}");

        let jpg = dir.join("photo.JPG");
        std::fs::write(&jpg, b"\xff\xd8\xff rest").expect("fixture file");
        assert!(
            read_icon_file(jpg.to_str().unwrap())
                .expect("data url")
                .starts_with("data:image/jpeg;base64,")
        );

        assert_eq!(read_icon_file(dir.join("gone.png").to_str().unwrap()), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
