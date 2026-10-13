//! Pixels for the `ci"` history: one PNG per clip, named after the hash of its
//! pixels, with a thumbnail beside it for the result rows. The index in
//! `clipboard.rs` carries only the hash.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::OnceLock;

const DIR_NAME: &str = "clipboard-images";
const EXTENSION: &str = "png";
/// Any suffix works: the sweep matches files by their leading hash.
const THUMBNAIL_SUFFIX: &str = ".thumb";
const THUMBNAIL_MAX_EDGE: u32 = 128;
const CHANNELS: usize = 4;
/// Characters in a name [`hash_pixels`] returns: a `u64` in hex.
const HASH_LEN: usize = 16;
/// Four bytes a pixel, so this is the real cap on what a capture costs. Clears
/// an 8K screen.
pub const MAX_PIXELS: usize = 64_000_000;

pub struct StoredImage {
    pub hash: String,
    pub width: u32,
    pub height: u32,
    pub byte_size: usize,
}

/// Resolved once: listing the history asks for a path per row.
fn dir() -> Option<&'static PathBuf> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = dirs::data_dir()?.join("look").join(DIR_NAME);
        std::fs::create_dir_all(&dir).ok()?;
        Some(dir)
    })
    .as_ref()
}

pub fn file_path(hash: &str) -> Option<PathBuf> {
    path_for(hash, "")
}

pub fn thumbnail_path(hash: &str) -> Option<PathBuf> {
    path_for(hash, THUMBNAIL_SUFFIX)
}

/// A hash is spliced into a file name, and the commands taking one are open to
/// the webview: anything but the name [`hash_pixels`] writes is refused, so a
/// separator or a `..` cannot reach outside [`DIR_NAME`].
fn path_for(hash: &str, suffix: &str) -> Option<PathBuf> {
    if hash.len() != HASH_LEN || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(dir()?.join(format!("{hash}{suffix}.{EXTENSION}")))
}

/// Writes the image and its thumbnail under the hash of the pixels. A picture
/// already there is left alone: the hash says the bytes are the same ones.
pub fn store(width: u32, height: u32, rgba: &[u8]) -> Option<StoredImage> {
    let hash = hash_pixels(width, height, rgba);
    let image_path = file_path(&hash)?;
    let thumb_path = thumbnail_path(&hash)?;

    let byte_size = match std::fs::metadata(&image_path) {
        Ok(meta) => meta.len() as usize,
        Err(_) => {
            let png = encode_png(width, height, rgba)?;
            std::fs::write(&image_path, &png).ok()?;
            png.len()
        }
    };

    if !thumb_path.exists() {
        let (thumb_width, thumb_height, thumb_rgba) = thumbnail(width, height, rgba);
        if let Some(png) = encode_png(thumb_width, thumb_height, &thumb_rgba) {
            let _ = std::fs::write(&thumb_path, png);
        }
    }

    Some(StoredImage {
        hash,
        width,
        height,
        byte_size,
    })
}

