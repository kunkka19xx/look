//! The Advanced tab: web answers, the background picture, indexing, logs,
//! startup, the config file, and the behind-window frost on Linux. The
//! webview's WebKitGTK switches (GPU compositing, blur fallback) have no
//! meaning on this shell and are not here.

use std::path::PathBuf;

use super::{Field, FieldKind, Settings, Slider, Slot, Switch, unit};
use crate::answers;
use crate::banner;
use crate::commands::KeyOutcome;
use crate::config_list;
use crate::controls::{self, Option_, Range};
use crate::elide::middle_elided;
use crate::launcher::Launcher;
use crate::launchpad::Tone;
use crate::pick;
use crate::search::{Changed, SearchInput, search_field};
use crate::theme::{self, BgLayout, Theme};
use crate::update::Update;
use gpui::prelude::*;
use gpui::{Context, Div, FontWeight, div, px};

const NUMBER_BOX_W: f32 = 80.0;
const ABOUT_PADDING_BOTTOM: f32 = 6.0;
const DIR_GAP_X: f32 = 6.0;
const DIR_GAP_Y: f32 = 4.0;
const DIR_PADDING_X: f32 = 8.0;
const DIR_PADDING_Y: f32 = 3.0;
const DIR_REMOVE: &str = "\u{2715}";
const DIR_REMOVE_PADDING_X: f32 = 6.0;
const DIR_REMOVE_PADDING_Y: f32 = 2.0;
const DIR_LIST_MAX_W: f32 = 520.0;
const BUTTON_GAP: f32 = 8.0;
const NO_BG_IMAGE: &str = "No background image";
const CHOOSE_BG: &str = "Choose Background Image";
const CLEAR_BG: &str = "Clear";
const BG_PROMPT: &str = "Choose";
const DIR_PROMPT: &str = "Add";
const NO_PICKER: &str = "No file dialog";
const FRESH_CONFIG: &str = "Create Fresh Config";
const FRESH_CONFIG_HINT: &str = "Regenerate a fresh default config file";
const AI_HINT: &str = "Inline answers + Google autocomplete over HTTPS";
const AI_NOTE: &str = "On-device AI (model that rewrites queries, drafts answers) is macOS-only.";
const LAZY_HINT: &str = "Refresh index automatically when file/app changes";
const DRIVES_HINT: &str = "Toggle non-system drives to scan them";
const LOG_HINT: &str = "Error only by default; use Info/Debug for troubleshooting";
const LAUNCH_HINT: &str = "Start Look automatically when you sign in";
const PATH_HINT: &str = "Run lookapp from a terminal; open a new one after enabling";

const AI: Switch = Switch {
    id: "settings-ai",
    key: answers::AI_KEY,
    label: "Enable",
    on: "true",
    off: "false",
    default_on: true,
};
const LAZY_INDEXING: Switch = Switch {
    id: "settings-lazy",
    key: "lazy_indexing_enabled",
    label: "Lazy indexing",
    on: "true",
    off: "false",
    default_on: true,
};
/// The value comes from the OS on open (`get_autostart`), merged into the
/// working copy under this key, and goes back to the OS on Save.
pub(super) const LAUNCH_AT_LOGIN: Switch = Switch {
    id: "settings-launch",
    key: "launch_at_login",
    label: "Launch at login",
    on: "true",
    off: "false",
    default_on: true,
};
/// Windows: the same round trip through `cli_path`.
pub(super) const ADD_TO_PATH: Switch = Switch {
    id: "settings-path",
    key: "add_to_path",
    label: "Add to PATH",
    on: "true",
    off: "false",
    default_on: false,
};
const COMPOSITOR_BLUR_ID: &str = "settings-frost";

