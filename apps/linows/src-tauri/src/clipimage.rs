//! Pixels for the `ci"` history.
//!
//! A copied image is megabytes, so the index in `clipboard.rs` keeps only a
//! hash and the bytes live in a file named after it. A re-copy of the same
//! picture lands on the file it already wrote. Beside each image sits a
//! thumbnail, which is what a result row draws: scaling a 4K screenshot down
//! to 32 px on every render stutters the list.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::OnceLock;

const DIR_NAME: &str = "clipboard-images";
const EXTENSION: &str = "png";
/// Any suffix is safe: the sweep matches files by their leading hash.
const THUMBNAIL_SUFFIX: &str = ".thumb";
/// Longest edge of the thumbnail a row draws.
const THUMBNAIL_MAX_EDGE: u32 = 128;
const CHANNELS: usize = 4;
/// Bytes say nothing about what they decode to, at four bytes a pixel. Clears
/// an 8K screenshot and bounds what a capture costs.
pub const MAX_PIXELS: usize = 64_000_000;

pub struct StoredImage {
    pub hash: String,
    pub width: u32,
    pub height: u32,
    pub byte_size: usize,
}

/// Where the bytes live. Resolved and created once: listing the history asks
/// for a path per row, and that is no reason to hit the filesystem per row.
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

fn path_for(hash: &str, suffix: &str) -> Option<PathBuf> {
    if hash.is_empty() {
        return None;
    }
    Some(dir()?.join(format!("{hash}{suffix}.{EXTENSION}")))
}

/// Writes the image and its thumbnail under the hash of the pixels. A picture
/// already on disk is left alone: the hash says the bytes are the same ones.
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
/// write gets in between; sweeping self-heals, and the directory holds tens of
/// files.
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

/// The RGBA pixels of a file this module wrote, for handing back to the
/// clipboard. Linux offers the PNG bytes as they are; Windows wants pixels.
#[cfg(target_os = "windows")]
pub fn read_rgba(path: &std::path::Path) -> Option<(u32, u32, Vec<u8>)> {
    let file = std::fs::File::open(path).ok()?;
    let decoder = png::Decoder::new(std::io::BufReader::new(file));
    let mut reader = decoder.read_info().ok()?;
    let info = reader.info();
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return None;
    }
    let mut buffer = vec![0; reader.output_buffer_size()];
    let frame = reader.next_frame(&mut buffer).ok()?;
    buffer.truncate(frame.buffer_size());
    Some((frame.width, frame.height, buffer))
}

/// Names a file and tells two clips apart. Not a checksum and never used as
/// one: it runs on the poll thread over megabytes, so it reads a word at a
/// time rather than a byte.
pub fn hash_pixels(width: u32, height: u32, rgba: &[u8]) -> String {
    const SEED: u64 = 0xcbf2_9ce4_8422_2325;
    const MIX: u64 = 0x0000_0100_0000_01b3;

    let mut hash = SEED ^ (((width as u64) << 32) | height as u64);
    let mut words = rgba.chunks_exact(8);
    for word in &mut words {
        let value = u64::from_le_bytes(word.try_into().unwrap_or_default());
        hash = (hash ^ value).wrapping_mul(MIX).rotate_left(27);
    }
    for &byte in words.remainder() {
        hash = (hash ^ byte as u64).wrapping_mul(MIX);
    }
    // Final avalanche, so two images differing in one pixel differ in the name.
    hash ^= hash >> 33;
    hash = hash.wrapping_mul(0xff51_afd7_ed55_8ccd);
    hash ^= hash >> 29;
    format!("{hash:016x}")
}

/// Fast rather than small: these are clips the user will delete, and the poll
/// thread pays for every level of compression.
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

/// Box average down to [`THUMBNAIL_MAX_EDGE`]. An image already that small is
/// its own thumbnail, so a row never has to ask which file exists.
fn thumbnail(width: u32, height: u32, rgba: &[u8]) -> (u32, u32, Vec<u8>) {
    let longest = width.max(height);
    if longest <= THUMBNAIL_MAX_EDGE || longest == 0 {
        return (width, height, rgba.to_vec());
    }
    let target_width = scaled_edge(width, longest);
    let target_height = scaled_edge(height, longest);

    let mut out = vec![0u8; target_width as usize * target_height as usize * CHANNELS];
    for y in 0..target_height {
        let (y0, y1) = source_span(y, target_height, height);
        for x in 0..target_width {
            let (x0, x1) = source_span(x, target_width, width);
            let mut sums = [0u32; CHANNELS];
            let mut count = 0u32;
            for sy in y0..y1 {
                let row = sy as usize * width as usize * CHANNELS;
                for sx in x0..x1 {
                    let at = row + sx as usize * CHANNELS;
                    let Some(pixel) = rgba.get(at..at + CHANNELS) else {
                        continue;
                    };
                    for (sum, &channel) in sums.iter_mut().zip(pixel) {
                        *sum += channel as u32;
                    }
                    count += 1;
                }
            }
            if count == 0 {
                continue;
            }
            let at = (y as usize * target_width as usize + x as usize) * CHANNELS;
            for (channel, sum) in sums.iter().enumerate() {
                out[at + channel] = (sum / count) as u8;
            }
        }
    }
    (target_width, target_height, out)
}

fn scaled_edge(edge: u32, longest: u32) -> u32 {
    ((edge as u64 * THUMBNAIL_MAX_EDGE as u64) / longest as u64).max(1) as u32
}

/// The source pixels one target pixel averages, never empty.
fn source_span(index: u32, target_edge: u32, source_edge: u32) -> (u32, u32) {
    let start = (index as u64 * source_edge as u64 / target_edge as u64) as u32;
    let end = ((index as u64 + 1) * source_edge as u64 / target_edge as u64) as u32;
    (start, end.max(start + 1).min(source_edge))
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

    /// The hash is the row's identity, so the same pixels must name the same
    /// file across runs and a different picture must not.
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
    fn a_thumbnail_keeps_the_aspect_ratio_and_the_colour() {
        let source = solid(512, 256, [10, 20, 30, 255]);
        let (width, height, pixels) = thumbnail(512, 256, &source);

        assert_eq!((width, height), (128, 64));
        assert_eq!(pixels.len(), 128 * 64 * CHANNELS);
        assert_eq!(&pixels[..CHANNELS], &[10, 20, 30, 255]);
    }

    /// Nothing to scale down: the row draws the image itself.
    #[test]
    fn a_small_image_is_its_own_thumbnail() {
        let source = solid(64, 32, [1, 2, 3, 255]);
        let (width, height, pixels) = thumbnail(64, 32, &source);

        assert_eq!((width, height), (64, 32));
        assert_eq!(pixels, source);
    }

    /// Every target pixel has at least one source pixel to average, however
    /// lopsided the picture is.
    #[test]
    fn every_target_pixel_covers_source_pixels() {
        for (target_edge, source_edge) in [(128u32, 4096u32), (1, 7), (3, 4)] {
            for index in 0..target_edge {
                let (start, end) = source_span(index, target_edge, source_edge);
                assert!(start < end, "{index} of {target_edge} covers nothing");
                assert!(end <= source_edge);
            }
        }
    }
}
