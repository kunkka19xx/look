//! The look, resolved from `~/.look/config` the way `src/js/theme-defaults.js`
//! and `src/css/theme.css` resolve it for the webview: a preset names the
//! palette, the `ui_*` keys the user owns override it, and `custom` is the
//! keys alone.

use std::collections::HashMap;
use std::sync::RwLock;

use gpui::{BoxShadow, Hsla, Rgba, hsla, point, px};
use linows_backend::config::{self, LauncherLayout};
use linows_backend::highlight::TokenType;
use palette::IntoColor;

pub const FONT_SIZE_DEFAULT: f32 = 14.0;
pub const CAPTION_SIZE: f32 = 11.0;
pub const TILE_VALUE_SIZE: f32 = 20.0;
pub const TILE_VALUE_SIZE_LARGE: f32 = 40.0;

pub const WINDOW_W: f32 = 1008.0;
pub const WINDOW_H: f32 = 672.0;

// Every themed radius is the macOS base times the surface scale.
const TILE_RADIUS_BASE: f32 = 12.0;
const BAR_RADIUS_BASE: f32 = 10.0;
const CONTROL_RADIUS_BASE: f32 = 8.0;
const CHIP_RADIUS_BASE: f32 = 6.0;

pub const CONTENT_PADDING: f32 = 14.0;
pub const INPUT_PADDING_X: f32 = 12.0;
pub const INPUT_PADDING_Y: f32 = 14.0;
pub const TOP_ROW_HEIGHT: f32 = 26.0;
pub const SEARCH_GAP: f32 = 10.0;
pub const SEARCH_ICON: f32 = 16.0;
pub const ROW_HEIGHT: f32 = 48.0;
pub const ROW_PADDING_X: f32 = 10.0;
pub const ROW_SPACING: f32 = 4.0;
pub const ICON_CHIP: f32 = 28.0;
pub const TILE_PADDING: f32 = 12.0;
pub const CHIP_PADDING_X: f32 = 6.0;
pub const CHIP_PADDING_Y: f32 = 1.0;
pub const CARET_WIDTH: f32 = 1.5;
/// Caret height as a share of the font size.
pub const CARET_HEIGHT: f32 = 1.2;

// Floating layout: the gap is every seam, bar to tiles and tile to tile.
pub const ROW_INSET: f32 = 6.0;
pub const HINT_INSET: f32 = 4.0;
pub const HINT_INSET_BOTTOM: f32 = 2.0;
const SHADOW_OFFSET_Y: f32 = 3.0;
const SHADOW_BLUR: f32 = 7.0;
const SHADOW_ALPHA: f32 = 0.25;

const DEFAULT_THEME: &str = "kanagawa";
const CUSTOM_THEME: &str = "custom";
const DEFAULT_TINT_OPACITY: f32 = 0.96;
const DEFAULT_FONT_OPACITY: f32 = 0.96;
const DEFAULT_BORDER_OPACITY: f32 = 0.5;
const DEFAULT_BORDER_THICKNESS: f32 = 2.0;
const DEFAULT_RADIUS_SCALE: f32 = 1.5;
const DEFAULT_INNER_GAP: f32 = 7.0;
/// "Let the theme decide", the config's spelling of no family.
const SYSTEM_FONT: &str = "system-ui";
#[cfg(target_os = "linux")]
pub const PLATFORM_FONT: &str = "Adwaita Sans";
#[cfg(not(target_os = "linux"))]
pub const PLATFORM_FONT: &str = "Segoe UI";
/// Kindle reads as paper, so it asks for a serif.
const KINDLE_FONT: &str = "Noto Serif";
/// Code in the preview. One designed pair per platform, as the CSS stack.
#[cfg(target_os = "linux")]
const PLATFORM_MONO: &str = "Adwaita Mono";
#[cfg(not(target_os = "linux"))]
const PLATFORM_MONO: &str = "Cascadia Code";

/// One preset's palette, the `:root[data-theme]` block of theme.css plus the
/// triplets theme-defaults.js seeds the config with.
struct Preset {
    id: &'static str,
    tint: [f32; 3],
    font: [f32; 3],
    border: [f32; 3],
    /// Set only by the presets whose transparency is their whole look.
    opacities: Option<[f32; 3]>,
    secondary: Rgba,
    muted: Rgba,
    panel: Rgba,
    control: Rgba,
    selection: Rgba,
    accent: Rgba,
    serif: bool,
    /// Keyword, string, comment, number.
    syntax: [Rgba; 4],
}