pub(super) static BG_OPACITY: Slider = unit(
    theme::BG_OPACITY_KEY,
    "Image Opacity",
    theme::DEFAULT_BG_OPACITY,
    Slot::Plain,
);
pub(super) static BG_BLUR: Slider = Slider {
    key: theme::BG_BLUR_KEY,
    label: "Image Blur",
    range: Range {
        min: 0.0,
        max: 30.0,
        step: 0.1,
    },
    default: theme::DEFAULT_BG_BLUR,
    slot: Slot::Plain,
    decimals: 1,
};
pub(super) static SLIDERS: [&Slider; 2] = [&BG_OPACITY, &BG_BLUR];

const BG_LAYOUTS: &[Option_] = &[
    ("center", "Center"),
    ("fill", "Fill"),
    ("stretch", "Stretch"),
    ("duplicate", "Duplicate"),
];
const BG_LAYOUT_HINTS: [&str; 4] = [
    "Original size, centered",
    "Fill area and crop edges",
    "Stretch to fill exactly",
    "Repeat as tiles",
];

const LOG_LEVEL_KEY: &str = "backend_log_level";
const LOG_LEVELS: &[Option_] = &[
    ("error", "Error"),
    ("warn", "Warn"),
    ("info", "Info"),
    ("debug", "Debug"),
];
const LOG_LEVEL_DEFAULT: &str = "error";

/// A whole-number box with its bounds; out of range snaps to the edge, a
/// blank or garbled entry to the default, as the webview's `change` did.
pub(crate) struct Number {
    key: &'static str,
    id: &'static str,
    label: &'static str,
    hint: &'static str,
    min: u32,
    max: u32,
    default: u32,
}

pub(super) static SCAN_DEPTH: Number = Number {
    key: "file_scan_depth",
    id: "settings-scan-depth",
    label: "File Scan Depth",
    hint: "How many directory levels to index",
    min: 1,
    max: 12,
    default: 4,
};
pub(super) static FILE_LIMIT: Number = Number {
    key: "file_scan_limit",
    id: "settings-file-limit",
    label: "File Scan Limit",
    hint: "Max files indexed per refresh",
    min: 500,
    max: 50_000,
    default: 8000,
};

/// A folder list under a config key: the add button, then a chip per path.
pub(super) struct DirList {
    key: &'static str,
    id: &'static str,
    label: &'static str,
    add: &'static str,
    empty: &'static str,
}

static EXTRA_DIRS: DirList = DirList {
    key: "file_scan_extra_roots",
    id: "settings-extra-dirs",
    label: "Extra Scan Dirs",
    add: "Add Directory",
    empty: "No extra scan directories",
};
static SKIP_DIRS: DirList = DirList {
    key: "file_exclude_paths",
    id: "settings-skip-dirs",
    label: "Skip Folders",
    add: "Add Folder",
    empty: "No excluded folder paths yet",
};

/// The keys this tab owns besides its sliders', for Save Config.
pub(super) fn keys() -> Vec<&'static str> {
    let mut keys = vec![
        AI.key,
        theme::BG_IMAGE_KEY,
        theme::BG_LAYOUT_KEY,
        SCAN_DEPTH.key,
        FILE_LIMIT.key,
        LAZY_INDEXING.key,
        EXTRA_DIRS.key,
        SKIP_DIRS.key,
        LOG_LEVEL_KEY,
        LAUNCH_AT_LOGIN.key,
    ];
    if cfg!(windows) {
        keys.push(ADD_TO_PATH.key);
    }
    if cfg!(target_os = "linux") {
        keys.push(theme::COMPOSITOR_BLUR_KEY);
    }
    keys
}

impl Settings {
    // --- Values ------------------------------------------------------------------

    fn number_value(&self, spec: &Number) -> u32 {
        self.get(spec.key)
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(spec.default)
    }

    fn dirs(&self, list: &DirList) -> Vec<String> {
        config_list::parse(self.get(list.key).unwrap_or_default())
    }

    fn bg_image(&self) -> Option<&str> {
        self.get(theme::BG_IMAGE_KEY)
    }

