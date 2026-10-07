//! Ctrl+Shift+, : the settings screen, the webview's `settings.js` and
//! `settings.html` on this shell, run the way the macOS ThemeStore runs:
//! every control edits a working copy that the theme follows at once, Save
//! Config writes the lot to the file in one go, Esc discards.

use std::collections::HashMap;
use std::time::Duration;

use gpui::prelude::*;
use gpui::{
    AnyElement, Context, Div, Entity, FocusHandle, FontWeight, ScrollHandle, SharedString,
    deferred, div, px, relative,
};
use linows_backend::config::{self, ConfigUpdate};
use linows_backend::files;
use linows_backend::search as engine;

use crate::bg;
use crate::commands::KeyOutcome;
use crate::controls::{self, Option_, Range, SliderEvent, SliderPhase};
use crate::fonts;
use crate::launcher::Launcher;
use crate::launchpad::Tone;
use crate::search::{Changed, SearchInput, search_field};
use crate::state as app_state;
use crate::theme::{self, Theme};

const TITLE: &str = "Settings";
const SAVE: &str = "Save Config";
const CLOSE_HINT: &str = "Esc or Ctrl+Shift+, to close";
const SAVED: &str = "Saved";
const SAVE_FAILED: &str = "Save failed";
const SAVE_MESSAGE_MS: u64 = 1600;
const SAVE_FAILED_BANNER_SECS: f32 = 4.0;
const PENDING_TAB: &str = "comes in its own PR";
pub const RELOADED: &str = "Config reloaded from file";
pub const RELOADED_SECS: f32 = 1.2;

const HEADER_PADDING_X: f32 = 14.0;
const HEADER_PADDING_Y: f32 = 10.0;
const HEADER_GAP: f32 = 10.0;
const TITLE_SIZE_STEP: f32 = 2.0;
const PILL_PADDING_X: f32 = 10.0;
const PILL_PADDING_Y: f32 = 3.0;
const TABS_GAP: f32 = 4.0;
const TABS_PADDING_BOTTOM: f32 = 6.0;
const TAB_PADDING_Y: f32 = 7.0;
const BODY_PADDING_BOTTOM: f32 = 14.0;
/// The header row's items, spaced as the webview's `margin-left: 40px`.
const HEADER_ROW_GAP: f32 = 40.0;
const HEADER_ITEM_GAP: f32 = 10.0;
const FONT_BOX_W: f32 = 200.0;
const FONT_PLACEHOLDER: &str = "Installed font name";
const SUGGESTIONS_MAX: usize = 8;
const SUGGESTION_PADDING_X: f32 = 10.0;
const SUGGESTION_PADDING_Y: f32 = 5.0;
const SUGGESTIONS_GAP: f32 = 4.0;
const SUGGESTIONS_PADDING: f32 = 4.0;

const THEME_KEY: &str = "ui_theme";
const SURFACE_KEY: &str = "ui_surface";
const LIQUID: &str = "liquid";
const LAYOUT_KEY: &str = "layout";
const FONT_NAME_KEY: &str = "ui_font_name";
const BLUR_STYLE_KEY: &str = "ui_blur_style";
const BLUR_OPACITY_KEY: &str = "ui_blur_opacity";
const SETTINGS_BLUR_KEY: &str = "settings_blur_multiplier";
const SETTINGS_BLUR_DEFAULT: f32 = 1.0;

const THEMES: &[Option_] = &[
    ("catppuccin", "Catppuccin"),
    ("tokyo-night", "Tokyo Night"),
    ("rose-pine", "Rose Pine"),
    ("gruvbox", "Gruvbox"),
    ("dracula", "Dracula"),
    ("kanagawa", "Kanagawa"),
    ("kindle", "Kindle"),
    (LIQUID, "Liquid"),
    (theme::CUSTOM_THEME, "Custom"),
];
const LAYOUTS: &[Option_] = &[("split", "Split"), ("compact", "Compact")];
const LAYOUT_DEFAULT: &str = "split";