fn c(r: u8, g: u8, b: u8, a: f32) -> Rgba {
    Rgba::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a)
}

/// The macOS SyntaxHighlighter palette: keyword, string, comment, number.
fn syntax_dark() -> [Rgba; 4] {
    [
        c(209, 125, 201, 1.0),
        c(232, 168, 107, 1.0),
        c(115, 120, 115, 1.0),
        c(140, 199, 214, 1.0),
    ]
}

/// Ink on paper: the same four, darkened for Kindle.
fn syntax_paper() -> [Rgba; 4] {
    [
        c(112, 51, 108, 1.0),
        c(133, 89, 8, 1.0),
        c(122, 116, 105, 1.0),
        c(30, 88, 104, 1.0),
    ]
}

/// Built on demand: palette's colour type has no const constructor, and this
/// runs once per config load.
fn presets() -> Vec<Preset> {
    vec![
        Preset {
            id: "catppuccin",
            tint: [0.12, 0.12, 0.18],
            font: [0.81, 0.8, 0.9],
            border: [0.58, 0.58, 0.65],
            opacities: None,
            secondary: c(200, 206, 224, 1.0),
            muted: c(180, 187, 210, 0.82),
            panel: c(49, 50, 68, 0.5),
            control: c(69, 71, 90, 0.5),
            selection: c(88, 91, 112, 0.4),
            accent: c(137, 180, 250, 1.0),
            serif: false,
            syntax: syntax_dark(),
        },
        Preset {
            id: "tokyo-night",
            tint: [0.1, 0.11, 0.15],
            font: [0.84, 0.87, 0.96],
            border: [0.66, 0.69, 0.84],
            opacities: None,
            secondary: c(189, 204, 230, 1.0),
            muted: c(143, 163, 199, 0.78),
            panel: c(15, 23, 46, 0.38),
            control: c(36, 43, 71, 0.34),
            selection: c(97, 120, 184, 0.28),
            accent: c(133, 184, 250, 1.0),
            serif: false,
            syntax: syntax_dark(),
        },
        Preset {
            id: "rose-pine",
            tint: [0.1, 0.09, 0.14],
            font: [0.95, 0.93, 0.91],
            border: [0.88, 0.87, 0.96],
            opacities: None,
            secondary: c(224, 214, 207, 1.0),
            muted: c(189, 178, 168, 0.78),
            panel: c(33, 28, 43, 0.38),
            control: c(51, 46, 64, 0.34),
            selection: c(148, 133, 168, 0.28),
            accent: c(184, 156, 199, 1.0),
            serif: false,
            syntax: syntax_dark(),
        },
        Preset {
            id: "gruvbox",
            tint: [0.16, 0.16, 0.16],
            font: [0.93, 0.89, 0.79],
            border: [0.92, 0.86, 0.7],
            opacities: None,
            secondary: c(222, 204, 163, 1.0),
            muted: c(184, 163, 122, 0.78),
            panel: c(36, 28, 23, 0.38),
            control: c(54, 43, 33, 0.34),
            selection: c(189, 138, 66, 0.28),
            accent: c(219, 184, 102, 1.0),
            serif: false,
            syntax: syntax_dark(),
        },
        Preset {
            id: "dracula",
            tint: [0.16, 0.16, 0.21],
            font: [0.97, 0.97, 0.98],
            border: [0.97, 0.97, 0.95],
            opacities: None,
            secondary: c(235, 222, 250, 1.0),
            muted: c(196, 189, 217, 0.78),
            panel: c(28, 25, 46, 0.38),
            control: c(54, 51, 77, 0.34),
            selection: c(158, 133, 201, 0.28),
            accent: c(163, 191, 250, 1.0),
            serif: false,
            syntax: syntax_dark(),
        },
        Preset {
            id: "kanagawa",
            tint: [0.09, 0.09, 0.11],
            font: [0.94, 0.85, 0.57],
            border: [0.86, 0.84, 0.73],
            opacities: None,
            secondary: c(204, 199, 168, 1.0),
            muted: c(168, 161, 128, 0.78),
            panel: c(25, 31, 36, 0.38),
            control: c(46, 51, 61, 0.34),
            selection: c(128, 122, 97, 0.28),
            accent: c(117, 166, 209, 1.0),
            serif: false,
            syntax: syntax_dark(),
        },
        Preset {
            id: "kindle",
            tint: [0.97, 0.95, 0.9],
            font: [0.13, 0.12, 0.1],
            border: [0.42, 0.38, 0.32],
            opacities: Some([0.93, 1.0, 0.26]),
            secondary: c(61, 56, 48, 1.0),
            muted: c(77, 71, 61, 1.0),
            panel: c(255, 252, 245, 0.55),
            control: c(217, 209, 194, 0.55),
            selection: c(61, 56, 48, 0.16),
            accent: c(46, 43, 38, 1.0),
            serif: true,
            syntax: syntax_paper(),
        },
        Preset {
            id: "liquid",
            tint: [0.1, 0.13, 0.2],
            font: [0.98, 0.98, 1.0],
            border: [1.0, 1.0, 1.0],
            opacities: Some([0.78, 1.0, 0.1]),
            secondary: c(219, 227, 242, 1.0),
            muted: c(184, 196, 222, 1.0),
            panel: c(158, 184, 235, 0.1),
            control: c(168, 194, 240, 0.12),
            selection: c(102, 153, 255, 0.58),
            accent: c(112, 184, 255, 1.0),
            serif: false,
            syntax: syntax_dark(),
        },
    ]
}