    /// The layout's index in `BG_LAYOUTS`, Fill when unset.
    fn bg_layout(&self) -> usize {
        let layout = self
            .get(theme::BG_LAYOUT_KEY)
            .and_then(BgLayout::parse)
            .unwrap_or_default();
        BgLayout::ALL.iter().position(|l| *l == layout).unwrap_or(0)
    }

    fn log_level(&self) -> &str {
        self.get(LOG_LEVEL_KEY).unwrap_or(LOG_LEVEL_DEFAULT)
    }

    /// Set means what it says; unset is the compositor's default.
    fn compositor_blur_on(&self) -> bool {
        match self.get(theme::COMPOSITOR_BLUR_KEY) {
            Some(set) => set != "false",
            None => theme::compositor_blur_default(),
        }
    }

    /// Whether `ai_enabled` is on in the working copy; the launcher reads
    /// it after Save.
    pub(super) fn ai_on(&self) -> bool {
        self.is_on(&AI)
    }

    pub(super) fn launch_at_login(&self) -> bool {
        self.is_on(&LAUNCH_AT_LOGIN)
    }

    pub(super) fn add_to_path(&self) -> bool {
        self.is_on(&ADD_TO_PATH)
    }

    // --- Changes -----------------------------------------------------------------

    fn set_dirs(&mut self, list: &DirList, dirs: &[String], cx: &mut Context<Launcher>) {
        self.set(list.key, config_list::render(dirs));
        cx.notify();
    }

    fn add_dir(&mut self, list: &'static DirList, cx: &mut Context<Launcher>) {
        let picked = pick::folder(DIR_PROMPT, cx);
        cx.spawn(async move |this, cx| {
            let folder = match picked.await {
                Ok(Some(folder)) => folder,
                Ok(None) => return,
                Err(err) => {
                    let _ = this.update(cx, |this, cx| {
                        this.banner.show(
                            format!("{NO_PICKER}: {err}"),
                            Tone::Error,
                            banner::SHORT,
                            cx,
                        );
                    });
                    return;
                }
            };
            let _ = this.update(cx, |this, cx| this.settings.push_dir(list, folder, cx));
        })
        .detach();
    }

    fn push_dir(&mut self, list: &DirList, folder: PathBuf, cx: &mut Context<Launcher>) {
        let folder = folder.to_string_lossy().into_owned();
        let mut dirs = self.dirs(list);
        if dirs.contains(&folder) {
            return;
        }
        dirs.push(folder);
        self.set_dirs(list, &dirs, cx);
    }

    fn remove_dir(&mut self, list: &DirList, folder: &str, cx: &mut Context<Launcher>) {
        let dirs: Vec<String> = self
            .dirs(list)
            .into_iter()
            .filter(|d| d != folder)
            .collect();
        self.set_dirs(list, &dirs, cx);
    }

    /// A drive chip: its root in or out of the extra scan roots, matched
    /// without case as the webview did.
    fn toggle_drive(&mut self, root: &str, cx: &mut Context<Launcher>) {
        let has = self
            .dirs(&EXTRA_DIRS)
            .iter()
            .any(|d| d.eq_ignore_ascii_case(root));
        if has {
            let dirs: Vec<String> = self
                .dirs(&EXTRA_DIRS)
                .into_iter()
                .filter(|d| !d.eq_ignore_ascii_case(root))
                .collect();
            self.set_dirs(&EXTRA_DIRS, &dirs, cx);
        } else {
            self.push_dir(&EXTRA_DIRS, PathBuf::from(root), cx);
        }
    }

