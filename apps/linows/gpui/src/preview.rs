//! The preview column: what the selected row is, drawn the way the webview's
//! `preview.js` and macOS `ResultPreviewView` draw it. The data comes from the
//! backend off the UI thread after a short dwell, so arrowing through rows
//! does not read every file on the way.

use std::collections::HashMap;
use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    AnyElement, Context, Div, Entity, EventEmitter, FontWeight, HighlightStyle, Image, ObjectFit,
    ScrollHandle, SharedString, Stateful, StyledText, Task, div, img, prelude::*, px, svg,
};
use linows_backend::highlight::{self, TokenType};
use linows_backend::process::ProcDetail;
use linows_backend::{clipboard, files, launch, process};

use crate::Shell;
use crate::glyphs;
use crate::icons::{self, IconRequest, IconStore};
use crate::modes::Mode;
use crate::rows::{self, Clip, ClipImage, Icon, Open, Process, Row};
use crate::theme::{self, Theme};

/// How long a row stays selected before its file is read; arrowing past it
/// costs nothing. The webview dwells the same on text files.
const DWELL: Duration = Duration::from_millis(120);
/// Previews kept for the session, so bouncing between rows re-reads nothing.
const CACHE_LIMIT: usize = 64;
const HEADER_ICON: f32 = 48.0;
const HEADER_ICON_RADIUS: f32 = 8.0;
const HEADER_GAP: f32 = 10.0;
const HEADER_MARGIN: f32 = 14.0;
const BADGE_PADDING_X: f32 = 8.0;
const BADGE_PADDING_Y: f32 = 2.0;
const LABEL_COLUMN: f32 = 96.0;
const INFO_GAP: f32 = 12.0;
const INFO_PADDING_Y: f32 = 5.0;
const BODY_MARGIN: f32 = 12.0;
const CODE_PADDING: f32 = 10.0;
const CODE_LINE_HEIGHT: f32 = 1.35;
const FOLDER_PADDING: f32 = 4.0;
const FOLDER_ITEM_PADDING_X: f32 = 8.0;
const FOLDER_ITEM_PADDING_Y: f32 = 6.0;
const FOLDER_ITEM_GAP: f32 = 8.0;
const FOLDER_ICON: f32 = 14.0;
const FOLDER_EMPTY_PADDING: f32 = 24.0;
const CARD_GAP: f32 = 14.0;
const CARD_PADDING: f32 = 20.0;
const CARD_ICON: f32 = 40.0;
const TRUNCATED_NOTE: &str = "File truncated at 64 KB";
/// The body scrolls inside itself past this, so the rows under it stay in
/// view: the webview's `calc(100vh - 280px)`, and 260 for a picture.
const BODY_MAX_H: f32 = theme::WINDOW_H - 280.0;
const IMAGE_MAX_H: f32 = theme::WINDOW_H - 260.0;
/// A clip's text card and a copied image, the webview's caps.
const CLIP_CARD_MAX_H: f32 = 200.0;
const CLIP_IMAGE_MAX_H: f32 = 300.0;
const CLIP_LABEL_MARGIN_TOP: f32 = 10.0;
const CLIP_LABEL_MARGIN_BOTTOM: f32 = 4.0;
const DELETE_PADDING_X: f32 = 12.0;
const DELETE_PADDING_Y: f32 = 4.0;
const DELETE_ICON: f32 = 13.0;
const DELETE_GAP: f32 = 4.0;
const HELP_PADDING: f32 = 12.0;
const HELP_GAP: f32 = 10.0;
const IMAGE_MISSING_PADDING: f32 = 24.0;
const IMAGE_MISSING: &str = "The image is no longer on disk";
const CPU_PROMPT: &str = "Enter to measure";
const CPU_MEASURING: &str = "measuring\u{2026}";
const UNAVAILABLE: &str = "unavailable";
const NOT_AVAILABLE: &str = "n/a";

/// The panel's events for the launcher: a clip removed from its history
/// through the header button, which empties the row it came from.
pub struct ClipDeleted;

impl EventEmitter<ClipDeleted> for Preview {}