#[cfg(target_os = "linux")]
const BLUR_STYLES: &[Option_] = &[
    ("high_contrast", "High Contrast"),
    ("balanced", "Balanced"),
    ("soft", "Soft"),
];
#[cfg(target_os = "linux")]
const BLUR_STYLE_HINTS: [&str; 3] = [
    "Darkest and most readable",
    "Default translucency",
    "Lightest, most transparent",
];
#[cfg(not(target_os = "linux"))]
const BLUR_STYLES: &[Option_] = &[
    ("high_contrast", "Mica"),
    ("balanced", "Acrylic"),
    ("soft", "Acrylic (Soft)"),
];
#[cfg(not(target_os = "linux"))]
const BLUR_STYLE_HINTS: [&str; 3] = [
    "Windows 11 native blur",
    "Translucent with blur",
    "Lightest acrylic",
];
/// The blur opacity each style brings along.
const BLUR_STYLE_OPACITY: [f32; 3] = [0.95, 0.8, 0.6];
const BLUR_STYLE_DEFAULT: usize = 0;

/// A switch's config spelling.
struct Switch {
    id: &'static str,
    key: &'static str,
    label: &'static str,
    on: &'static str,
    off: &'static str,
    default_on: bool,
}

const RUNNING_APPS: Switch = Switch {
    id: "settings-running",
    key: "running_apps_placement",
    label: "Running Apps",
    on: "right",
    off: "none",
    default_on: true,
};
const SUPER_ACTIONS: Switch = Switch {
    id: "settings-launchpad",
    key: "super_actions_enabled",
    label: "Super Actions",
    on: "true",
    off: "false",
    default_on: false,
};
/// Saved only: the motion pass (M7) reads it.
const ANIMATIONS: Switch = Switch {
    id: "settings-animations",
    key: "animations_enabled",
    label: "Animations",
    on: "true",
    off: "false",
    default_on: true,
};

/// Where a slider's value comes from when the config has none: a preset
/// colour channel, a preset opacity, or the spec's default.
#[derive(Clone, Copy, PartialEq)]
enum Slot {
    Plain,
    Tint(usize),
    Font(usize),
    Border(usize),
    TintOpacity,
    FontOpacity,
    BorderOpacity,
}

impl Slot {
    fn is_colour(self) -> bool {
        matches!(self, Slot::Tint(_) | Slot::Font(_) | Slot::Border(_))
    }
}

struct Slider {
    key: &'static str,
    label: &'static str,
    range: Range,
    default: f32,
    slot: Slot,
    decimals: usize,
}

const UNIT: Range = Range {
    min: 0.0,
    max: 1.0,
    step: 0.01,
};

const fn unit(key: &'static str, label: &'static str, default: f32, slot: Slot) -> Slider {
    Slider {
        key,
        label,
        range: UNIT,
        default,
        slot,
        decimals: 2,
    }
}

static INNER_GAP: Slider = Slider {
    key: "inner_gap",
    label: "Inner Gap",
    range: Range {
        min: 0.0,
        max: 24.0,
        step: 1.0,
    },
    default: theme::DEFAULT_INNER_GAP,
    slot: Slot::Plain,
    decimals: 0,
};
static CORNER_RADIUS: Slider = Slider {
    key: "ui_surface_radius",
    label: "Corner Radius",
    range: Range {
        min: 0.0,
        max: 2.5,
        step: 0.1,
    },
    default: theme::DEFAULT_RADIUS_SCALE,
    slot: Slot::Plain,
    decimals: 2,
};
static TINT: [Slider; 4] = [
    unit("ui_tint_red", "Red", 0.0, Slot::Tint(0)),
    unit("ui_tint_green", "Green", 0.0, Slot::Tint(1)),
    unit("ui_tint_blue", "Blue", 0.0, Slot::Tint(2)),
    unit(
        "ui_tint_opacity",
        "Tint Opacity",
        theme::DEFAULT_TINT_OPACITY,
        Slot::TintOpacity,
    ),
];
static BLUR_OPACITY: Slider = unit(BLUR_OPACITY_KEY, "Blur Opacity", 0.95, Slot::Plain);
static SETTINGS_BLUR: Slider = Slider {
    key: SETTINGS_BLUR_KEY,
    label: "Settings Blur",
    range: Range {
        min: 0.4,
        max: 1.0,
        step: 0.01,
    },
    default: SETTINGS_BLUR_DEFAULT,
    slot: Slot::Plain,
    decimals: 2,
};
static FONT_SIZE: Slider = Slider {
    key: "ui_font_size",
    label: "Font Size",
    range: Range {
        min: 10.0,
        max: 28.0,
        step: 1.0,
    },
    default: theme::FONT_SIZE_DEFAULT,
    slot: Slot::Plain,
    decimals: 0,
};
static FONT: [Slider; 4] = [
    unit("ui_font_red", "Text Red", 0.0, Slot::Font(0)),
    unit("ui_font_green", "Text Green", 0.0, Slot::Font(1)),
    unit("ui_font_blue", "Text Blue", 0.0, Slot::Font(2)),
    unit(
        "ui_font_opacity",
        "Text Opacity",
        theme::DEFAULT_FONT_OPACITY,
        Slot::FontOpacity,
    ),
];
static BORDER_THICKNESS: Slider = Slider {
    key: "ui_border_thickness",
    label: "Border Thick",
    range: Range {
        min: 0.0,
        max: 6.0,
        step: 0.5,
    },
    default: theme::DEFAULT_BORDER_THICKNESS,
    slot: Slot::Plain,
    decimals: 2,
};
static BORDER: [Slider; 4] = [
    unit("ui_border_red", "Border Red", 0.0, Slot::Border(0)),
    unit("ui_border_green", "Border Green", 0.0, Slot::Border(1)),
    unit("ui_border_blue", "Border Blue", 0.0, Slot::Border(2)),
    unit(
        "ui_border_opacity",
        "Border Opacity",
        theme::DEFAULT_BORDER_OPACITY,
        Slot::BorderOpacity,
    ),
];

