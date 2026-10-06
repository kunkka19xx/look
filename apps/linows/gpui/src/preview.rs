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
    AnyElement, Context, Div, Entity, FontWeight, HighlightStyle, ObjectFit, ScrollHandle,
    SharedString, Stateful, StyledText, Task, div, img, prelude::*, px, svg,
};
use linows_backend::highlight::{self, TokenType};
use linows_backend::{files, launch};

use crate::Shell;
use crate::icons::{IconRequest, IconStore};
use crate::rows::{self, Icon, Open, Row};
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

/// What the panel knows about one row once the backend has answered.
pub enum Content {
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
    _load: Option<Task<()>>,
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
            _load: None,
        }
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
    pub fn show(&mut self, row: &Row, cx: &mut Context<Self>) {
        let key = row.id.clone();
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
                .spawn(async move { Arc::new(load(&row)) })
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
fn load(row: &Row) -> Content {
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
    }
}

fn standard(row: &Row) -> Content {
    let glyph = match &row.icon {
        Icon::Resolve { glyph } | Icon::Glyph(glyph) => *glyph,
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