fn preset(id: &str) -> Preset {
    let mut all = presets();
    let at = all
        .iter()
        .position(|p| p.id == id)
        .or_else(|| all.iter().position(|p| p.id == DEFAULT_THEME))
        .expect("the default preset exists");
    all.swap_remove(at)
}

#[derive(Clone)]
pub struct Theme {
    pub tint: Rgba,
    pub text: Rgba,
    pub text_secondary: Rgba,
    pub text_muted: Rgba,
    pub panel_fill: Rgba,
    pub control_fill: Rgba,
    pub selection_fill: Rgba,
    pub border: Rgba,
    pub accent: Rgba,
    pub font_family: String,
    pub mono_family: String,
    pub font_size: f32,
    pub border_thickness: f32,
    pub radius_scale: f32,
    pub inner_gap: f32,
    pub layout: LauncherLayout,
    syntax: [Rgba; 4],
}

impl Theme {
    /// Read the config and resolve every token. Called at start and on reload.
    pub fn from_config() -> Self {
        let entries: HashMap<String, String> = config::get_config()
            .entries
            .into_iter()
            // `key=` means unset, not "".
            .filter(|e| !e.value.is_empty())
            .map(|e| (e.key, e.value))
            .collect();
        let get = |key: &str| entries.get(key).map(String::as_str);
        let num =
            |key: &str, fallback: f32| get(key).and_then(|v| v.parse().ok()).unwrap_or(fallback);

        let id = get("ui_theme").unwrap_or(DEFAULT_THEME);
        let preset = preset(id);
        let [tint_op, font_op, border_op] = preset.opacities.unwrap_or([
            DEFAULT_TINT_OPACITY,
            DEFAULT_FONT_OPACITY,
            DEFAULT_BORDER_OPACITY,
        ]);
        // A custom theme is its keys; a preset ignores stale ones left behind.
        let triplet = |prefix: &str, fallback: [f32; 3]| {
            if id != CUSTOM_THEME {
                return fallback;
            }
            [
                num(&format!("{prefix}_red"), fallback[0]),
                num(&format!("{prefix}_green"), fallback[1]),
                num(&format!("{prefix}_blue"), fallback[2]),
            ]
        };
        let rgba = |[r, g, b]: [f32; 3], a: f32| Rgba::new(r, g, b, a);

        let font_family = match get("ui_font_name").filter(|f| *f != SYSTEM_FONT) {
            Some(family) => family.to_string(),
            None if preset.serif => KINDLE_FONT.to_string(),
            None => PLATFORM_FONT.to_string(),
        };

        Self {
            tint: rgba(
                triplet("ui_tint", preset.tint),
                num("ui_tint_opacity", tint_op),
            ),
            text: rgba(
                triplet("ui_font", preset.font),
                num("ui_font_opacity", font_op),
            ),
            text_secondary: preset.secondary,
            text_muted: preset.muted,
            panel_fill: preset.panel,
            control_fill: preset.control,
            selection_fill: preset.selection,
            border: rgba(
                triplet("ui_border", preset.border),
                num("ui_border_opacity", border_op),
            ),
            accent: preset.accent,
            font_family,
            mono_family: PLATFORM_MONO.to_string(),
            font_size: num("ui_font_size", FONT_SIZE_DEFAULT),
            border_thickness: num("ui_border_thickness", DEFAULT_BORDER_THICKNESS),
            radius_scale: num("ui_surface_radius", DEFAULT_RADIUS_SCALE),
            inner_gap: num("inner_gap", DEFAULT_INNER_GAP),
            layout: config::launcher_layout(),
            syntax: preset.syntax,
        }
    }