/// What the panel knows about one row once the backend has answered.
pub enum Content {
    /// A text clip: its counts, the text in a card, when it was captured.
    Clip(Clip),
    /// A copied image, decoded, or gone from disk.
    ClipImage {
        meta: ClipImage,
        title: String,
        image: Option<Arc<Image>>,
    },
    /// A process: its command line and the facts `/proc` gives at once; the
    /// CPU sample waits for Enter.
    Process {
        meta: Process,
        detail: Option<ProcDetail>,
    },
    /// The "How to use" half of an empty clipboard history.
    Help(Mode),
    /// A block's row with nothing on disk: what Enter is about to run.
    Block {
        title: String,
        name: String,
        steps: Vec<String>,
        file: Option<String>,
        preview: Option<Result<String, String>>,
    },
    /// A row with nothing on disk: the calculator or a URL. One centred card
    /// saying what Enter does.
    Card {
        glyph: &'static str,
        title: String,
        subtitle: String,
        hint: &'static str,
    },
    Standard {
        icon: IconRequest,
        glyph: &'static str,
        title: String,
        badge: String,
        /// The size or the item count, beside the badge.
        aside: Option<String>,
        info: Vec<(&'static str, String)>,
        body: Body,
    },
}

pub enum Body {
    None,
    Image(PathBuf),
    Code {
        text: String,
        highlights: Vec<(Range<usize>, TokenType)>,
        truncated: bool,
    },
    Folder {
        path: String,
        items: Vec<(String, bool, Option<u64>)>,
        truncated: bool,
    },
}

pub struct Preview {
    shell: Shell,
    icons: Entity<IconStore>,
    current: Option<(String, Arc<Content>)>,
    cache: HashMap<String, Arc<Content>>,
    /// The row the panel is waiting on; an answer for another is dropped.
    wanted: Option<String>,
    scroll: ScrollHandle,
    /// The body's own scroll, reset per row.
    body_scroll: ScrollHandle,
    /// The CPU reading of the process on screen, by pid, so a sample for a
    /// row the user has left is dropped.
    cpu: Option<(u32, String)>,
    _load: Option<Task<()>>,
    _cpu: Option<Task<()>>,
}

impl Preview {
    pub fn new(shell: Shell, icons: Entity<IconStore>) -> Self {
        Self {
            shell,
            icons,
            current: None,
            cache: HashMap::new(),
            wanted: None,
            scroll: ScrollHandle::new(),
            body_scroll: ScrollHandle::new(),
            cpu: None,
            _load: None,
            _cpu: None,
        }
    }

    /// The tips half of an empty clipboard history.
    pub fn show_help(&mut self, mode: Mode, cx: &mut Context<Self>) {
        self.wanted = None;
        self._load = None;
        self.current = Some((format!("help:{mode:?}"), Arc::new(Content::Help(mode))));
        cx.notify();
    }