    fn choose_bg(&mut self, cx: &mut Context<Launcher>) {
        let picked = pick::image(BG_PROMPT, cx);
        cx.spawn(async move |this, cx| {
            let image = match picked.await {
                Ok(Some(image)) => image,
                Ok(None) => return,
                Err(err) => {
                    let _ = this.update(cx, |this, cx| {
                        this.banner.show(
                            format!("{NO_PICKER}: {err}"),
                            Tone::Error,
                            banner::SHORT,
                            cx,
                        );
                    });
                    return;
                }
            };
            let _ = this.update(cx, |this, cx| {
                this.settings
                    .set(theme::BG_IMAGE_KEY, image.to_string_lossy().into_owned());
                this.settings.apply(cx);
            });
        })
        .detach();
    }

    fn clear_bg(&mut self, cx: &mut Context<Launcher>) {
        self.set(theme::BG_IMAGE_KEY, "");
        self.apply(cx);
    }

    fn pick_bg_layout(&mut self, layout: &str, cx: &mut Context<Launcher>) {
        self.select = None;
        self.set(theme::BG_LAYOUT_KEY, layout);
        self.apply(cx);
    }

    fn pick_log_level(&mut self, level: &str, cx: &mut Context<Launcher>) {
        self.select = None;
        self.set(LOG_LEVEL_KEY, level);
        cx.notify();
    }

    fn flip_compositor_blur(&mut self, cx: &mut Context<Launcher>) {
        let on = !self.compositor_blur_on();
        self.set(
            theme::COMPOSITOR_BLUR_KEY,
            if on { "true" } else { "false" },
        );
        self.apply(cx);
    }

    fn open_number_field(&mut self, spec: &'static Number, cx: &mut Context<Launcher>) {
        let input = cx.new(SearchInput::new);
        let text = self.number_value(spec).to_string();
        input.update(cx, |input, cx| {
            input.set_text(&text, cx);
            input.select_all(cx);
        });
        cx.subscribe(&input, |_, _, _: &Changed, cx| cx.notify())
            .detach();
        self.field = Some(Field {
            input,
            highlighted: None,
            kind: FieldKind::Number(spec),
        });
        cx.notify();
    }

    /// Enter, or a click away: what the box holds, snapped into range.
    pub(super) fn commit_number(
        &mut self,
        spec: &Number,
        cx: &mut Context<Launcher>,
    ) -> KeyOutcome {
        let Some(field) = self.field.take() else {
            return KeyOutcome::Consumed;
        };
        let typed = field.input.read(cx).committed();
        let value = typed
            .trim()
            .parse::<u32>()
            .unwrap_or(spec.default)
            .clamp(spec.min, spec.max);
        self.set(spec.key, value.to_string());
        cx.notify();
        KeyOutcome::Consumed
    }

    // --- Render ------------------------------------------------------------------

