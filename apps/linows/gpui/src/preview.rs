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
    AnyElement, Context, Div, Entity, EventEmitter, FontWeight, HighlightStyle, Hsla,
    ListAlignment, ListState, ObjectFit, ScrollHandle, SharedString, Stateful, StyledText, Task,
    div, img, list, prelude::*, px, relative, svg,
};
use linows_backend::highlight::{self, TokenType};
use linows_backend::process::ProcDetail;
use linows_backend::{clipboard, clipimage, files, launch, process};

use crate::Shell;
use crate::elide::middle_elided;
use crate::glyphs;
use crate::icons::{IconRequest, IconStore};
use crate::imagecache::PreviewImages;
use crate::launchpad::Notice;
use crate::modes::Mode;
use crate::qactions::QuickActions;
use crate::rows::{self, Clip, ClipImage, Icon, Open, Process, Row};
use crate::theme::{self, Theme};

/// How long a row stays selected before its file is read; arrowing past it
/// costs nothing. The webview dwells the same on text files. Rows that read
/// no file skip it, see `reads_file`.
const DWELL: Duration = Duration::from_millis(120);
/// Previews kept for the session, so bouncing between rows re-reads nothing.
const CACHE_LIMIT: usize = 64;
/// Past this many lines, or bytes, a text body is a list of its lines and
/// only those in view are laid out. As one text, a 64 KB file is shaped on
/// the UI thread the frame it lands and walked glyph by glyph on every
/// frame the launcher animates, which on a laptop CPU stalls the pill
/// mid-glide. Shorter bodies stay one text, sized to their content.
const LONG_TEXT_LINES: usize = 80;
const LONG_TEXT_BYTES: usize = 8 * 1024;
/// A minified file is one line, so a longer line is cut into list items
/// this long, each wrapping on its own.
const LINE_PIECE_BYTES: usize = 512;
/// How far past the view a text list lays out lines, so a scroll lands on
/// some already shaped.
const TEXT_OVERDRAW: f32 = 200.0;
const HEADER_ICON: f32 = 48.0;
const HEADER_ICON_RADIUS: f32 = 8.0;
const HEADER_GAP: f32 = 10.0;
const HEADER_MARGIN: f32 = 14.0;
const BADGE_PADDING_X: f32 = 8.0;
const BADGE_PADDING_Y: f32 = 2.0;
const INFO_GAP: f32 = 12.0;
const INFO_ROW_GAP: f32 = 12.0;
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
/// The webview's cap for a picture; text bodies fill the column instead.
fn image_max_h() -> f32 {
    theme::window_h() - 260.0
}
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
pub const FACT_SEP: &str = " \u{b7} ";
const CPU_PROMPT: &str = "Enter to measure";
const CPU_MEASURING: &str = "measuring\u{2026}";
const UNAVAILABLE: &str = "unavailable";
const NOT_AVAILABLE: &str = "n/a";

/// The panel's events for the launcher: a clip removed from its history
/// through the header button, which empties the row it came from.
pub struct ClipDeleted;

impl EventEmitter<ClipDeleted> for Preview {}
impl EventEmitter<Notice> for Preview {}

/// What the panel knows about one row once the backend has answered.
pub enum Content {
    /// A text clip: its counts, the text in a card, when it was captured.
    Clip { clip: Clip, text: TextBody },
    /// A copied image by its file, or gone from disk.
    ClipImage {
        meta: ClipImage,
        title: String,
        image: Option<PathBuf>,
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
        /// The row came from a block the user declared.
        declared: bool,
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
        text: TextBody,
        truncated: bool,
    },
    Folder {
        path: String,
        items: Vec<(String, bool, Option<u64>)>,
        truncated: bool,
    },
}

/// A body's text: whole while short, by lines once long, see
/// `LONG_TEXT_LINES`.
pub enum TextBody {
    Whole {
        text: SharedString,
        highlights: Vec<(Range<usize>, TokenType)>,
    },
    Lines(Arc<[TextLine]>),
}

/// One item of a long text: a line, or a piece of a longer one, its
/// highlights relative to it.
pub struct TextLine {
    text: SharedString,
    highlights: Vec<(Range<usize>, TokenType)>,
}

/// The size and colour a text body is drawn in.
#[derive(Clone, Copy)]
struct TextLook {
    size: f32,
    line_height: Option<f32>,
    color: Option<Hsla>,
}