    /// `ps"` Enter: sample the previewed process once, on the background
    /// executor; the row stays instant until asked.
    pub fn measure_cpu(&mut self, cx: &mut Context<Self>) {
        let Some((_, content)) = &self.current else {
            return;
        };
        let Content::Process { meta, .. } = content.as_ref() else {
            return;
        };
        let pid = meta.pid;
        self.cpu = Some((pid, CPU_MEASURING.to_string()));
        cx.notify();
        self._cpu = Some(cx.spawn(async move |this, cx| {
            let pct = cx
                .background_executor()
                .spawn(async move { process::process_cpu(pid) })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.cpu.as_ref().is_some_and(|(p, _)| *p == pid) {
                    let text = pct.map_or(NOT_AVAILABLE.to_string(), |p| format!("{p:.1}%"));
                    this.cpu = Some((pid, text));
                    cx.notify();
                }
            });
        }));
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.current = None;
        self.wanted = None;
        self._load = None;
        cx.notify();
    }

    /// Show `row`: at once from the cache, else after the dwell from the
    /// backend. The old preview stays up until the new one lands, so the
    /// column does not blink between rows.
    pub fn show(&mut self, row: &Row, ancestors: String, cx: &mut Context<Self>) {
        // A block row is a different panel at a different depth.
        let key = if crate::actions::is_source_row(&row.id) {
            format!("{}\u{0}{}", row.id, ancestors)
        } else {
            row.id.clone()
        };
        if self.current.as_ref().is_some_and(|(k, _)| *k == key) {
            return;
        }
        if let Some(content) = self.cache.get(&key) {
            self.current = Some((key, content.clone()));
            self.wanted = None;
            self.scroll.set_offset(Default::default());
            self.body_scroll.set_offset(Default::default());
            cx.notify();
            return;
        }
        self.wanted = Some(key.clone());
        let row = row.clone();
        self._load = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(DWELL).await;
            let content = cx
                .background_executor()
                .spawn(async move { Arc::new(load(&row, &ancestors)) })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.wanted.as_deref() != Some(key.as_str()) {
                    return;
                }
                if this.cache.len() >= CACHE_LIMIT {
                    this.cache.clear();
                }
                this.cache.insert(key.clone(), content.clone());
                this.current = Some((key, content));
                this.wanted = None;
                this.scroll.set_offset(Default::default());
                this.body_scroll.set_offset(Default::default());
                cx.notify();
            });
        }));
    }

    pub fn render_in(&mut self, th: &Theme, cx: &mut Context<Self>) -> Stateful<Div> {
        let panel = div()
            .id("preview")
            .size_full()
            .p(px(theme::CONTENT_PADDING))
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .flex()
            .flex_col();
        let Some((_, content)) = self.current.clone() else {
            return panel;
        };
        match content.as_ref() {
            Content::Clip(clip) => {
                let remove = Remove::Clip {
                    timestamp: clip.timestamp,
                    text: clip.text.clone(),
                };
                panel
                    .child(self.clip_header(glyphs::CLIPBOARD, "Clipboard item", remove, th, cx))
                    .child(badge_row(
                        "Clipboard",
                        format!("{} chars  {} lines", clip.chars, clip.lines),
                        th,
                    ))
                    .child(clip_label("Preview", th))
                    .child(
                        div()
                            .id("clip-text")
                            .max_h(px(CLIP_CARD_MAX_H))
                            .overflow_y_scroll()
                            .track_scroll(&self.body_scroll)
                            .p(px(CODE_PADDING))
                            .rounded(px(th.control_radius()))
                            .bg(th.control_fill)
                            .font_family(th.mono_family.clone())
                            .text_size(px(th.font_size - 1.0))
                            .child(SharedString::from(clip.text.clone())),
                    )
                    .child(info_rows(
                        &[("Captured", rows::format_medium_date(clip.timestamp))],
                        th,
                    ))
            }
            Content::ClipImage { meta, title, image } => {
                let remove = Remove::Image {
                    hash: meta.hash.clone(),
                };
                let picture = match image {
                    Some(image) => div()
                        .mb(px(CLIP_LABEL_MARGIN_TOP))
                        .max_h(px(CLIP_IMAGE_MAX_H + 2.0 * CODE_PADDING))
                        .p(px(CODE_PADDING))
                        .rounded(px(th.control_radius()))
                        .bg(th.control_fill)
                        .flex()
                        .justify_center()
                        .child(
                            img(image.clone())
                                .max_w_full()
                                .max_h(px(CLIP_IMAGE_MAX_H))
                                .object_fit(ObjectFit::Contain)
                                .rounded(px(th.chip_radius())),
                        ),
                    None => div()
                        .mb(px(CLIP_LABEL_MARGIN_TOP))
                        .py(px(IMAGE_MISSING_PADDING))
                        .rounded(px(th.control_radius()))
                        .bg(th.control_fill)
                        .text_size(px(th.font_size - 1.0))
                        .text_color(th.text_muted)
                        .text_center()
                        .child(IMAGE_MISSING),
                };
                panel
                    .child(self.clip_header(glyphs::IMAGE, title, remove, th, cx))
                    .child(badge_row(
                        "Image",
                        format!(
                            "{} \u{d7} {}  {}",
                            meta.width,
                            meta.height,
                            format_size(meta.bytes as u64)
                        ),
                        th,
                    ))
                    .child(picture)
                    .child(info_rows(
                        &[("Captured", rows::format_medium_date(meta.timestamp))],
                        th,
                    ))
            }
            Content::Process { meta, detail } => {
                let picture = meta.icon_path.as_ref().and_then(|path| {
                    self.icons.update(cx, |store, cx| {
                        store.get(
                            IconRequest {
                                kind: "app".into(),
                                path: path.clone(),
                                id: None,
                            },
                            cx,
                        )
                    })
                });
                let cmdline = match detail {
                    Some(d) if !d.cmdline.is_empty() => d.cmdline.clone(),
                    _ => meta.name.clone(),
                };
                let fact = |f: fn(&ProcDetail) -> String| match detail {
                    Some(d) => f(d),
                    None => UNAVAILABLE.to_string(),
                };
                let cpu = match &self.cpu {
                    Some((pid, text)) if *pid == meta.pid => text.clone(),
                    _ => CPU_PROMPT.to_string(),
                };
                let mut info: Vec<(&'static str, String)> = vec![
                    (
                        "Memory",
                        fact(|d| {
                            if d.rss_kb > 0 {
                                format_size(d.rss_kb * 1024)
                            } else {
                                NOT_AVAILABLE.to_string()
                            }
                        }),
                    ),
                    (
                        "User",
                        fact(|d| {
                            if d.user.is_empty() {
                                NOT_AVAILABLE.to_string()
                            } else {
                                d.user.clone()
                            }
                        }),
                    ),
                    ("Parent PID", fact(|d| d.ppid.to_string())),
                    ("Started", fact(|d| format_start(d.start_epoch))),
                    ("CPU", cpu),
                ];
                if !meta.ports.is_empty() {
                    let ports: Vec<String> = meta.ports.iter().map(|p| format!(":{p}")).collect();
                    info.push(("Ports", ports.join("  ")));
                }
                panel
                    .child(header(
                        picture,
                        glyphs::CPU,
                        &meta.name,
                        "Process",
                        Some(&format!("PID {}", meta.pid)),
                        th,
                    ))
                    .child(clip_label("Command", th))
                    .child(
                        div()
                            .id("proc-cmd")
                            .max_h(px(CLIP_CARD_MAX_H))
                            .overflow_y_scroll()
                            .track_scroll(&self.body_scroll)
                            .p(px(CODE_PADDING))
                            .rounded(px(th.control_radius()))
                            .bg(th.control_fill)
                            .font_family(th.mono_family.clone())
                            .text_size(px(th.font_size - 1.0))
                            .child(SharedString::from(cmdline)),
                    )
                    .child(info_rows(&info, th))
            }
            Content::Help(mode) => panel.child(help(*mode, th)),
            Content::Block {
                title,
                name,
                steps,
                file,
                preview,
            } => {
                let mut panel = panel
                    .child(header(None, glyphs::ZAP, title, name, None, th))
                    .when(!steps.is_empty(), |el| {
                        el.child(clip_label("Enter runs", th)).child(
                            div()
                                .p(px(CODE_PADDING))
                                .rounded(px(th.control_radius()))
                                .bg(th.control_fill)
                                .font_family(th.mono_family.clone())
                                .text_size(px(th.font_size - 2.0))
                                .flex()
                                .flex_col()
                                .children(steps.iter().map(|step| div().child(step.clone()))),
                        )
                    });
                if let Some(preview) = preview {
                    let (text, colour) = match preview {
                        Ok(text) => (text.clone(), th.text_secondary),
                        Err(err) => (err.clone(), th.danger),
                    };
                    panel = panel.child(clip_label("Preview", th)).child(
                        div()
                            .id("block-preview")
                            .max_h(px(CLIP_CARD_MAX_H))
                            .overflow_y_scroll()
                            .track_scroll(&self.body_scroll)
                            .p(px(CODE_PADDING))
                            .rounded(px(th.control_radius()))
                            .bg(th.control_fill)
                            .font_family(th.mono_family.clone())
                            .text_size(px(th.font_size - 2.0))
                            .text_color(colour)
                            .child(SharedString::from(text)),
                    );
                }
                let mut info: Vec<(&'static str, String)> = Vec::new();
                if let Some(file) = file {
                    info.push(("Declared in", file.clone()));
                }
                panel.child(info_rows(&info, th))
            }
            Content::Card {
                glyph,
                title,
                subtitle,
                hint,
            } => panel.child(card(glyph, title, subtitle, hint, th)),
            Content::Standard {
                icon,
                glyph,
                title,
                badge,
                aside,
                info,
                body,
            } => {
                let picture = self.icons.update(cx, |store, cx| {
                    store.get(
                        IconRequest {
                            kind: icon.kind.clone(),
                            path: icon.path.clone(),
                            id: icon.id.clone(),
                        },
                        cx,
                    )
                });
                let shell = self.shell.clone();
                panel
                    .child(header(picture, glyph, title, badge, aside.as_deref(), th))
                    .child(self.body(body, th, shell))
                    .child(info_rows(info, th))
            }
        }
    }

    /// The clipboard header: glyph, title and the Delete button that takes
    /// the row out of its history.
    fn clip_header(
        &self,
        glyph: &'static str,
        title: &str,
        remove: Remove,
        th: &Theme,
        cx: &mut Context<Self>,
    ) -> Div {
        let icon = div()
            .size(px(HEADER_ICON))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(HEADER_ICON_RADIUS))
            .bg(th.control_fill)
            .child(
                svg()
                    .path(glyph)
                    .size(px(HEADER_ICON / 2.0))
                    .text_color(th.text_secondary),
            );
        let delete = div()
            .id("clip-delete")
            .flex_shrink_0()
            .px(px(DELETE_PADDING_X))
            .py(px(DELETE_PADDING_Y))
            .rounded(px(th.chip_radius()))
            .bg(th.danger)
            .text_color(th.on_accent)
            .text_size(px(th.font_size - 2.0))
            .font_weight(FontWeight::SEMIBOLD)
            .flex()
            .items_center()
            .gap(px(DELETE_GAP))
            .cursor_pointer()
            .hover(|s| s.opacity(0.9))
            .on_click(cx.listener(move |this, _, _, cx| {
                let remove = remove.clone();
                cx.spawn(async move |this, cx| {
                    let removed = cx
                        .background_executor()
                        .spawn(async move {
                            match remove {
                                Remove::Clip { timestamp, text } => {
                                    clipboard::delete_clipboard_entry(timestamp, &text)
                                }
                                Remove::Image { hash } => clipboard::delete_clipboard_image(&hash),
                            }
                        })
                        .await;
                    if removed {
                        let _ = this.update(cx, |this, cx| {
                            this.clear(cx);
                            cx.emit(ClipDeleted);
                        });
                    }
                })
                .detach();
                let _ = this;
            }))
            .child(
                svg()
                    .path(glyphs::TRASH)
                    .size(px(DELETE_ICON))
                    .text_color(th.on_accent),
            )
            .child("Delete");
        div()
            .flex()
            .items_center()
            .gap(px(HEADER_GAP))
            .mb(px(HEADER_MARGIN))
            .child(icon)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .font_weight(FontWeight::SEMIBOLD)
                    .truncate()
                    .child(SharedString::from(title.to_string())),
            )
            .child(delete)
    }

    fn body(&self, body: &Body, th: &Theme, shell: Shell) -> AnyElement {
        match body {
            Body::None => div().into_any_element(),
            Body::Image(path) => div()
                .mt(px(BODY_MARGIN))
                .flex()
                .justify_center()
                .child(
                    img(path.clone())
                        .max_w_full()
                        .max_h(px(IMAGE_MAX_H))
                        .object_fit(ObjectFit::Contain)
                        .rounded(px(th.chip_radius())),
                )
                .into_any_element(),
            Body::Code {
                text,
                highlights,
                truncated,
            } => {
                let runs = highlights
                    .iter()
                    .map(|(range, token)| {
                        (
                            range.clone(),
                            HighlightStyle {
                                color: Some(theme::hsla_of(th.syntax(*token))),
                                ..Default::default()
                            },
                        )
                    })
                    .collect::<Vec<_>>();
                div()
                    .mt(px(BODY_MARGIN))
                    .max_h(px(BODY_MAX_H))
                    .rounded(px(th.control_radius()))
                    .bg(th.control_fill)
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .id("code")
                            .min_h_0()
                            .overflow_y_scroll()
                            .track_scroll(&self.body_scroll)
                            .p(px(CODE_PADDING))
                            .font_family(th.mono_family.clone())
                            .text_size(px(th.font_size - 2.0))
                            .line_height(px((th.font_size - 2.0) * CODE_LINE_HEIGHT))
                            .text_color(th.text_secondary)
                            .child(StyledText::new(text.clone()).with_highlights(runs)),
                    )
                    .when(*truncated, |el| {
                        el.child(
                            div()
                                .py(px(INFO_PADDING_Y))
                                .border_t(px(1.0))
                                .border_color(th.border)
                                .text_size(px(th.font_size - 2.0))
                                .text_color(th.text_muted)
                                .text_center()
                                .child(TRUNCATED_NOTE),
                        )
                    })
                    .into_any_element()
            }
            Body::Folder {
                path,
                items,
                truncated,
            } => {
                if items.is_empty() {
                    return div()
                        .mt(px(BODY_MARGIN))
                        .py(px(FOLDER_EMPTY_PADDING))
                        .text_size(px(th.font_size - 1.0))
                        .text_color(th.text_muted)
                        .text_center()
                        .child("Empty folder")
                        .into_any_element();
                }
                let separator = path_separator(path);
                let mut list = div()
                    .id("folder-list")
                    .mt(px(BODY_MARGIN))
                    .max_h(px(BODY_MAX_H))
                    .overflow_y_scroll()
                    .track_scroll(&self.body_scroll)
                    .p(px(FOLDER_PADDING))
                    .rounded(px(th.control_radius()))
                    .bg(th.control_fill)
                    .flex()
                    .flex_col();
                let mut folders_done = false;
                for (i, (name, is_dir, size)) in items.iter().enumerate() {
                    if !is_dir && !folders_done && i > 0 {
                        folders_done = true;
                        list = list.child(
                            div()
                                .h(px(1.0))
                                .mx(px(FOLDER_ITEM_PADDING_X))
                                .my(px(FOLDER_PADDING))
                                .bg(th.border),
                        );
                    }
                    let item_path = format!("{path}{separator}{name}");
                    let kind = if *is_dir { "folder" } else { "file" };
                    let shell = shell.clone();
                    list = list.child(
                        div()
                            .id(("folder-item", i))
                            .flex()
                            .items_center()
                            .gap(px(FOLDER_ITEM_GAP))
                            .px(px(FOLDER_ITEM_PADDING_X))
                            .py(px(FOLDER_ITEM_PADDING_Y))
                            .rounded(px(th.chip_radius()))
                            .hover(|el| el.bg(th.selection_fill))
                            .cursor_pointer()
                            .on_click(move |_, _, _| {
                                if let Err(err) =
                                    launch::open_path(&shell, item_path.clone(), Some(kind), None)
                                {
                                    eprintln!("open {item_path}: {err}");
                                }
                            })
                            .child(
                                svg()
                                    .path(if *is_dir {
                                        rows::GLYPH_FOLDER
                                    } else {
                                        rows::GLYPH_FILE
                                    })
                                    .size(px(FOLDER_ICON))
                                    .text_color(th.text_muted),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_size(px(th.font_size - 1.0))
                                    .text_color(th.text_secondary)
                                    .truncate()
                                    .child(name.clone()),
                            )
                            .when_some(*size, |el, size| {
                                el.child(
                                    div()
                                        .text_size(px(th.font_size - 2.0))
                                        .text_color(th.text_muted)
                                        .child(format_size(size)),
                                )
                            }),
                    );
                }
                let _ = truncated;
                list.into_any_element()
            }
        }
    }
}