    pub(super) fn advanced(&self, th: &Theme, update: &Update, cx: &mut Context<Launcher>) -> Div {
        div()
            .flex()
            .flex_col()
            .child(controls::section("Web answers", th))
            .child(
                self.switch_row(&AI, th, cx)
                    .child(controls::hint(AI_HINT, th)),
            )
            .child(controls::row("", th).child(controls::hint(AI_NOTE, th)))
            .child(controls::divider(th))
            .child(controls::section("Background", th))
            .child(self.bg_rows(th, cx))
            .child(controls::divider(th))
            .child(controls::section("Indexing", th))
            .child(self.number_row(&SCAN_DEPTH, th, cx))
            .child(self.number_row(&FILE_LIMIT, th, cx))
            .child(
                self.switch_row(&LAZY_INDEXING, th, cx)
                    .child(controls::hint(LAZY_HINT, th)),
            )
            .when(cfg!(windows), |el| el.child(self.drives_rows(th, cx)))
            .child(self.dir_rows(&EXTRA_DIRS, th, cx))
            .child(self.dir_rows(&SKIP_DIRS, th, cx))
            .child(controls::divider(th))
            .child(controls::section("Privacy & Logs", th))
            .child(
                controls::row("Backend Log Level", th)
                    .child(controls::select(
                        "settings-log-level",
                        LOG_LEVELS,
                        self.log_level(),
                        self.select == Some(LOG_LEVEL_KEY),
                        th,
                        cx.listener(|this, open: &bool, _, cx| {
                            this.settings.select = open.then_some(LOG_LEVEL_KEY);
                            cx.notify();
                        }),
                        cx.listener(|this, level: &str, _, cx| {
                            this.settings.pick_log_level(level, cx)
                        }),
                    ))
                    .child(controls::hint(LOG_HINT, th)),
            )
            .child(controls::divider(th))
            .child(controls::section("Startup", th))
            .child(
                self.switch_row(&LAUNCH_AT_LOGIN, th, cx)
                    .child(controls::hint(LAUNCH_HINT, th)),
            )
            .when(cfg!(windows), |el| {
                el.child(
                    self.switch_row(&ADD_TO_PATH, th, cx)
                        .child(controls::hint(PATH_HINT, th)),
                )
            })
            .child(controls::divider(th))
            .child(controls::section("Config File", th))
            .child(
                controls::row("", th)
                    .child(
                        controls::button("settings-fresh-config", FRESH_CONFIG, th)
                            .on_click(cx.listener(|this, _, _, cx| this.fresh_config(cx))),
                    )
                    .child(controls::hint(FRESH_CONFIG_HINT, th)),
            )
            .when(cfg!(target_os = "linux"), |el| {
                el.child(controls::divider(th))
                    .child(controls::section("Rendering", th))
                    .child(self.frost_row(th, cx))
            })
            // About, at the foot of the scroll as on macOS.
            .child(controls::divider(th))
            .child(controls::section("About", th))
            .child(update.about(th, cx).pb(px(ABOUT_PADDING_BOTTOM)))
    }

    fn switch_row(&self, sw: &'static Switch, th: &Theme, cx: &mut Context<Launcher>) -> Div {
        controls::row(sw.label, th).child(
            controls::toggle(sw.id, self.is_on(sw), th)
                .on_click(cx.listener(move |this, _, _, cx| this.settings.flip(sw, cx))),
        )
    }

