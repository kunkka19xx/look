//! Platform-specific code.
//!
//! Cross-platform entry points and types stay here. Per-OS implementations
//! live under `platform/{linux,windows}/`. `platform/shared.rs` holds helpers
//! reused across platforms.

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "windows")]
pub mod windows;

pub mod shared;

/// Whether the desktop asks for less motion: GNOME's animations switch,
/// which is what GTK and the webview's `prefers-reduced-motion` read.
/// Elsewhere, or without gsettings, the answer is no.
pub fn reduce_motion() -> bool {
    #[cfg(target_os = "linux")]
    {
        linux::host_command("gsettings")
            .args(["get", "org.gnome.desktop.interface", "enable-animations"])
            .output()
            .ok()
            .filter(|out| out.status.success())
            .is_some_and(|out| String::from_utf8_lossy(&out.stdout).trim() == "false")
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;

// --- Icon resolution (cross-platform entry point + cache) ---

#[derive(Default)]
pub struct IconCache(pub Mutex<HashMap<String, Option<String>>>);

impl IconCache {
    pub fn new() -> Self {
        Self::default()
    }
}

/// The kind the frontend asks with for an image a block or row named.
const DECLARED_ICON_KIND: &str = "declared";

#[derive(Serialize)]
pub struct IconResult {
    pub data_url: Option<String>,
}
pub fn get_icon(cache: &IconCache, kind: &str, path: &str, id: Option<&str>) -> IconResult {
    let key = format!("{kind}:{path}");

    {
        let map = cache.0.lock().unwrap();
        if let Some(cached) = map.get(&key) {
            return IconResult {
                data_url: cached.clone(),
            };
        }
    }

    let data_url = if kind == DECLARED_ICON_KIND {
        // The named image IS the icon, so read it rather than ask the theme.
        shared::read_icon_file(path)
    } else {
        resolve_icon(kind, path, id)
    };

    {
        let mut map = cache.0.lock().unwrap();
        map.insert(key, data_url.clone());
    }

    IconResult { data_url }
}

#[cfg(target_os = "linux")]
fn resolve_icon(kind: &str, path: &str, id: Option<&str>) -> Option<String> {
    match kind {
        "app" => linux::icons::resolve_app_icon(path, id),
        "folder" => linux::icons::resolve_themed_icon("folder"),
        _ => linux::icons::resolve_file_icon(path),
    }
}

#[cfg(target_os = "windows")]
fn resolve_icon(kind: &str, path: &str, _id: Option<&str>) -> Option<String> {
    windows::icons::resolve(kind, path)
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn resolve_icon(_kind: &str, _path: &str, _id: Option<&str>) -> Option<String> {
    None
}

/// One blurred rectangle in window-local logical pixels. The UI sends these:
/// only it knows which surfaces are painted (the window at inner-gap 0, each
/// tile once the panes float). Lives here, not under `linux`, because every
/// shell on every OS spells its request with it.
#[derive(serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub struct BlurRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl BlurRect {
    /// Both protocols take rectangles, so a rounded card is its middle band
    /// plus rows stepping around each corner. Rows sit inside the arc: blur
    /// past the curve would frost the gap, while a sliver short of it hides
    /// under the anti-aliased edge. Same construction as src/js/blur.js.
    pub fn rounded(x: i32, y: i32, width: u32, height: u32, radius: f32) -> Vec<BlurRect> {
        let inset = radius
            .min(width as f32 / 2.0)
            .min(height as f32 / 2.0)
            .floor() as i32;
        if inset <= 0 {
            return vec![BlurRect {
                x,
                y,
                width,
                height,
            }];
        }
        let band = inset as u32;
        let mut rects = vec![BlurRect {
            x,
            y: y + inset,
            width,
            height: height - band * 2,
        }];
        // Row i spans [i, i + 1) from the edge; its outer side is the tighter one.
        let indent_at = |i: i32| {
            let r = inset as f32;
            (r - (r * r - (r - i as f32).powi(2)).sqrt()).ceil() as i32
        };
        // Equal neighbours merge, so a large radius costs its steps, not its pixels.
        let mut start = 0;
        for i in 0..inset {
            let indent = indent_at(i);
            if i + 1 < inset && indent_at(i + 1) == indent {
                continue;
            }
            let rows = (i + 1 - start) as u32;
            let indent_w = indent as u32 * 2;
            if width > indent_w {
                let row = BlurRect {
                    x: x + indent,
                    y: y + start,
                    width: width - indent_w,
                    height: rows,
                };
                rects.push(row);
                rects.push(BlurRect {
                    y: y + height as i32 - start - rows as i32,
                    ..row
                });
            }
            start = i + 1;
        }
        rects
    }
}

#[cfg(test)]
mod tests {
    use super::BlurRect;

    #[test]
    fn a_square_corner_is_one_rectangle() {
        assert_eq!(BlurRect::rounded(0, 0, 10, 10, 0.0).len(), 1);
    }

    #[test]
    fn a_rounded_card_keeps_its_middle_band_and_steps_the_corners() {
        let rects = BlurRect::rounded(0, 0, 100, 50, 8.0);
        assert_eq!(
            rects[0],
            BlurRect {
                x: 0,
                y: 8,
                width: 100,
                height: 34
            }
        );
        assert!(rects.len() > 1);
        assert!(
            rects
                .iter()
                .all(|r| r.x >= 0 && r.x + r.width as i32 <= 100)
        );
        assert!(
            rects
                .iter()
                .all(|r| r.y >= 0 && r.y + r.height as i32 <= 50)
        );
    }
}

// --- Drive enumeration (Windows-only payload; stub elsewhere) ---

#[derive(Serialize)]
pub struct CandidateDrive {
    pub letter: String,
    pub root: String,
}
pub fn list_candidate_drives() -> Vec<CandidateDrive> {
    #[cfg(target_os = "windows")]
    {
        windows::drives::enumerate_candidates()
            .into_iter()
            .map(|d| CandidateDrive {
                letter: d.letter,
                root: d.root,
            })
            .collect()
    }
    #[cfg(not(target_os = "windows"))]
    {
        Vec::new()
    }
}