/// The row's whole description, read on the background executor.
fn load(row: &Row, ancestors: &str) -> Content {
    if crate::actions::is_source_row(&row.id) && row.path.is_empty() {
        let args = crate::actions::row_args(row, ancestors);
        let detail = linows_backend::sources::source_block(args);
        let preview = detail.as_ref().filter(|d| d.has_preview).and_then(|_| {
            linows_backend::sources::source_preview(crate::actions::row_args(row, ancestors)).map(
                |p| match p.error {
                    Some(err) => Err(err),
                    None => Ok(p.text),
                },
            )
        });
        let home = crate::blocks::get().home.clone();
        let tilde = |path: String| match &home {
            Some(home) if path.starts_with(&format!("{home}/")) => {
                format!("~{}", &path[home.len()..])
            }
            _ => path,
        };
        return Content::Block {
            title: row.title.clone(),
            name: detail
                .as_ref()
                .map(|d| d.name.clone())
                .unwrap_or_else(|| row.title.clone()),
            steps: detail.as_ref().map(|d| d.steps.clone()).unwrap_or_default(),
            file: detail.and_then(|d| d.file).map(tilde),
            preview,
        };
    }
    match &row.open {
        Open::Calc { expr, .. } => Content::Card {
            glyph: rows::GLYPH_CALC,
            title: row.title.clone(),
            subtitle: expr.clone(),
            hint: "Press Enter to copy",
        },
        Open::Url(url) => Content::Card {
            glyph: rows::GLYPH_GLOBE,
            title: url.clone(),
            subtitle: row.context.clone(),
            hint: "Press Enter to open in browser",
        },
        Open::Path { .. } => standard(row),
        Open::Clip(clip) => Content::Clip(clip.clone()),
        Open::ClipImage(meta) => Content::ClipImage {
            meta: meta.clone(),
            title: row.title.clone(),
            image: clipboard::clipboard_image_data_url(&meta.hash)
                .as_deref()
                .and_then(icons::decode_data_url)
                .map(Arc::new),
        },
        Open::Process(meta) => Content::Process {
            meta: meta.clone(),
            detail: process::process_detail(meta.pid),
        },
        // Menu rows show nothing; the launcher hides the column for them.
        Open::Prefix(_) | Open::Command(_) => Content::Card {
            glyph: glyphs::SEARCH,
            title: row.title.clone(),
            subtitle: row.context.clone(),
            hint: "",
        },
        Open::WebSuggestion(text) => Content::Card {
            glyph: glyphs::SEARCH,
            title: text.clone(),
            subtitle: rows::WEB_SUGGEST_SUBTITLE.to_string(),
            hint: "Press Enter to search the web",
        },
    }
}