    fn bg_rows(&self, th: &Theme, cx: &mut Context<Launcher>) -> Div {
        let image = self.bg_image().map(str::to_string);
        let layout = self.bg_layout();
        div()
            .flex()
            .flex_col()
            .child(
                controls::row("", th)
                    .child(
                        controls::button("settings-choose-bg", CHOOSE_BG, th)
                            .on_click(cx.listener(|this, _, _, cx| this.settings.choose_bg(cx))),
                    )
                    .when(image.is_some(), |el| {
                        el.child(
                            controls::button("settings-clear-bg", CLEAR_BG, th)
                                .on_click(cx.listener(|this, _, _, cx| this.settings.clear_bg(cx))),
                        )
                    }),
            )
            .child(
                controls::row("", th).child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .font_family(th.mono_family.clone())
                        .text_size(px(th.font_size - 2.0))
                        .text_color(th.text_muted)
                        .child(middle_elided(
                            image.unwrap_or_else(|| NO_BG_IMAGE.to_string()),
                        )),
                ),
            )
            .child(
                controls::row("Image Layout", th)
                    .child(controls::select(
                        "settings-bg-layout",
                        BG_LAYOUTS,
                        BG_LAYOUTS[layout].0,
                        self.select == Some(theme::BG_LAYOUT_KEY),
                        th,
                        cx.listener(|this, open: &bool, _, cx| {
                            this.settings.select = open.then_some(theme::BG_LAYOUT_KEY);
                            cx.notify();
                        }),
                        cx.listener(|this, layout: &str, _, cx| {
                            this.settings.pick_bg_layout(layout, cx)
                        }),
                    ))
                    .child(controls::hint(BG_LAYOUT_HINTS[layout], th)),
            )
            .child(self.slider_row(&BG_OPACITY, th, cx))
            .child(self.slider_row(&BG_BLUR, th, cx))
    }

    fn number_row(&self, spec: &'static Number, th: &Theme, cx: &mut Context<Launcher>) -> Div {
        let editing = self
            .field
            .as_ref()
            .filter(|f| matches!(f.kind, FieldKind::Number(n) if std::ptr::eq(n, spec)));
        let content = match editing {
            Some(field) => search_field(field.input.clone(), "").into_any_element(),
            None => div()
                .flex_1()
                .text_center()
                .child(self.number_value(spec).to_string())
                .into_any_element(),
        };
        let boxed = controls::text_box(spec.id, NUMBER_BOX_W, content, editing.is_some(), th)
            .when(editing.is_none(), |el| {
                el.on_click(
                    cx.listener(move |this, _, _, cx| this.settings.open_number_field(spec, cx)),
                )
            })
            .when(editing.is_some(), |el| {
                el.on_mouse_down_out(cx.listener(move |this, _, _, cx| {
                    this.settings.commit_number(spec, cx);
                }))
            });
        controls::row(spec.label, th)
            .child(boxed)
            .child(controls::hint(spec.hint, th))
    }

    /// The add button on the label's row, the chips under it at the
    /// label's indent, as the webview's `.settings-dir-list`.
    fn dir_rows(&self, list: &'static DirList, th: &Theme, cx: &mut Context<Launcher>) -> Div {
        let dirs = self.dirs(list);
        let chips: Vec<_> = dirs
            .iter()
            .enumerate()
            .map(|(i, dir)| {
                let path = dir.clone();
                div()
                    .id((list.id, i))
                    .max_w(px(DIR_LIST_MAX_W))
                    .px(px(DIR_PADDING_X))
                    .py(px(DIR_PADDING_Y))
                    .rounded(px(th.chip_radius()))
                    .bg(th.panel_fill)
                    .flex()
                    .items_center()
                    .gap(px(DIR_GAP_X))
                    .child(
                        div()
                            .min_w_0()
                            .font_family(th.mono_family.clone())
                            .text_size(px(th.font_size - 2.0))
                            .text_color(th.text_secondary)
                            .child(middle_elided(dir.clone())),
                    )
                    .child(
                        div()
                            .id(("remove", i))
                            .px(px(DIR_REMOVE_PADDING_X))
                            .py(px(DIR_REMOVE_PADDING_Y))
                            .rounded(px(th.chip_radius()))
                            .text_size(px(th.font_size - 3.0))
                            .text_color(th.text_muted)
                            .cursor_pointer()
                            .hover(|s| s.text_color(th.danger).bg(th.control_fill))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.settings.remove_dir(list, &path, cx)
                            }))
                            .child(DIR_REMOVE),
                    )
            })
            .collect();
        div()
            .flex()
            .flex_col()
            .child(
                controls::row(list.label, th).child(
                    controls::button(list.id, list.add, th).on_click(
                        cx.listener(move |this, _, _, cx| this.settings.add_dir(list, cx)),
                    ),
                ),
            )
            .child(
                div()
                    .pl(px(controls::LABEL_W + BUTTON_GAP))
                    .pb(px(DIR_GAP_Y))
                    .flex()
                    .flex_wrap()
                    .gap_x(px(DIR_GAP_X))
                    .gap_y(px(DIR_GAP_Y))
                    .map(|el| {
                        if chips.is_empty() {
                            el.child(controls::hint(list.empty, th))
                        } else {
                            el.children(chips)
                        }
                    }),
            )
    }

    /// Windows: one chip per fixed drive that is not the system's, filled
    /// while its root is among the extra scan roots.
    fn drives_rows(&self, th: &Theme, cx: &mut Context<Launcher>) -> Div {
        let roots = self.dirs(&EXTRA_DIRS);
        let chips = self.drives.iter().enumerate().map(|(i, drive)| {
            let on = roots.iter().any(|r| r.eq_ignore_ascii_case(&drive.root));
            let root = drive.root.clone();
            div()
                .id(("settings-drive", i))
                .px(px(DIR_PADDING_X))
                .py(px(DIR_PADDING_Y))
                .rounded(px(th.chip_radius()))
                .text_size(px(th.font_size - 2.0))
                .font_weight(FontWeight::SEMIBOLD)
                .cursor_pointer()
                .map(|el| {
                    if on {
                        el.bg(th.accent).text_color(th.on_accent)
                    } else {
                        el.bg(th.panel_fill)
                            .text_color(th.text_secondary)
                            .hover(|s| s.bg(th.selection_fill))
                    }
                })
                .on_click(cx.listener(move |this, _, _, cx| this.settings.toggle_drive(&root, cx)))
                .child(format!("{} ({})", drive.letter, drive.root))
        });
        div()
            .flex()
            .flex_col()
            .child(controls::row("Detected Drives", th).child(controls::hint(DRIVES_HINT, th)))
            .child(
                div()
                    .pl(px(controls::LABEL_W + BUTTON_GAP))
                    .pb(px(DIR_GAP_Y))
                    .flex()
                    .flex_wrap()
                    .gap(px(DIR_GAP_X))
                    .children(chips),
            )
    }

    /// Linux: the frost switch with what each side costs, since the tint
    /// sliders cannot reach a sharp view through while the frost is on.
    fn frost_row(&self, th: &Theme, cx: &mut Context<Launcher>) -> Div {
        controls::row("Blur behind window", th)
            .child(
                controls::toggle(COMPOSITOR_BLUR_ID, self.compositor_blur_on(), th)
                    .on_click(cx.listener(|this, _, _, cx| this.settings.flip_compositor_blur(cx))),
            )
            .child(controls::hint(self.frost_hint(), th))
    }

    fn frost_hint(&self) -> &'static str {
        if swayfx() {
            return if self.compositor_blur_on() {
                "Frosted by swayfx: needs layer_effects \"lookapp\" blur and blur_ignore_transparent"
            } else {
                "Clear glass; card shadows on"
            };
        }
        if !crate::blur::is_supported() {
            return "Your compositor does not offer it; Look stays clear glass";
        }
        let niri = compositor_is("niri");
        match (self.compositor_blur_on(), niri) {
            (true, true) => {
                "Frosted by niri: the wallpaper only, unless a niri rule sets xray false"
            }
            (true, false) => "Frosted by the compositor; turn off to see through",
            (false, true) => {
                "Clear glass. niri blurs the wallpaper only until a niri rule sets xray false"
            }
            (false, false) => "Clear glass: the desktop shows sharp behind the tint",
        }
    }
}