/// Every slider, for Save Config and the custom flip.
static SLIDERS: [&Slider; 18] = [
    &INNER_GAP,
    &CORNER_RADIUS,
    &TINT[0],
    &TINT[1],
    &TINT[2],
    &TINT[3],
    &BLUR_OPACITY,
    &SETTINGS_BLUR,
    &FONT_SIZE,
    &FONT[0],
    &FONT[1],
    &FONT[2],
    &FONT[3],
    &BORDER_THICKNESS,
    &BORDER[0],
    &BORDER[1],
    &BORDER[2],
    &BORDER[3],
];

/// The keys this screen owns besides the sliders'.
const KEYS: [&str; 8] = [
    THEME_KEY,
    SURFACE_KEY,
    LAYOUT_KEY,
    FONT_NAME_KEY,
    BLUR_STYLE_KEY,
    RUNNING_APPS.key,
    SUPER_ACTIONS.key,
    ANIMATIONS.key,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Appearance,
    Advanced,
    Shortcuts,
}

impl Tab {
    const ALL: [Tab; 3] = [Tab::Appearance, Tab::Advanced, Tab::Shortcuts];

    fn label(self) -> &'static str {
        match self {
            Tab::Appearance => "Appearance",
            Tab::Advanced => "Advanced",
            Tab::Shortcuts => "Shortcuts",
        }
    }

    fn hint(self) -> &'static str {
        match self {
            Tab::Appearance => {
                "Tab: Switch tabs \u{2022} Save Config: Keep changes \u{2022} Esc: Discard"
            }
            Tab::Advanced => {
                "Save Config applies changes immediately. Ctrl+Shift+; is only needed after editing .look/config manually."
            }
            Tab::Shortcuts => {
                "Tips: t\"word for web EN/VI/JA translation \u{2022} /kill to force quit apps"
            }
        }
    }

    fn step(self, back: bool) -> Tab {
        let at = Tab::ALL.iter().position(|t| *t == self).unwrap_or(0);
        let n = Tab::ALL.len();
        Tab::ALL[if back { (at + n - 1) % n } else { (at + 1) % n }]
    }
}

/// The font box while it edits.
struct Field {
    input: Entity<SearchInput>,
    /// The suggestion the arrows landed on.
    highlighted: Option<usize>,
}

struct SaveMessage {
    text: &'static str,
    ok: bool,
}

pub struct Settings {
    pub tab: Tab,
    /// The config as this screen last read it, with its own changes on top.
    entries: HashMap<String, String>,
    fonts: Vec<String>,
    field: Option<Field>,
    /// The open dropdown's key.
    select: Option<&'static str>,
    /// The slider under the pointer's drag.
    drag: Option<&'static str>,
    scroll: ScrollHandle,
    /// Holds the keys while no box edits, so a stray keystroke cannot reach
    /// the hidden search field.
    focus: FocusHandle,
    save_message: Option<SaveMessage>,
    save_seq: u64,
}

impl Settings {
    pub fn new(cx: &mut Context<Launcher>) -> Self {
        Self {
            tab: Tab::default(),
            entries: HashMap::new(),
            fonts: Vec::new(),
            field: None,
            select: None,
            drag: None,
            scroll: ScrollHandle::new(),
            focus: cx.focus_handle(),
            save_message: None,
            save_seq: 0,
        }
    }