/// Which history a Delete button acts on.
#[derive(Clone)]
enum Remove {
    Clip { timestamp: u64, text: String },
    Image { hash: String },
}

/// The clipboard badge beside its counts, under the header.
fn badge_row(kind: &str, facts: String, th: &Theme) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(HEADER_GAP - 4.0))
        .child(
            div()
                .px(px(BADGE_PADDING_X))
                .py(px(BADGE_PADDING_Y))
                .rounded(px(th.chip_radius() / 1.5))
                .bg(th.panel_fill)
                .text_size(px(th.font_size - 4.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(kind.to_uppercase()),
        )
        .child(
            div()
                .text_size(px(th.font_size - 2.0))
                .text_color(th.text_secondary)
                .child(facts),
        )
}

fn clip_label(text: &'static str, th: &Theme) -> Div {
    div()
        .mt(px(CLIP_LABEL_MARGIN_TOP))
        .mb(px(CLIP_LABEL_MARGIN_BOTTOM))
        .text_size(px(th.font_size - 1.0))
        .text_color(th.text_muted)
        .child(text)
}

/// The "How to use" tips of an empty history, the right half of its empty
/// state; the list card shows the left half.
fn help(mode: Mode, th: &Theme) -> Div {
    let tips: &[&str] = match mode {
        Mode::ClipboardImage => &[
            "Type ci\" to list the latest images",
            "Type ci\"firefox to filter by name",
            "Press Enter to copy the selected image",
        ],
        _ => &[
            "Type c\" to list latest 10 clips",
            "Type c\"mail to filter",
            "Press Enter to copy selected item",
        ],
    };
    div()
        .p(px(HELP_PADDING))
        .flex()
        .flex_col()
        .gap(px(HELP_GAP))
        .child(div().font_weight(FontWeight::SEMIBOLD).child("How to use"))
        .children(tips.iter().map(|tip| {
            div()
                .text_size(px(th.font_size - 1.0))
                .text_color(th.text_secondary)
                .child(format!("\u{2022} {tip}"))
        }))
}

fn format_start(epoch: Option<u64>) -> String {
    use chrono::TimeZone;
    match epoch.and_then(|e| chrono::Local.timestamp_opt(e as i64, 0).single()) {
        Some(t) => t.format("%b %-d, %H:%M").to_string(),
        None => NOT_AVAILABLE.to_string(),
    }
}

fn standard(row: &Row) -> Content {
    let glyph = match &row.icon {
        Icon::Resolve { glyph } | Icon::Glyph(glyph) => *glyph,
        Icon::File(_) => glyphs::IMAGE,
        Icon::Text(_) => glyphs::ZAP,
    };
    let icon = IconRequest {
        kind: row.kind.clone(),
        path: row.path.clone(),
        id: Some(row.id.clone()),
    };
    let badge = badge_label(&row.kind).to_string();
    let mut info: Vec<(&'static str, String)> = Vec::new();
    let mut aside = None;
    let mut body = Body::None;

    if row.kind == "app" {
        if let Some(version) = files::get_app_version(&row.path) {
            info.push(("Version", version));
        }
        let label = if is_command_line(&row.path) {
            "Command"
        } else {
            "Path"
        };
        info.push((label, row.path.clone()));
    } else {
        let meta = files::get_file_meta(&row.path);
        if let Some(size) = meta.size {
            aside = Some(format_size(size));
        }
        info.push(("Path", row.path.clone()));
        if let Some(modified) = meta.modified {
            info.push(("Modified", modified));
        }
        if row.kind == "folder" || meta.is_dir {
            if let Some(listing) = files::list_folder(&row.path) {
                let mut parts = Vec::new();
                if listing.folder_count > 0 {
                    parts.push(plural(listing.folder_count, "folder"));
                }
                if listing.file_count > 0 {
                    parts.push(plural(listing.file_count, "file"));
                }
                if !parts.is_empty() {
                    aside = Some(parts.join(", "));
                }
                body = Body::Folder {
                    path: row.path.clone(),
                    items: listing
                        .items
                        .into_iter()
                        .map(|e| (e.name, e.is_dir, e.size))
                        .collect(),
                    truncated: listing.truncated,
                };
            }
        } else if meta.is_image {
            body = Body::Image(PathBuf::from(&row.path));
        } else if let Some(runs) = highlight::highlight_runs(&row.path) {
            body = Body::Code {
                text: runs.text,
                highlights: runs
                    .spans
                    .into_iter()
                    .map(|s| (s.start..s.end, s.token))
                    .collect(),
                truncated: runs.truncated,
            };
        }
    }
    if let Some(at) = row.last_used {
        info.push(("Last used", format_last_used(at)));
    }

    Content::Standard {
        icon,
        glyph,
        title: row.title.clone(),
        badge,
        aside,
        info,
        body,
    }
}

fn header(
    picture: Option<Arc<gpui::Image>>,
    glyph: &'static str,
    title: &str,
    badge: &str,
    aside: Option<&str>,
    th: &Theme,
) -> Div {
    let icon = div()
        .size(px(HEADER_ICON))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(HEADER_ICON_RADIUS))
        .bg(th.control_fill)
        .child(match picture {
            Some(image) => img(image)
                .size(px(HEADER_ICON))
                .object_fit(ObjectFit::Contain)
                .into_any_element(),
            None => svg()
                .path(glyph)
                .size(px(HEADER_ICON / 2.0))
                .text_color(th.text_secondary)
                .into_any_element(),
        });
    let badge_el = div()
        .px(px(BADGE_PADDING_X))
        .py(px(BADGE_PADDING_Y))
        .rounded(px(th.chip_radius() / 1.5))
        .bg(th.panel_fill)
        .text_size(px(th.font_size - 4.0))
        .font_weight(FontWeight::SEMIBOLD)
        .child(badge.to_uppercase());
    div()
        .flex()
        .items_center()
        .gap(px(HEADER_GAP))
        .mb(px(HEADER_MARGIN))
        .child(icon)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(BADGE_PADDING_Y * 2.0))
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(SharedString::from(title.to_string())),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(HEADER_GAP - 4.0))
                        .child(badge_el)
                        .when_some(aside, |el, aside| {
                            el.child(
                                div()
                                    .text_size(px(th.font_size - 2.0))
                                    .text_color(th.text_muted)
                                    .child(SharedString::from(aside.to_string())),
                            )
                        }),
                ),
        )
}