fn swayfx() -> bool {
    #[cfg(target_os = "linux")]
    {
        linows_backend::platform::linux::wm::is_swayfx()
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

fn compositor_is(name: &str) -> bool {
    #[cfg(target_os = "linux")]
    {
        linows_backend::platform::linux::wm::detect_compositor().as_deref() == Some(name)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = name;
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_snap_into_range() {
        let clamp = |typed: &str, spec: &Number| {
            typed
                .trim()
                .parse::<u32>()
                .unwrap_or(spec.default)
                .clamp(spec.min, spec.max)
        };
        assert_eq!(clamp("0", &SCAN_DEPTH), 1);
        assert_eq!(clamp("99", &SCAN_DEPTH), 12);
        assert_eq!(clamp("abc", &SCAN_DEPTH), 4);
        assert_eq!(clamp(" 700 ", &FILE_LIMIT), 700);
        assert_eq!(clamp("", &FILE_LIMIT), 8000);
    }

    #[test]
    fn every_layout_has_a_hint_and_a_label() {
        assert_eq!(BG_LAYOUTS.len(), BG_LAYOUT_HINTS.len());
        for ((value, _), layout) in BG_LAYOUTS.iter().zip(BgLayout::ALL) {
            assert_eq!(*value, layout.key());
        }
    }
}