pub struct Preview {
    shell: Shell,
    icons: Entity<IconStore>,
    /// The pictures on screen lately, see `imagecache`.
    images: Entity<PreviewImages>,
    current: Option<(String, Arc<Content>)>,
    cache: HashMap<String, Arc<Content>>,
    /// The row the panel is waiting on; an answer for another is dropped.
    wanted: Option<String>,
    /// The body's own scroll, reset per row.
    body_scroll: ScrollHandle,
    /// A long text body's lines, and the body they were last reset for.
    lines: ListState,
    lines_for: Option<Arc<[TextLine]>>,
    /// The CPU reading of the process on screen, by pid, so a sample for a
    /// row the user has left is dropped.
    cpu: Option<(u32, String)>,
    _load: Option<Task<()>>,
    _cpu: Option<Task<()>>,
    /// The Quick Actions of the row on screen, see `qactions`.
    pub(crate) actions: Option<QuickActions>,
}

impl Preview {
    pub fn new(shell: Shell, icons: Entity<IconStore>, cx: &mut Context<Self>) -> Self {
        Self {
            shell,
            icons,
            images: cx.new(PreviewImages::new),
            current: None,
            cache: HashMap::new(),
            wanted: None,
            body_scroll: ScrollHandle::new(),
            lines: ListState::new(0, ListAlignment::Top, px(TEXT_OVERDRAW)),
            lines_for: None,
            cpu: None,
            _load: None,
            _cpu: None,
            actions: None,
        }
    }

