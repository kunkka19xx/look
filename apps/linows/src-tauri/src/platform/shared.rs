//! Helpers shared across platforms.

use base64::Engine;
use std::fs;

/// `file://` with every byte outside the RFC 3986 unreserved set percent-encoded.
/// `/` is kept literal, since it separates the path rather than belonging to a
/// segment.
pub(crate) fn file_uri(path: &str) -> String {
    let mut encoded = String::from("file://");
    for byte in path.as_bytes() {
        match byte {
            b'/' | b'-' | b'.' | b'_' | b'~' => encoded.push(*byte as char),
            _ if byte.is_ascii_alphanumeric() => encoded.push(*byte as char),
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

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

#[cfg(test)]
mod tests {
    use super::file_uri;

    /// A path with a space is what breaks a naive `file://` + path.
    #[test]
    fn a_uri_encodes_everything_outside_the_unreserved_set() {
        assert_eq!(
            file_uri("/tmp/my project/a b.txt"),
            "file:///tmp/my%20project/a%20b.txt"
        );
        assert_eq!(file_uri("/tmp/a~b-c_d.txt"), "file:///tmp/a~b-c_d.txt");
    }
}