    pub fn enter(&mut self, cx: &mut Context<Launcher>) {
        self.tab = Tab::default();
        self.scroll.set_offset(Default::default());
        self.reload(cx);
        if self.fonts.is_empty() {
            bg::fetch(cx, files::list_fonts, |this, fonts, _| {
                this.settings.fonts = fonts;
            });
        }
    }

    pub fn leave(&mut self) {
        self.field = None;
        self.select = None;
        self.drag = None;
    }

    /// Read the file again: on open, and on Ctrl+Shift+; while open.
    pub fn reload(&mut self, cx: &mut Context<Launcher>) {
        bg::fetch(cx, theme::entries, |this, entries, _| {
            this.settings.entries = entries;
        });
    }

    pub fn hint(&self) -> &'static str {
        self.tab.hint()
    }

    pub fn editing_field(&self) -> Option<Entity<SearchInput>> {
        self.field.as_ref().map(|f| f.input.clone())
    }

    pub fn focus_handle(&self) -> FocusHandle {
        self.focus.clone()
    }

    /// `settings_blur_multiplier`: the tint alpha's share while this screen
    /// is up.
    pub fn tint_share(&self) -> f32 {
        self.number(SETTINGS_BLUR_KEY, SETTINGS_BLUR_DEFAULT)
    }

    pub fn handle_key(&mut self, ks: &gpui::Keystroke, cx: &mut Context<Launcher>) -> KeyOutcome {
        let shift = ks.modifiers.shift;
        if self.select.is_some() {
            if ks.key == "escape" {
                self.select = None;
                cx.notify();
            }
            return KeyOutcome::Consumed;
        }
        if self.field.is_some() {
            return match ks.key.as_str() {
                "escape" => {
                    self.field = None;
                    cx.notify();
                    KeyOutcome::Consumed
                }
                "enter" => {
                    self.commit_font(cx);
                    KeyOutcome::Consumed
                }
                "down" => self.move_highlight(1, cx),
                "up" => self.move_highlight(-1, cx),
                _ => KeyOutcome::Pass,
            };
        }
        match ks.key.as_str() {
            "escape" => KeyOutcome::Exit,
            "tab" => {
                self.tab = self.tab.step(shift);
                self.scroll.set_offset(Default::default());
                cx.notify();
                KeyOutcome::Consumed
            }
            _ => KeyOutcome::Consumed,
        }
    }

    // --- Values ------------------------------------------------------------------

    fn get(&self, key: &str) -> Option<&str> {
        self.entries.get(key).map(String::as_str)
    }

    fn number(&self, key: &str, fallback: f32) -> f32 {
        self.get(key)
            .and_then(|v| v.parse().ok())
            .unwrap_or(fallback)
    }

    fn theme_id(&self) -> &str {
        self.get(THEME_KEY).unwrap_or(theme::DEFAULT_THEME)
    }

    fn is_on(&self, sw: &Switch) -> bool {
        match self.get(sw.key) {
            Some(v) if v == sw.on => true,
            Some(v) if v == sw.off => false,
            _ => sw.default_on,
        }
    }

    /// The key, else what the named preset gives that slot.
    fn slider_value(&self, spec: &Slider) -> f32 {
        let preset = theme::preset_colours(self.theme_id());
        let fallback = match spec.slot {
            Slot::Tint(i) => preset.tint[i],
            Slot::Font(i) => preset.font[i],
            Slot::Border(i) => preset.border[i],
            Slot::TintOpacity => preset.opacities.map_or(spec.default, |o| o[0]),
            Slot::FontOpacity => preset.opacities.map_or(spec.default, |o| o[1]),
            Slot::BorderOpacity => preset.opacities.map_or(spec.default, |o| o[2]),
            Slot::Plain => spec.default,
        };
        self.number(spec.key, fallback)
    }

    fn blur_style(&self) -> usize {
        self.get(BLUR_STYLE_KEY)
            .and_then(|v| BLUR_STYLES.iter().position(|(value, _)| *value == v))
            .unwrap_or(BLUR_STYLE_DEFAULT)
    }

    fn font_name(&self) -> &str {
        self.get(FONT_NAME_KEY).unwrap_or(theme::SYSTEM_FONT)
    }

    // --- Changes -----------------------------------------------------------------
    //
    // Every control edits the working copy and the theme follows it on the
    // same frame; nothing touches the file until Save Config, as the macOS
    // ThemeStore keeps its settings in memory and writes the file on that
    // button alone.

    /// Set a key on the working copy; empty unsets it.
    fn set(&mut self, key: &str, value: impl Into<String>) {
        let value = value.into();
        if value.is_empty() {
            self.entries.remove(key);
        } else {
            self.entries.insert(key.to_string(), value);
        }
    }

    /// The theme every render reads follows the working copy.
    fn apply(&self, cx: &mut Context<Launcher>) {
        let th = Theme::from_entries(&self.entries);
        fonts::ensure_family(cx, &th.font_family);
        theme::install(th);
        cx.notify();
    }

    fn flip(&mut self, sw: &Switch, cx: &mut Context<Launcher>) {
        let on = !self.is_on(sw);
        self.set(sw.key, if on { sw.on } else { sw.off });
        self.apply(cx);
    }

    /// A preset copies its triplets into the keys and keeps its name, so the
    /// sliders tune it from there under its own palette. Custom keeps what
    /// is on screen and drops the name; the palette then derives from the
    /// user's colours.
    fn pick_theme(&mut self, id: &str, cx: &mut Context<Launcher>) {
        self.select = None;
        let colours = theme::preset_colours(id);
        if id == theme::CUSTOM_THEME {
            for spec in SLIDERS.iter().filter(|s| s.slot.is_colour()) {
                let value = self.slider_value(spec);
                self.set(spec.key, format_value(value, spec.decimals));
            }
        } else {
            for (sliders, triplet) in [
                (&TINT, colours.tint),
                (&FONT, colours.font),
                (&BORDER, colours.border),
            ] {
                for (spec, value) in sliders.iter().zip(triplet) {
                    self.set(spec.key, format_value(value, spec.decimals));
                }
            }
            // A preset whose transparency is its look overwrites the user's.
            if let Some(opacities) = colours.opacities {
                for (spec, value) in [&TINT[3], &FONT[3], &BORDER[3]].into_iter().zip(opacities) {
                    self.set(spec.key, format_value(value, spec.decimals));
                }
            }
        }
        self.set(THEME_KEY, id);
        self.set(SURFACE_KEY, if id == LIQUID { LIQUID } else { "" });
        self.apply(cx);
    }

    fn slide(&mut self, spec: &'static Slider, event: &SliderEvent, cx: &mut Context<Launcher>) {
        // A release over a track that was never pressed is someone else's.
        if event.phase != SliderPhase::Start && self.drag != Some(spec.key) {
            return;
        }
        self.set(spec.key, format_value(event.value, spec.decimals));
        self.drag = match event.phase {
            SliderPhase::Start | SliderPhase::Move => Some(spec.key),
            SliderPhase::End => None,
        };
        self.apply(cx);
    }

    fn pick_layout(&mut self, layout: &str, cx: &mut Context<Launcher>) {
        self.set(LAYOUT_KEY, layout);
        self.apply(cx);
    }

    fn pick_blur_style(&mut self, style: &str, cx: &mut Context<Launcher>) {
        self.select = None;
        let at = BLUR_STYLES
            .iter()
            .position(|(value, _)| *value == style)
            .unwrap_or(BLUR_STYLE_DEFAULT);
        self.set(BLUR_STYLE_KEY, style);
        self.set(
            BLUR_OPACITY_KEY,
            format_value(BLUR_STYLE_OPACITY[at], BLUR_OPACITY.decimals),
        );
        self.apply(cx);
    }

    fn open_font_field(&mut self, cx: &mut Context<Launcher>) {
        let text = match self.font_name() {
            theme::SYSTEM_FONT => String::new(),
            name => name.to_string(),
        };
        let input = cx.new(SearchInput::new);
        input.update(cx, |input, cx| {
            input.set_text(&text, cx);
            input.select_all(cx);
        });
        cx.subscribe(&input, |this, _, _: &Changed, cx| {
            if let Some(field) = &mut this.settings.field {
                field.highlighted = None;
            }
            cx.notify();
        })
        .detach();
        self.field = Some(Field {
            input,
            highlighted: None,
        });
        cx.notify();
    }

    /// The installed families containing what the box holds.
    fn suggestions(&self, cx: &Context<Launcher>) -> Vec<String> {
        let Some(field) = &self.field else {
            return Vec::new();
        };
        let needle = field.input.read(cx).text().trim().to_lowercase();
        if needle.is_empty() {
            return Vec::new();
        }
        self.fonts
            .iter()
            .filter(|f| f.to_lowercase().contains(&needle))
            .take(SUGGESTIONS_MAX)
            .cloned()
            .collect()
    }

    fn move_highlight(&mut self, step: isize, cx: &mut Context<Launcher>) -> KeyOutcome {
        let count = self.suggestions(cx).len();
        if let Some(field) = &mut self.field
            && count > 0
        {
            let at = field.highlighted.map_or(-1, |i| i as isize) + step;
            field.highlighted = Some(at.rem_euclid(count as isize) as usize);
            cx.notify();
        }
        KeyOutcome::Consumed
    }

    /// Enter, or a click away: the highlighted family, else the text;
    /// empty means the theme's own font.
    fn commit_font(&mut self, cx: &mut Context<Launcher>) {
        let suggestions = self.suggestions(cx);
        let Some(field) = self.field.take() else {
            return;
        };
        let name = match field.highlighted.and_then(|i| suggestions.get(i)) {
            Some(family) => family.clone(),
            None => field.input.read(cx).committed(),
        };
        self.set_font(name, cx);
    }

    fn set_font(&mut self, name: String, cx: &mut Context<Launcher>) {
        let name = name.trim().to_string();
        self.set(
            FONT_NAME_KEY,
            if name.is_empty() {
                theme::SYSTEM_FONT.to_string()
            } else {
                name
            },
        );
        self.apply(cx);
    }

    /// Save Config: the screen's keys, in one write, then the engine reads
    /// the file again.
    fn save_all(&mut self, cx: &mut Context<Launcher>) {
        self.field = None;
        self.select = None;
        let updates: Vec<ConfigUpdate> = KEYS
            .iter()
            .copied()
            .chain(SLIDERS.iter().map(|s| s.key))
            .map(|key| ConfigUpdate {
                key: key.to_string(),
                value: self.get(key).unwrap_or_default().to_string(),
            })
            .collect();
        bg::fetch(
            cx,
            move || {
                config::set_config(updates)?;
                engine::reload_config(app_state());
                Ok::<(), String>(())
            },
            |this, result, cx| {
                let ok = result.is_ok();
                if let Err(err) = result {
                    this.banner.show(
                        format!("{SAVE_FAILED}: {err}"),
                        Tone::Error,
                        SAVE_FAILED_BANNER_SECS,
                        cx,
                    );
                }
                this.settings.show_saved(ok, cx);
            },
        );
    }

    fn show_saved(&mut self, ok: bool, cx: &mut Context<Launcher>) {
        self.save_seq += 1;
        let seq = self.save_seq;
        self.save_message = Some(SaveMessage {
            text: if ok { SAVED } else { SAVE_FAILED },
            ok,
        });
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(SAVE_MESSAGE_MS))
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.settings.save_seq == seq {
                    this.settings.save_message = None;
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    // --- Render ------------------------------------------------------------------

    pub fn render(&self, th: &Theme, cx: &mut Context<Launcher>) -> Div {
        let body: AnyElement = match self.tab {
            Tab::Appearance => self.appearance(th, cx).into_any_element(),
            Tab::Advanced | Tab::Shortcuts => div()
                .pt(px(HEADER_PADDING_Y))
                .text_color(th.text_muted)
                .child(format!("{} {PENDING_TAB}", self.tab.label()))
                .into_any_element(),
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(self.header(th, cx))
            .child(self.tabs(th, cx))
            .child(
                div()
                    .id("settings-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .px(px(HEADER_PADDING_X))
                    .pb(px(BODY_PADDING_BOTTOM))
                    .flex()
                    .flex_col()
                    .child(body),
            )
    }

    fn header(&self, th: &Theme, cx: &mut Context<Launcher>) -> Div {
        let pill = self.save_message.as_ref().map(|m| {
            div()
                .px(px(PILL_PADDING_X))
                .py(px(PILL_PADDING_Y))
                .rounded_full()
                .bg(if m.ok { th.success } else { th.danger })
                .text_color(th.on_accent)
                .text_size(px(th.font_size - 1.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(m.text)
        });
        div()
            .px(px(HEADER_PADDING_X))
            .py(px(HEADER_PADDING_Y))
            .flex()
            .items_center()
            .gap(px(HEADER_GAP))
            .child(
                div()
                    .flex_1()
                    .text_size(px(th.font_size + TITLE_SIZE_STEP))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(TITLE),
            )
            .children(pill)
            .child(
                controls::button("settings-save", SAVE, th)
                    .on_click(cx.listener(|this, _, _, cx| this.settings.save_all(cx))),
            )
            .child(controls::hint(CLOSE_HINT, th).text_size(px(th.font_size - 1.0)))
    }

    fn tabs(&self, th: &Theme, cx: &mut Context<Launcher>) -> Div {
        div()
            .px(px(HEADER_PADDING_X))
            .pb(px(TABS_PADDING_BOTTOM))
            .flex()
            .gap(px(TABS_GAP))
            .children(Tab::ALL.into_iter().enumerate().map(|(i, tab)| {
                let active = tab == self.tab;
                div()
                    .id(("settings-tab", i))
                    .flex_1()
                    .py(px(TAB_PADDING_Y))
                    .rounded(px(th.control_radius()))
                    .text_center()
                    .text_size(px(th.font_size - 1.0))
                    .font_weight(FontWeight::MEDIUM)
                    .cursor_pointer()
                    .map(|el| {
                        if active {
                            el.bg(th.selection_fill).text_color(th.text)
                        } else {
                            el.bg(th.panel_fill)
                                .text_color(th.text_secondary)
                                .hover(|s| s.bg(th.control_fill).text_color(th.text))
                        }
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.settings.tab = tab;
                        this.settings.scroll.set_offset(Default::default());
                        this.settings.leave();
                        cx.notify();
                    }))
                    .child(tab.label())
            }))
    }

    fn appearance(&self, th: &Theme, cx: &mut Context<Launcher>) -> Div {
        let blur_style = self.blur_style();
        div()
            .flex()
            .flex_col()
            .child(self.header_row(th, cx))
            .child(controls::divider(th))
            .child(controls::section("Layout", th))
            .child(controls::row("Window", th).child(controls::segmented(
                "settings-layout",
                LAYOUTS,
                self.get(LAYOUT_KEY).unwrap_or(LAYOUT_DEFAULT),
                th,
                cx.listener(|this, layout: &str, _, cx| this.settings.pick_layout(layout, cx)),
            )))
            .child(self.slider_row(&INNER_GAP, th, cx))
            .child(self.slider_row(&CORNER_RADIUS, th, cx))
            .child(controls::divider(th))
            .child(controls::section("Tint Color", th))
            .children(TINT.iter().map(|s| self.slider_row(s, th, cx)))
            .child(controls::section("Blur", th))
            .child(self.slider_row(&BLUR_OPACITY, th, cx))
            .child(self.slider_row(&SETTINGS_BLUR, th, cx))
            .child(
                controls::row("Blur Style", th)
                    .child(controls::select(
                        "settings-blur-style",
                        BLUR_STYLES,
                        BLUR_STYLES[blur_style].0,
                        self.select == Some(BLUR_STYLE_KEY),
                        th,
                        cx.listener(|this, open: &bool, _, cx| {
                            this.settings.select = open.then_some(BLUR_STYLE_KEY);
                            cx.notify();
                        }),
                        cx.listener(|this, style: &str, _, cx| {
                            this.settings.pick_blur_style(style, cx)
                        }),
                    ))
                    .child(controls::hint(BLUR_STYLE_HINTS[blur_style], th)),
            )
            .child(controls::section("Font", th))
            .child(self.font_row(th, cx))
            .child(self.slider_row(&FONT_SIZE, th, cx))
            .child(controls::section("Font Color", th))
            .children(FONT.iter().map(|s| self.slider_row(s, th, cx)))
            .child(controls::section("Border", th))
            .child(self.slider_row(&BORDER_THICKNESS, th, cx))
            .children(BORDER.iter().map(|s| self.slider_row(s, th, cx)))
    }

    /// Theme, then the switches; the strip ones only in the split layout,
    /// which is the only one with a strip.
    fn header_row(&self, th: &Theme, cx: &mut Context<Launcher>) -> Div {
        let split = th.split();
        let switch = |sw: &'static Switch| {
            div()
                .flex()
                .items_center()
                .gap(px(HEADER_ITEM_GAP))
                .child(
                    div()
                        .text_size(px(th.font_size - 1.0))
                        .text_color(th.text_secondary)
                        .child(sw.label),
                )
                .child(
                    controls::toggle(sw.id, self.is_on(sw), th)
                        .on_click(cx.listener(move |this, _, _, cx| this.settings.flip(sw, cx))),
                )
        };
        div()
            .py(px(HEADER_PADDING_Y))
            .flex()
            .items_center()
            .gap(px(HEADER_ROW_GAP))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(HEADER_ITEM_GAP))
                    .child(
                        div()
                            .text_size(px(th.font_size - 1.0))
                            .text_color(th.text_secondary)
                            .child("Theme"),
                    )
                    .child(controls::select(
                        "settings-theme",
                        THEMES,
                        self.theme_id(),
                        self.select == Some(THEME_KEY),
                        th,
                        cx.listener(|this, open: &bool, _, cx| {
                            this.settings.select = open.then_some(THEME_KEY);
                            cx.notify();
                        }),
                        cx.listener(|this, id: &str, _, cx| this.settings.pick_theme(id, cx)),
                    )),
            )
            .when(split, |el| el.child(switch(&RUNNING_APPS)))
            .when(split, |el| el.child(switch(&SUPER_ACTIONS)))
            .child(switch(&ANIMATIONS))
    }

    fn slider_row(&self, spec: &'static Slider, th: &Theme, cx: &mut Context<Launcher>) -> Div {
        let value = self.slider_value(spec);
        controls::row(spec.label, th)
            .child(controls::slider(
                value,
                spec.range,
                self.drag == Some(spec.key),
                th,
                cx.listener(move |this, event: &SliderEvent, _, cx| {
                    this.settings.slide(spec, event, cx)
                }),
            ))
            .child(controls::value(format_value(value, spec.decimals), th))
    }

    fn font_row(&self, th: &Theme, cx: &mut Context<Launcher>) -> Div {
        let editing = self.field.as_ref();
        let content: AnyElement = match editing {
            Some(field) => search_field(field.input.clone(), FONT_PLACEHOLDER).into_any_element(),
            None => div()
                .flex_1()
                .overflow_hidden()
                .whitespace_nowrap()
                .child(self.font_name().to_string())
                .into_any_element(),
        };
        let suggestions = self.suggestions(cx);
        let highlighted = editing.and_then(|f| f.highlighted);
        let boxed = controls::text_box("settings-font", FONT_BOX_W, content, editing.is_some(), th)
            .when(editing.is_none(), |el| {
                el.on_click(cx.listener(|this, _, _, cx| this.settings.open_font_field(cx)))
            })
            .when(editing.is_some(), |el| {
                el.on_mouse_down_out(cx.listener(|this, _, _, cx| this.settings.commit_font(cx)))
            });
        let list = (!suggestions.is_empty()).then(|| {
            deferred(
                div()
                    .id("settings-font-suggestions")
                    .occlude()
                    .absolute()
                    .top(relative(1.0))
                    .mt(px(SUGGESTIONS_GAP))
                    .left_0()
                    .w(px(FONT_BOX_W))
                    .p(px(SUGGESTIONS_PADDING))
                    .rounded(px(th.control_radius()))
                    .bg(theme::opaque(th.card_face()))
                    .border(px(1.0))
                    .border_color(th.border)
                    .shadow(th.card_shadow())
                    .flex()
                    .flex_col()
                    .children(suggestions.into_iter().enumerate().map(|(i, family)| {
                        let name = family.clone();
                        div()
                            .id(("settings-font-suggestion", i))
                            .px(px(SUGGESTION_PADDING_X))
                            .py(px(SUGGESTION_PADDING_Y))
                            .rounded(px(th.chip_radius()))
                            .text_size(px(th.font_size - 1.0))
                            .whitespace_nowrap()
                            .overflow_hidden()
                            .cursor_pointer()
                            .when(highlighted == Some(i), |el| el.bg(th.selection_fill))
                            .hover(|s| s.bg(th.selection_fill))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.settings.field = None;
                                this.settings.set_font(name.clone(), cx);
                            }))
                            .child(SharedString::from(family))
                    })),
            )
        });
        controls::row("Font Name", th)
            .child(div().relative().child(boxed).children(list))
            .child(controls::hint(FONT_PLACEHOLDER, th))
    }
}

/// A slider's value as the config and the readout spell it.
fn format_value(value: f32, decimals: usize) -> String {
    format!("{value:.decimals$}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tabs_cycle_both_ways() {
        assert_eq!(Tab::Appearance.step(false), Tab::Advanced);
        assert_eq!(Tab::Shortcuts.step(false), Tab::Appearance);
        assert_eq!(Tab::Appearance.step(true), Tab::Shortcuts);
    }

    #[test]
    fn values_print_as_the_webview_does() {
        assert_eq!(format_value(7.0, 0), "7");
        assert_eq!(format_value(0.5, 2), "0.50");
    }

    #[test]
    fn every_tab_hint_names_a_way_out_or_a_tip() {
        for tab in Tab::ALL {
            assert!(!tab.hint().is_empty());
        }
    }
}