    /// The tips half of an empty clipboard history.
    pub fn show_help(&mut self, mode: Mode, cx: &mut Context<Self>) {
        self.actions = None;
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

    /// Memory and CPU of the shown process, for the compact row that stands
    /// in for the panel; CPU once Enter has asked for it.
    pub fn process_facts(&self, pid: u32) -> Option<String> {
        let (_, content) = self.current.as_ref()?;
        let Content::Process { meta, detail } = content.as_ref() else {
            return None;
        };
        if meta.pid != pid {
            return None;
        }
        let memory = detail
            .as_ref()
            .filter(|d| d.rss_kb > 0)
            .map(|d| format_size(d.rss_kb * 1024));
        let cpu = self
            .cpu
            .as_ref()
            .filter(|(p, _)| *p == pid)
            .map(|(_, text)| format!("CPU {text}"));
        let facts: Vec<String> = memory.into_iter().chain(cpu).collect();
        (!facts.is_empty()).then(|| facts.join(FACT_SEP))
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.actions = None;
        self.current = None;
        self.wanted = None;
        self._load = None;
        cx.notify();
    }

    /// Show `row`: at once from the cache, else after the dwell from the
    /// backend. The old preview stays up until the new one lands, so the
    /// column does not blink between rows.
    pub fn show(&mut self, row: &Row, ancestors: String, cx: &mut Context<Self>) {
        self.sync_actions(row, cx);
        // A block row is a different panel at a different depth.
        let key = if row.declared() {
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
            self.body_scroll.set_offset(Default::default());
            self.lines_for = None;
            cx.notify();
            return;
        }
        self.wanted = Some(key.clone());
        let dwell = reads_file(row);
        let row = row.clone();
        self._load = Some(cx.spawn(async move |this, cx| {
            if dwell {
                cx.background_executor().timer(DWELL).await;
            }
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
                this.body_scroll.set_offset(Default::default());
                this.lines_for = None;
                cx.notify();
            });
        }));
    }

    pub fn render_in(&mut self, th: &Theme, cx: &mut Context<Self>) -> Stateful<Div> {
        // The panel never scrolls: the body gives way to the rows under it,
        // as the webview's `.preview-slot` and the macOS ScrollView do, so
        // the header and the metadata stay wherever the column ends.
        let panel = div()
            .id("preview")
            .size_full()
            .p(px(theme::CONTENT_PADDING))
            .overflow_hidden()
            .flex()
            .flex_col();
        let Some((_, content)) = self.current.clone() else {
            return panel;
        };
        match content.as_ref() {
            Content::Clip { clip, text } => {
                let remove = Remove::Clip {
                    timestamp: clip.timestamp,
                    text: clip.text.clone(),
                };
                let look = TextLook {
                    size: th.font_size - 1.0,
                    line_height: None,
                    color: None,
                };
                let lines = matches!(text, TextBody::Lines(_));
                let scroll = self.text_scroll("clip-text", text, look, th);
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
                            .map(|card| {
                                if lines {
                                    card.h(px(CLIP_CARD_MAX_H))
                                } else {
                                    card.max_h(px(CLIP_CARD_MAX_H))
                                }
                            })
                            .min_h_0()
                            .flex()
                            .flex_col()
                            .rounded(px(th.control_radius()))
                            .bg(th.control_fill)
                            .child(scroll),
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
                        .min_h_0()
                        .p(px(CODE_PADDING))
                        .rounded(px(th.control_radius()))
                        .bg(th.control_fill)
                        .flex()
                        .justify_center()
                        .child(
                            img(image.clone())
                                .image_cache(&self.images)
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
                                size: HEADER_ICON,
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
                        false,
                        th,
                    ))
                    .child(clip_label("Command", th))
                    .child(
                        div()
                            .id("proc-cmd")
                            .max_h(px(CLIP_CARD_MAX_H))
                            .min_h_0()
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
                    .child(header(None, glyphs::ZAP, title, name, None, true, th))
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
                            .min_h_0()
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
                declared,
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
                            size: icon.size,
                        },
                        cx,
                    )
                });
                let shell = self.shell.clone();
                panel
                    .child(header(
                        picture,
                        glyph,
                        title,
                        badge,
                        aside.as_deref(),
                        *declared,
                        th,
                    ))
                    .children(self.actions_section(th, cx))
                    .child(self.body(body, th, shell))
                    // Under a body the rows sit at the foot, as macOS files do.
                    .child(info_rows(info, th).when(!matches!(body, Body::None), |r| r.mt_auto()))
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
            .on_click(cx.listener(move |_, _, _, cx| {
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

    /// A text body's scrolling part. Whole, one text sized to its content;
    /// by lines, a list filling the box it is given that lays out only the
    /// lines in view.
    fn text_scroll(
        &mut self,
        id: &'static str,
        text: &TextBody,
        look: TextLook,
        th: &Theme,
    ) -> AnyElement {
        match text {
            TextBody::Whole { text, highlights } => look
                .apply(div(), th)
                .id(id)
                .min_h_0()
                .overflow_y_scroll()
                .track_scroll(&self.body_scroll)
                .p(px(CODE_PADDING))
                .child(StyledText::new(text.clone()).with_highlights(syntax_runs(highlights, th)))
                .into_any_element(),
            TextBody::Lines(lines) => {
                if !self
                    .lines_for
                    .as_ref()
                    .is_some_and(|shown| Arc::ptr_eq(shown, lines))
                {
                    self.lines.reset(lines.len());
                    self.lines_for = Some(lines.clone());
                }
                let lines = lines.clone();
                let th = theme::get();
                list(self.lines.clone(), move |ix, _, _| {
                    let Some(line) = lines.get(ix) else {
                        return div().into_any_element();
                    };
                    look.apply(div(), &th)
                        .child(
                            StyledText::new(line.text.clone())
                                .with_highlights(syntax_runs(&line.highlights, &th)),
                        )
                        .into_any_element()
                })
                .flex_1()
                .min_h_0()
                .p(px(CODE_PADDING))
                .into_any_element()
            }
        }
    }

    fn body(&mut self, body: &Body, th: &Theme, shell: Shell) -> AnyElement {
        match body {
            Body::None => div().into_any_element(),
            Body::Image(path) => div()
                .mt(px(BODY_MARGIN))
                .min_h_0()
                .flex()
                .justify_center()
                .child(
                    img(path.clone())
                        .image_cache(&self.images)
                        .max_w_full()
                        .max_h(px(image_max_h()))
                        .object_fit(ObjectFit::Contain)
                        .rounded(px(th.chip_radius()))
                        .id("preview-image"),
                )
                .into_any_element(),
            Body::Code { text, truncated } => {
                let size = th.font_size - 2.0;
                let look = TextLook {
                    size,
                    line_height: Some(size * CODE_LINE_HEIGHT),
                    color: Some(theme::hsla_of(th.text_secondary)),
                };
                // A long file fills the column; a short one keeps to its
                // lines, the rows under it at the foot.
                let lines = matches!(text, TextBody::Lines(_));
                let scroll = self.text_scroll("code", text, look, th);
                div()
                    .mt(px(BODY_MARGIN))
                    .min_h_0()
                    .when(lines, |el| el.flex_1())
                    .rounded(px(th.control_radius()))
                    .bg(th.control_fill)
                    .flex()
                    .flex_col()
                    .child(scroll)
                    .when(*truncated, |el| {
                        el.child(truncated_note(TRUNCATED_NOTE, th))
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
                    .min_h_0()
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
                list.when(*truncated, |el| {
                    el.child(truncated_note(
                        format!("First {} entries", files::LIST_FOLDER_CAP),
                        th,
                    ))
                })
                .into_any_element()
            }
        }
    }
}

impl TextLook {
    fn apply(self, el: Div, th: &Theme) -> Div {
        el.font_family(th.mono_family.clone())
            .text_size(px(self.size))
            .when_some(self.line_height, |el, h| el.line_height(px(h)))
            .when_some(self.color, |el, c| el.text_color(c))
    }
}

impl TextBody {
    /// Built off the UI thread, with the load.
    fn new(text: String, highlights: Vec<(Range<usize>, TokenType)>) -> Self {
        if text.len() <= LONG_TEXT_BYTES && text.lines().count() <= LONG_TEXT_LINES {
            return TextBody::Whole {
                text: text.into(),
                highlights,
            };
        }
        TextBody::Lines(text_lines(&text, &highlights).into())
    }
}

/// `text` as list items: each line, a long one cut at `LINE_PIECE_BYTES`,
/// with the highlights that fall in it. `highlights` run in order without
/// overlapping, as the tokenizer gives them.
fn text_lines(text: &str, highlights: &[(Range<usize>, TokenType)]) -> Vec<TextLine> {
    let mut items = Vec::new();
    let mut next = 0;
    let mut start = 0;
    for line in text.strip_suffix('\n').unwrap_or(text).split('\n') {
        let end = start + line.len();
        let mut piece = start;
        loop {
            let piece_end = if end - piece > LINE_PIECE_BYTES {
                text.floor_char_boundary(piece + LINE_PIECE_BYTES)
            } else {
                end
            };
            while highlights.get(next).is_some_and(|(r, _)| r.end <= piece) {
                next += 1;
            }
            let slice = text[piece..piece_end].trim_end_matches('\r');
            let spans = highlights[next..]
                .iter()
                .take_while(|(r, _)| r.start < piece_end)
                .map(|(r, token)| {
                    let clip = |at: usize| at.clamp(piece, piece + slice.len()) - piece;
                    (clip(r.start)..clip(r.end), *token)
                })
                .filter(|(r, _)| !r.is_empty())
                .collect();
            items.push(TextLine {
                // A blank line still takes a line's height.
                text: if slice.is_empty() {
                    " ".into()
                } else {
                    slice.to_string().into()
                },
                highlights: spans,
            });
            piece = piece_end;
            if piece >= end {
                break;
            }
        }
        start = end + 1;
    }
    items
}

fn syntax_runs(
    highlights: &[(Range<usize>, TokenType)],
    th: &Theme,
) -> Vec<(Range<usize>, HighlightStyle)> {
    highlights
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
        .collect()
}

/// The row's whole description, read on the background executor.
/// Only a body that reads bytes off disk waits out the dwell: a file's text
/// or picture, a copied image. Apps, folders, clips and cards describe
/// themselves from the row and show at once.
fn reads_file(row: &Row) -> bool {
    match &row.open {
        Open::Path { .. } => row.kind == "file",
        Open::ClipImage(_) => true,
        _ => false,
    }
}

fn load(row: &Row, ancestors: &str) -> Content {
    if row.declared() && row.path.is_empty() {
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
        Open::Clip(clip) => Content::Clip {
            clip: clip.clone(),
            text: TextBody::new(clip.text.clone(), Vec::new()),
        },
        Open::ClipImage(meta) => Content::ClipImage {
            meta: meta.clone(),
            title: row.title.clone(),
            image: clipimage::existing_path(&meta.hash),
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
/// The clipboard histories' "How to use": the preview column's in split,
/// under the empty state in compact.
pub fn help(mode: Mode, th: &Theme) -> Div {
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
        size: HEADER_ICON,
    };
    let badge = badge_label(&row.kind).to_string();
    let declared = row.declared();
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
                text: TextBody::new(
                    runs.text,
                    runs.spans
                        .into_iter()
                        .map(|s| (s.start..s.end, s.token))
                        .collect(),
                ),
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
        declared,
        title: row.title.clone(),
        badge,
        aside,
        info,
        body,
    }
}

/// `declared` marks a row from the user's own block: the tile wears the
/// accent fill and ring the list gives the same rows.
fn header(
    picture: Option<Arc<gpui::RenderImage>>,
    glyph: &'static str,
    title: &str,
    badge: &str,
    aside: Option<&str>,
    declared: bool,
    th: &Theme,
) -> Div {
    let glyph_colour = if declared {
        th.accent
    } else {
        th.text_secondary
    };
    let icon = div()
        .size(px(HEADER_ICON))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(HEADER_ICON_RADIUS))
        .bg(th.control_fill)
        .when(declared, |tile| th.declared(tile))
        .child(match picture {
            Some(image) => img(image)
                .size(px(HEADER_ICON))
                .object_fit(ObjectFit::Contain)
                .into_any_element(),
            None => svg()
                .path(glyph)
                .size(px(HEADER_ICON / 2.0))
                .text_color(glyph_colour)
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
    info_list(th).children(
        info.iter()
            .map(|(label, value)| info_row(*label, value.clone(), th)),
    )
}

/// The column the label/value rows sit in, at their size and leading.
pub(crate) fn info_list(th: &Theme) -> Div {
    div()
        .pt(px(HEADER_MARGIN))
        .flex_shrink_0()
        .flex()
        .flex_col()
        .gap(px(INFO_ROW_GAP))
        .text_size(px(th.font_size - 2.0))
        .line_height(relative(theme::LABEL_LINE_HEIGHT))
}

/// A muted label left, its value right.
pub(crate) fn info_row(label: impl Into<SharedString>, value: String, th: &Theme) -> Div {
    div()
        .flex()
        .items_baseline()
        .gap(px(INFO_GAP))
        .child(
            div()
                .flex_shrink_0()
                .text_color(th.text_muted)
                .child(label.into()),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_right()
                .text_color(th.text_secondary)
                .child(middle_elided(value)),
        )
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
/// The footer line under a body that stops short of the whole file.
fn truncated_note(text: impl Into<SharedString>, th: &Theme) -> Div {
    div()
        .py(px(INFO_PADDING_Y))
        .border_t(px(1.0))
        .border_color(th.border)
        .text_size(px(th.font_size - 2.0))
        .text_color(th.text_muted)
        .text_center()
        .child(text.into())
}

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

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(lines: &[TextLine]) -> Vec<&str> {
        lines.iter().map(|l| l.text.as_ref()).collect()
    }

    /// A block comment over two lines lands on both, each part relative to
    /// its own line; a blank line keeps its row; CRLF loses the CR.
    #[test]
    fn highlights_follow_their_lines() {
        let text = "a /* b\r\nc */ d\n\nend";
        let comment = 2..12;
        let lines = text_lines(text, &[(comment, TokenType::Comment)]);
        assert_eq!(texts(&lines), ["a /* b", "c */ d", " ", "end"]);
        assert_eq!(lines[0].highlights, [(2..6, TokenType::Comment)]);
        assert_eq!(lines[1].highlights, [(0..4, TokenType::Comment)]);
        assert!(lines[2].highlights.is_empty() && lines[3].highlights.is_empty());
    }

    /// A long line is cut into pieces on character boundaries, and a span
    /// across a cut is split with it.
    #[test]
    fn a_long_line_is_cut_into_pieces() {
        let text = "é".repeat(LINE_PIECE_BYTES);
        let lines = text_lines(&text, &[(0..text.len(), TokenType::String)]);
        assert_eq!(lines.len(), 2);
        assert_eq!(
            lines.iter().map(|l| l.text.len()).sum::<usize>(),
            text.len()
        );
        for line in &lines {
            assert_eq!(line.highlights, [(0..line.text.len(), TokenType::String)]);
        }
    }

    #[test]
    fn only_a_long_text_goes_by_lines() {
        let short = "line\n".repeat(LONG_TEXT_LINES);
        assert!(matches!(
            TextBody::new(short, Vec::new()),
            TextBody::Whole { .. }
        ));
        let long = "line\n".repeat(LONG_TEXT_LINES + 1);
        assert!(matches!(
            TextBody::new(long, Vec::new()),
            TextBody::Lines(_)
        ));
    }
}