/// Deletes every file the index no longer refers to. Unlinking at each removal
/// site instead leaves megabytes behind whenever a trim, a crash or a failed
/// write gets in between.
pub fn sweep_orphans(live: &HashSet<String>) {
    let Some(dir) = dir() else { return };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !live.iter().any(|hash| name.starts_with(hash.as_str())) {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// Linux offers a paste the PNG bytes as they are; Windows wants pixels.
#[cfg(target_os = "windows")]
pub fn read_rgba(path: &std::path::Path) -> Option<(u32, u32, Vec<u8>)> {
    let file = std::fs::File::open(path).ok()?;
    let mut reader = png::Decoder::new(std::io::BufReader::new(file))
        .read_info()
        .ok()?;
    let info = reader.info();
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return None;
    }
    let mut buffer = vec![0; reader.output_buffer_size()];
    let frame = reader.next_frame(&mut buffer).ok()?;
    buffer.truncate(frame.buffer_size());
    Some((frame.width, frame.height, buffer))
}

/// Names a file and tells two clips apart. Not a checksum: it runs on the poll
/// thread over megabytes, so it reads a word at a time rather than a byte.
pub fn hash_pixels(width: u32, height: u32, rgba: &[u8]) -> String {
    const SEED: u64 = 0xcbf2_9ce4_8422_2325;
    const MIX: u64 = 0x0000_0100_0000_01b3;

    let mut hash = SEED ^ (((width as u64) << 32) | height as u64);
    let (words, remainder) = rgba.as_chunks::<8>();
    for word in words {
        hash = (hash ^ u64::from_le_bytes(*word))
            .wrapping_mul(MIX)
            .rotate_left(27);
    }
    for &byte in remainder {
        hash = (hash ^ byte as u64).wrapping_mul(MIX);
    }
    // Avalanche, so two images a pixel apart differ in the name.
    hash ^= hash >> 33;
    hash = hash.wrapping_mul(0xff51_afd7_ed55_8ccd);
    hash ^= hash >> 29;
    format!("{hash:016x}")
}

/// Fast rather than small: the poll thread pays for every level of compression.
fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    if width == 0 || height == 0 {
        return None;
    }
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Fast);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(rgba).ok()?;
    }
    Some(out)
}

/// Averages whole blocks of pixels, which keeps the aspect ratio without any
/// per-pixel span arithmetic. An image already small enough is its own
/// thumbnail, so a row never has to ask which file exists.
fn thumbnail(width: u32, height: u32, rgba: &[u8]) -> (u32, u32, Vec<u8>) {
    let block = width.max(height).div_ceil(THUMBNAIL_MAX_EDGE);
    if block <= 1 {
        return (width, height, rgba.to_vec());
    }
    let (out_width, out_height) = (width.div_ceil(block), height.div_ceil(block));
    let mut out = Vec::with_capacity(out_width as usize * out_height as usize * CHANNELS);
    for y in (0..height).step_by(block as usize) {
        for x in (0..width).step_by(block as usize) {
            let mut sums = [0u32; CHANNELS];
            let mut count = 0;
            for sy in y..(y + block).min(height) {
                for sx in x..(x + block).min(width) {
                    let at = (sy as usize * width as usize + sx as usize) * CHANNELS;
                    let Some(pixel) = rgba.get(at..at + CHANNELS) else {
                        continue;
                    };
                    for (sum, &channel) in sums.iter_mut().zip(pixel) {
                        *sum += channel as u32;
                    }
                    count += 1;
                }
            }
            out.extend(sums.iter().map(|sum| (sum / count.max(1)) as u8));
        }
    }
    (out_width, out_height, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(width: u32, height: u32, pixel: [u8; 4]) -> Vec<u8> {
        pixel
            .iter()
            .copied()
            .cycle()
            .take(width as usize * height as usize * CHANNELS)
            .collect()
    }

    /// The hash is the row's identity and the file's name, so it has to be the
    /// same across runs for the same pixels, and different for anything else.
    #[test]
    fn the_same_pixels_always_hash_the_same_way() {
        let red = solid(4, 4, [255, 0, 0, 255]);
        let blue = solid(4, 4, [0, 0, 255, 255]);

        assert_eq!(hash_pixels(4, 4, &red), hash_pixels(4, 4, &red));
        assert_ne!(hash_pixels(4, 4, &red), hash_pixels(4, 4, &blue));
        // Same bytes, different shape: two pictures, not one.
        assert_ne!(hash_pixels(4, 4, &red), hash_pixels(2, 8, &red));
    }

    #[test]
    fn a_thumbnail_scales_down_without_distorting_or_discolouring() {
        let source = solid(512, 256, [10, 20, 30, 255]);
        let (width, height, pixels) = thumbnail(512, 256, &source);

        assert_eq!((width, height), (128, 64));
        assert_eq!(pixels.len(), 128 * 64 * CHANNELS);
        assert_eq!(&pixels[..CHANNELS], &[10, 20, 30, 255]);

        // Nothing to scale down: the row draws the image itself.
        let small = solid(64, 32, [1, 2, 3, 255]);
        assert_eq!(thumbnail(64, 32, &small), (64, 32, small));
    }
}