fn info_rows(info: &[(&'static str, String)], th: &Theme) -> Div {
    let mut rows_el = div().mt(px(HEADER_MARGIN)).flex().flex_col();
    for (label, value) in info {
        rows_el = rows_el.child(
            div()
                .flex()
                .items_baseline()
                .gap(px(INFO_GAP))
                .py(px(INFO_PADDING_Y))
                .child(
                    div()
                        .w(px(LABEL_COLUMN))
                        .flex_shrink_0()
                        .text_size(px(th.font_size - 1.0))
                        .text_color(th.text_muted)
                        .child(*label),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(px(th.font_size - 1.0))
                        .text_color(th.text_secondary)
                        .truncate()
                        .child(value.clone()),
                ),
        );
    }
    rows_el
}

/// A centred "what Enter does" card, the web-suggestion layout the webview
/// shares between the calculator and URL rows.
fn card(glyph: &'static str, title: &str, subtitle: &str, hint: &'static str, th: &Theme) -> Div {
    div()
        .flex_1()
        .p(px(CARD_PADDING))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(CARD_GAP))
        .text_center()
        .child(svg().path(glyph).size(px(CARD_ICON)).text_color(th.accent))
        .child(
            div()
                .text_size(px(th.font_size + 3.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(SharedString::from(title.to_string())),
        )
        .child(
            div()
                .text_color(th.text_muted)
                .child(SharedString::from(subtitle.to_string())),
        )
        .child(
            div()
                .text_size(px(th.font_size - 1.0))
                .text_color(th.text_muted)
                .child(hint),
        )
}

fn badge_label(kind: &str) -> &str {
    match kind {
        "app" => "App",
        "file" => "File",
        "folder" => "Folder",
        "setting" => "Setting",
        other => other,
    }
}

/// Whether an app row's `path` is a command line, as Linux Exec lines are.
fn is_command_line(value: &str) -> bool {
    let filesystem = value.starts_with(['~', '/'])
        || (value.len() > 2
            && value.as_bytes()[0].is_ascii_alphabetic()
            && &value[1..2] == ":"
            && value[2..].starts_with(['\\', '/']));
    !filesystem
        || value
            .split_whitespace()
            .skip(1)
            .any(|arg| arg.starts_with('-') || (arg.len() == 2 && arg.starts_with('%')))
}

fn path_separator(path: &str) -> char {
    if path.contains('\\') { '\\' } else { '/' }
}

fn plural(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

/// Shared with the ci" rows, which say how big a copied image is.
pub fn format_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    let b = bytes as f64;
    if b < KB {
        format!("{bytes} B")
    } else if b < KB * KB {
        format!("{:.1} KB", b / KB)
    } else if b < KB * KB * KB {
        format!("{:.1} MB", b / (KB * KB))
    } else {
        format!("{:.2} GB", b / (KB * KB * KB))
    }
}

fn format_last_used(epoch: i64) -> String {
    use chrono::TimeZone;
    match chrono::Local.timestamp_opt(epoch, 0) {
        chrono::LocalResult::Single(t) => t.format("%b %-d, %Y").to_string(),
        _ => String::new(),
    }
}