    /// Whether the preview column is shown beside the results.
    pub fn split(&self) -> bool {
        matches!(self.layout, LauncherLayout::Split)
    }

    pub fn syntax(&self, token: TokenType) -> Rgba {
        let at = match token {
            TokenType::Keyword => 0,
            TokenType::String => 1,
            TokenType::Comment => 2,
            TokenType::Number => 3,
        };
        self.syntax[at]
    }

    pub fn tile_radius(&self) -> f32 {
        TILE_RADIUS_BASE * self.radius_scale
    }
    pub fn bar_radius(&self) -> f32 {
        BAR_RADIUS_BASE * self.radius_scale
    }
    pub fn control_radius(&self) -> f32 {
        CONTROL_RADIUS_BASE * self.radius_scale
    }
    pub fn chip_radius(&self) -> f32 {
        CHIP_RADIUS_BASE * self.radius_scale
    }

    /// The top bar and results card: control fill over the tint (`.pane-tile`).
    pub fn card_face(&self) -> Rgba {
        over(self.control_fill, self.tint)
    }

    /// Launchpad tiles stack the tint twice so desktop text cannot read through
    /// (`.ctl-tile --ctl-face`).
    pub fn tile_face(&self) -> Rgba {
        over(self.control_fill, over(self.tint, self.tint))
    }

    /// Off under a compositor blur: any shadow pixel in a gap would be frosted
    /// with it (shadow or blur, not both, as the macOS material).
    pub fn card_shadow(&self) -> Vec<BoxShadow> {
        if crate::blur::is_supported() {
            return Vec::new();
        }
        vec![BoxShadow {
            color: hsla(0.0, 0.0, 0.0, SHADOW_ALPHA),
            offset: point(px(0.0), px(SHADOW_OFFSET_Y)),
            blur_radius: px(SHADOW_BLUR),
            spread_radius: px(0.0),
            inset: false,
        }]
    }
}

static THEME: RwLock<Option<Theme>> = RwLock::new(None);

/// Resolve the config again. Cheap: one file read.
pub fn load() {
    *THEME.write().unwrap_or_else(|p| p.into_inner()) = Some(Theme::from_config());
}

/// A snapshot for one render, so every token in a frame agrees.
pub fn get() -> Theme {
    let guard = THEME.read().unwrap_or_else(|p| p.into_inner());
    match guard.as_ref() {
        Some(theme) => theme.clone(),
        None => Theme::from_config(),
    }
}

pub fn hsla_of(c: Rgba) -> Hsla {
    c.into_color()
}

/// Source-over, what the CSS gradient stacks resolve to on a transparent
/// window. One flat colour per face keeps it a single quad.
fn over(top: Rgba, bottom: Rgba) -> Rgba {
    let alpha = top.alpha + bottom.alpha * (1.0 - top.alpha);
    if alpha == 0.0 {
        return Rgba::new(0.0, 0.0, 0.0, 0.0);
    }
    let mix = |t: f32, b: f32| (t * top.alpha + b * bottom.alpha * (1.0 - top.alpha)) / alpha;
    Rgba::new(
        mix(top.color.red, bottom.color.red),
        mix(top.color.green, bottom.color.green),
        mix(top.color.blue, bottom.color.blue),
        alpha,
    )
}
