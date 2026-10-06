//! The launcher shell: search bar, launchpad bento while the query is empty,
//! one results card once it is not. Floating layout: no window box, the bar
//! and the cards float on the desktop with the inner gap as every seam.
//! Outcomes and warnings surface as a short notice chip over the bottom edge.

use std::cell::RefCell;
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    Animation, AnimationExt, Bounds, ClipboardItem, Context, Div, Entity, FontWeight, KeyDownEvent,
    Pixels, Render, ScrollStrategy, SharedString, Task, UniformListScrollHandle, Window, div, img,
    prelude::*, px, svg, uniform_list,
};
use linows_backend::host::LauncherWindow;
use linows_backend::search as engine;
use linows_backend::{clipboard, launch, weburl};

use crate::blur::{self, BlurRect};
use crate::glyphs;
use crate::icons::{IconRequest, IconStore};
use crate::launchpad::{Launchpad, Notice, Tone};
use crate::motion;
use crate::preview::Preview;
use crate::query;
use crate::rows::{Icon, Open, Row};
use crate::search::{Changed, SearchInput, search_field};
use crate::theme::{self, Theme};
use crate::{Shell, state as app_state};

const PLACEHOLDER: &str = "Search apps, files, actions";
const HINT_MAIN: &str = "Enter: Open \u{2022} Ctrl+F: Reveal \u{2022} Ctrl+C: Copy path";
const HINT_EMPTY: &str = "No match \u{2022} Ctrl+Enter: Search the web";
const WEB_SEARCH_URL: &str = "https://www.google.com/search?q=";
/// How long a keystroke waits for the next before the query runs; the
/// webview uses the same.
const DEBOUNCE: Duration = Duration::from_millis(70);
/// A row's picture, or the glyph that stands in for it.
const ROW_ICON: f32 = 22.0;
/// The notice chip's fade, the banner's.
const NOTICE_FADE_MS: u64 = 180;
const NOTICE_PADDING_X: f32 = 12.0;
const NOTICE_PADDING_Y: f32 = 6.0;

pub struct Launcher {
    shell: Shell,
    input: Entity<SearchInput>,
    icons: Entity<IconStore>,
    preview: Entity<Preview>,
    launchpad: Entity<Launchpad>,
    notice: Option<Notice>,
    /// Identifies the notice on screen, so an older timer cannot clear a newer one.
    notice_seq: u64,
    _notice_timer: Option<Task<()>>,
    rows: Arc<Vec<Row>>,
    selected: usize,
    scroll: UniformListScrollHandle,
    /// Bumped per keystroke; a search that comes back for an older one is dropped.
    version: u64,
    _search: Option<Task<()>>,
}

impl Launcher {
    pub fn new(shell: Shell, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(SearchInput::new);
        cx.subscribe(&input, |this, _, _: &Changed, cx| this.search(cx))
            .detach();
        let icons = cx.new(|_| IconStore::new());
        cx.observe(&icons, |_, _, cx| cx.notify()).detach();
        let preview = cx.new(|_| Preview::new(shell.clone(), icons.clone()));
        cx.observe(&preview, |_, _, cx| cx.notify()).detach();
        let launchpad = cx.new(Launchpad::new);
        cx.observe(&launchpad, |_, _, cx| cx.notify()).detach();
        cx.subscribe(&launchpad, |this, _, notice: &Notice, cx| {
            this.show_notice(notice.clone(), cx)
        })
        .detach();
        let focus_handle = input.read(cx).focus_handle.clone();
        window.focus(&focus_handle, cx);
        blur::attach_window(window);

        Self {
            shell,
            input,
            icons,
            preview,
            launchpad,
            notice: None,
            notice_seq: 0,
            _notice_timer: None,
            rows: Arc::new(Vec::new()),
            selected: 0,
            scroll: UniformListScrollHandle::new(),
            version: 0,
            _search: None,
        }
    }

    pub fn query(&self, cx: &gpui::App) -> String {
        self.input.read(cx).text().to_string()
    }

    pub fn set_query(&mut self, text: &str, cx: &mut Context<Self>) {
        self.input.update(cx, |input, cx| input.set_text(text, cx));
    }

    /// Run the query again: a keystroke, or the index finishing a refresh.
    /// The rows come back off the UI thread; a stale answer is dropped.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.search(cx);
    }

    fn search(&mut self, cx: &mut Context<Self>) {
        self.version += 1;
        let version = self.version;
        let query = self.input.read(cx).committed();
        let home = query.trim().is_empty();
        self.launchpad
            .update(cx, |launchpad, cx| launchpad.set_shown(home, cx));
        if home {
            self.rows = Arc::new(Vec::new());
            self.selected = 0;
            self.sync_preview(cx);
            cx.notify();
            return;
        }
        self._search = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(DEBOUNCE).await;
            let rows = cx
                .background_executor()
                .spawn(async move {
                    let started = std::time::Instant::now();
                    let rows = query::run(&query);
                    if crate::probe_wanted() {
                        eprintln!(
                            "search {:.1} ms rows={} query={query:?}",
                            started.elapsed().as_secs_f64() * 1000.0,
                            rows.len()
                        );
                    }
                    rows
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.version != version {
                    return;
                }
                this.rows = Arc::new(rows);
                this.selected = 0;
                this.scroll.scroll_to_item(0, ScrollStrategy::Top);
                this.sync_preview(cx);
                cx.notify();
            });
        }));
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let ks = &event.keystroke;
        let ctrl = ks.modifiers.control;
        let shift = ks.modifiers.shift;
        // Alt+<letter> on the home screen is a launchpad mnemonic.
        if ks.modifiers.alt && !ctrl && self.on_home(cx) {
            let mut chars = ks.key.chars();
            if let (Some(ch), None) = (chars.next(), chars.next())
                && self
                    .launchpad
                    .update(cx, |launchpad, cx| launchpad.mnemonic(ch, cx))
            {
                cx.stop_propagation();
                return;
            }
        }
        let handled = match ks.key.as_str() {
            "escape" => {
                self.shell.hide();
                true
            }
            "enter" if ctrl => self.search_web(cx),
            "enter" => self.open_selected(cx),
            "down" => self.move_selection(1, cx),
            "up" => self.move_selection(-1, cx),
            "tab" => self.move_selection(if shift { -1 } else { 1 }, cx),
            "n" if ctrl => self.move_selection(1, cx),
            "p" if ctrl => self.move_selection(-1, cx),
            "backspace" if ctrl => self.edit(cx, SearchInput::delete_word_back),
            "w" if ctrl => self.edit(cx, SearchInput::delete_word_back),
            "backspace" => self.edit(cx, SearchInput::backspace),
            "delete" => self.edit(cx, SearchInput::delete),
            "left" if ctrl => self.edit(cx, move |i, cx| i.word_left(shift, cx)),
            "right" if ctrl => self.edit(cx, move |i, cx| i.word_right(shift, cx)),
            "left" => self.edit(cx, move |i, cx| i.left(shift, cx)),
            "right" => self.edit(cx, move |i, cx| i.right(shift, cx)),
            "home" => self.edit(cx, move |i, cx| i.home(shift, cx)),
            "end" => self.edit(cx, move |i, cx| i.end(shift, cx)),
            "a" if ctrl => self.edit(cx, SearchInput::select_all),
            "u" if ctrl => self.edit(cx, SearchInput::clear),
            "v" if ctrl => self.paste(cx),
            "c" if ctrl => self.copy(cx),
            "f" if ctrl => self.reveal_selected(),
            _ => false,
        };
        if handled {
            cx.stop_propagation();
        }
    }

    fn edit(
        &mut self,
        cx: &mut Context<Self>,
        op: impl FnOnce(&mut SearchInput, &mut Context<SearchInput>),
    ) -> bool {
        self.input.update(cx, op);
        true
    }

    fn move_selection(&mut self, delta: isize, cx: &mut Context<Self>) -> bool {
        if !self.rows.is_empty() {
            let len = self.rows.len() as isize;
            self.selected = (self.selected as isize + delta).rem_euclid(len) as usize;
            self.scroll
                .scroll_to_item(self.selected, ScrollStrategy::Nearest);
            self.sync_preview(cx);
            cx.notify();
        }
        true
    }

    /// The preview follows the selection; the compact layout has none.
    fn sync_preview(&mut self, cx: &mut Context<Self>) {
        let split = theme::get().split();
        let row = self.rows.get(self.selected).cloned();
        self.preview.update(cx, |preview, cx| match row {
            Some(row) if split => preview.show(&row, cx),
            _ => preview.clear(cx),
        });
    }

    fn selected_row(&self) -> Option<&Row> {
        self.rows.get(self.selected)
    }

    /// Enter: what the row says. The backend opens paths and hides the
    /// launcher through the shell; a URL goes to the browser and the history;
    /// an answer goes to the clipboard.
    fn open_selected(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(row) = self.selected_row().cloned() else {
            return true;
        };
        match row.open {
            Open::Path { usage } => {
                match launch::open_path(&self.shell, row.path, Some(&row.kind), Some(&row.id)) {
                    Ok(()) => {
                        engine::record_usage(app_state(), &row.id, usage);
                    }
                    Err(err) => eprintln!("open {}: {err}", row.title),
                }
            }
            Open::Url(url) => {
                self.open_url(&url);
                cx.background_executor()
                    .spawn(async move {
                        weburl::record_url_hit(&url);
                    })
                    .detach();
            }
            Open::Calc { raw, expr } => {
                // History keeps the working; the paste is the number.
                if let Err(err) =
                    clipboard::copy_to_clipboard_labeled(raw, format!("{expr} = {}", row.title))
                {
                    eprintln!("copy: {err}");
                }
                self.shell.hide();
            }
        }
        true
    }

    fn open_url(&self, url: &str) {
        if let Err(err) = launch::open_path(&self.shell, url.to_string(), Some("browser"), None) {
            eprintln!("open {url}: {err}");
        }
    }

    /// Ctrl+Enter: the query as a web search.
    fn search_web(&mut self, cx: &mut Context<Self>) -> bool {
        let query = self.input.read(cx).committed();
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return true;
        }
        self.open_url(&format!("{WEB_SEARCH_URL}{}", url_encode(trimmed)));
        true
    }

    fn reveal_selected(&self) -> bool {
        if let Some(row) = self.selected_row()
            && matches!(row.open, Open::Path { .. })
        {
            match launch::reveal_path(&row.path) {
                Ok(()) => self.shell.hide(),
                Err(err) => eprintln!("reveal: {err}"),
            }
        }
        true
    }

    /// Ctrl+C: the field's selection when there is one, else the row's path.
    fn copy(&mut self, cx: &mut Context<Self>) -> bool {
        if let Some(text) = self.input.read(cx).selected_text() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            return true;
        }
        if let Some(row) = self.selected_row()
            && !row.path.is_empty()
            && let Err(err) = clipboard::copy_to_clipboard(&row.path)
        {
            eprintln!("copy: {err}");
        }
        true
    }

    fn paste(&mut self, cx: &mut Context<Self>) -> bool {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            // One line: the field is single-line, and a pasted path or query
            // never wants its newlines.
            let text = text.replace(['\r', '\n'], " ");
            self.input.update(cx, |input, cx| input.insert(&text, cx));
        }
        true
    }

    fn top_bar(&self, th: &Theme) -> impl IntoElement {
        let bar_h = theme::TOP_ROW_HEIGHT + 2.0 * theme::INPUT_PADDING_Y;
        let bar = card(div(), th)
            .absolute()
            .left_0()
            .right_0()
            .h(px(bar_h))
            .px(px(theme::INPUT_PADDING_X))
            .flex()
            .items_center()
            .gap(px(theme::SEARCH_GAP))
            .rounded(px(th.bar_radius()))
            .text_size(px(th.font_size + 1.0))
            .child(
                svg()
                    .path(glyphs::SEARCH)
                    .size(px(theme::SEARCH_ICON))
                    .text_color(th.accent),
            )
            .child(search_field(self.input.clone(), PLACEHOLDER))
            .child(hint_chip("Esc", th));

        // The bar rises in place, so its slot is fixed and the body below
        // does not move with it.
        let spawn = Duration::from_millis(motion::SPAWN_MS);
        let radius = th.bar_radius();
        div()
            .relative()
            .h(px(bar_h))
            .mx(px(theme::CONTENT_PADDING))
            .mt(px(theme::CONTENT_PADDING))
            .on_children_prepainted(move |cards, _, _| mark_cards(&cards, radius))
            .child(
                bar.with_animation("top-bar", Animation::new(spawn), |bar, t| {
                    let t = motion::spring(t);
                    bar.opacity(t.min(1.0))
                        .top(px(motion::SPAWN_RISE * (1.0 - t)))
                }),
            )
    }

    /// The empty query shows the launchpad: the tiles the entity draws, in a
    /// grid whose seams are the inner gap, each marked for the blur region.
    fn bento(&mut self, th: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let (grid_h, tiles) = self
            .launchpad
            .update(cx, |launchpad, cx| launchpad.tiles_in(th, cx));
        let radius = th.tile_radius();
        div()
            .relative()
            .mx(px(theme::CONTENT_PADDING))
            .mt(px(th.inner_gap))
            .h(px(grid_h))
            .on_children_prepainted(move |cards, _, _| mark_cards(&cards, radius))
            .children(tiles)
    }

    /// Whether the empty query is showing, launchpad or not.
    fn on_home(&self, cx: &gpui::App) -> bool {
        self.input.read(cx).text().is_empty()
    }

    fn show_notice(&mut self, notice: Notice, cx: &mut Context<Self>) {
        self.notice_seq += 1;
        let seq = self.notice_seq;
        let shown_for = Duration::from_secs_f32(notice.seconds);
        self.notice = Some(notice);
        self._notice_timer = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(shown_for).await;
            let _ = this.update(cx, |this, cx| {
                if this.notice_seq == seq {
                    this.notice = None;
                    cx.notify();
                }
            });
        }));
        cx.notify();
    }

    /// The notice chip, centred over the bottom padding.
    fn notice_chip(&self, th: &Theme) -> Option<impl IntoElement> {
        let notice = self.notice.as_ref()?;
        let colour = match notice.tone {
            Tone::Success => th.success,
            Tone::Info => th.accent,
            Tone::Warning => th.warning,
            Tone::Error => th.danger,
        };
        let chip = card(div(), th)
            .px(px(NOTICE_PADDING_X))
            .py(px(NOTICE_PADDING_Y))
            .rounded(px(th.bar_radius()))
            .text_size(px(th.font_size - 1.0))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(colour)
            .child(notice.text.clone());
        Some(
            div()
                .absolute()
                .bottom(px(theme::CONTENT_PADDING))
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .child(chip)
                .with_animation(
                    ("notice", self.notice_seq as usize),
                    Animation::new(Duration::from_millis(NOTICE_FADE_MS)),
                    |chip, t| chip.opacity(motion::curve(t)),
                ),
        )
    }

    fn results(&self, composing: bool, th: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let rows = self.rows.clone();
        let selected = self.selected;
        let icons = self.icons.clone();
        let row_theme = th.clone();
        let launcher = cx.entity();
        let list = uniform_list("results", rows.len(), move |range, _window, cx| {
            range
                .map(|i| {
                    let row = &rows[i];
                    let th = &row_theme;
                    let picture = match &row.icon {
                        Icon::Resolve { glyph } => {
                            let image = icons.update(cx, |store, cx| {
                                store.get(
                                    IconRequest {
                                        kind: row.kind.clone(),
                                        path: row.path.clone(),
                                        id: Some(row.id.clone()),
                                    },
                                    cx,
                                )
                            });
                            match image {
                                Some(image) => img(image).size(px(ROW_ICON)).into_any_element(),
                                None => glyph_icon(glyph, th).into_any_element(),
                            }
                        }
                        Icon::Glyph(glyph) => glyph_icon(glyph, th).into_any_element(),
                    };
                    let launcher = launcher.clone();
                    div()
                        .id(("row", i))
                        .w_full()
                        .h(px(theme::ROW_HEIGHT))
                        .px(px(theme::ROW_PADDING_X))
                        .flex()
                        .items_center()
                        .gap(px(theme::SEARCH_GAP))
                        .rounded(px(th.control_radius()))
                        .when(i == selected, |el| el.bg(th.selection_fill))
                        .on_click(move |_, _, cx| {
                            launcher.update(cx, |this, cx| {
                                this.selected = i;
                                this.open_selected(cx);
                            });
                        })
                        .child(
                            div()
                                .size(px(theme::ICON_CHIP))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(picture),
                        )
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .min_w_0()
                                .child(
                                    div()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .truncate()
                                        .child(row.title.clone()),
                                )
                                .when(!row.context.is_empty(), |col| {
                                    col.child(muted_text(row.context.clone(), th).truncate())
                                }),
                        )
                        .when(!row.kind_label.is_empty(), |el| {
                            el.child(muted_text(row.kind_label.clone(), th))
                        })
                        .when(i == selected, |el| el.child(hint_chip("Enter", th)))
                })
                .collect()
        })
        .track_scroll(&self.scroll)
        .flex_1()
        .min_h_0();

        let hint = if composing {
            "Composing with fcitx5"
        } else if self.rows.is_empty() {
            HINT_EMPTY
        } else {
            HINT_MAIN
        };

        // One floating card: rows, then the hint as the card's own footer.
        let radius = th.tile_radius();
        let results_card = card(div(), th)
            .px(px(theme::ROW_INSET))
            .pt(px(theme::ROW_INSET))
            .pb(px(theme::HINT_INSET_BOTTOM))
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .gap(px(theme::ROW_SPACING))
            .rounded(px(radius))
            .child(list)
            .child(
                muted_text(hint, th)
                    .px(px(theme::ROW_PADDING_X))
                    .pt(px(theme::HINT_INSET))
                    .pb(px(theme::HINT_INSET_BOTTOM)),
            );
        // Split: the preview floats beside the list as its own card.
        let preview_card = th.split().then(|| {
            let panel_theme = th.clone();
            let body = self
                .preview
                .update(cx, |preview, cx| preview.render_in(&panel_theme, cx));
            card(div(), th)
                .flex_1()
                .min_w_0()
                .min_h_0()
                .overflow_hidden()
                .rounded(px(radius))
                .child(body)
        });
        div()
            .flex_1()
            .min_h_0()
            .mx(px(theme::CONTENT_PADDING))
            .mt(px(th.inner_gap))
            .mb(px(theme::CONTENT_PADDING))
            .flex()
            .flex_row()
            .gap(px(th.inner_gap))
            .on_children_prepainted(move |cards, _, _| mark_cards(&cards, radius))
            .child(results_card)
            .children(preview_card)
    }
}

impl Drop for Launcher {
    fn drop(&mut self) {
        blur::detach();
    }
}

impl Render for Launcher {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let th = theme::get();
        let input = self.input.read(cx);
        let home = input.text().is_empty();
        let composing = input.is_composing();
        let focus_handle = input.focus_handle.clone();

        // The launchpad is a setting; off, the empty query is the bar alone.
        let below = match (home, th.launchpad) {
            (true, true) => self.bento(&th, cx).into_any_element(),
            (true, false) => div().into_any_element(),
            (false, _) => self.results(composing, &th, cx).into_any_element(),
        };
        let body = div()
            .size_full()
            .flex()
            .flex_col()
            .child(self.top_bar(&th))
            .child(below);

        // gpui 0.2.2 has no element scale, so the arrive scale is an inset.
        let arrive = Duration::from_millis(motion::ARRIVE_MS);
        let inset_x = theme::WINDOW_W * (1.0 - motion::ARRIVE_SCALE) / 2.0;
        let inset_y = theme::WINDOW_H * (1.0 - motion::ARRIVE_SCALE) / 2.0;

        div()
            .when(crate::probe_wanted(), |root| root.child(pace_probe()))
            .size_full()
            .font_family(th.font_family.clone())
            .text_size(px(th.font_size))
            .text_color(th.text)
            .track_focus(&focus_handle)
            .on_key_down(cx.listener(Self::on_key_down))
            .on_children_prepainted(|_, _, _| commit_blur_region())
            .child(div().size_full().child(body).with_animation(
                "arrive",
                Animation::new(arrive),
                move |root, t| {
                    let t = motion::curve(t);
                    root.opacity(t)
                        .px(px(inset_x * (1.0 - t)))
                        .py(px(inset_y * (1.0 - t)))
                },
            ))
            .children(self.notice_chip(&th))
    }
}

thread_local! {
    /// Card bounds gathered during one prepaint pass; the root hands them to
    /// the compositor once every card is placed.
    static BLUR_FRAME: RefCell<Vec<BlurRect>> = const { RefCell::new(Vec::new()) };
}

fn mark_cards(cards: &[Bounds<Pixels>], radius: f32) {
    BLUR_FRAME.with_borrow_mut(|frame| {
        for b in cards {
            frame.extend(BlurRect::rounded(
                f32::from(b.origin.x).round() as i32,
                f32::from(b.origin.y).round() as i32,
                f32::from(b.size.width).round() as u32,
                f32::from(b.size.height).round() as u32,
                radius,
            ));
        }
    });
}

fn commit_blur_region() {
    blur::set_region(BLUR_FRAME.take());
}

/// The frosted card face shared by the bar, the tiles and the results card.
fn card(el: Div, th: &Theme) -> Div {
    el.bg(th.card_face())
        .border(px(th.border_thickness))
        .border_color(th.border)
        .shadow(th.card_shadow())
}

fn muted_text(text: impl Into<SharedString>, th: &Theme) -> Div {
    div()
        .text_size(px(th.font_size - 1.0))
        .text_color(th.text_muted)
        .child(text.into())
}

fn hint_chip(label: &'static str, th: &Theme) -> Div {
    div()
        .px(px(theme::CHIP_PADDING_X))
        .py(px(theme::CHIP_PADDING_Y))
        .rounded(px(th.chip_radius()))
        .bg(th.panel_fill)
        .text_size(px(theme::CAPTION_SIZE))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(th.text_secondary)
        .child(label)
}

/// The glyph a row wears until its picture lands, in the accent like the
/// webview's kind glyphs.
fn glyph_icon(path: &'static str, th: &Theme) -> impl IntoElement {
    svg()
        .path(path)
        .size(px(theme::SEARCH_ICON + 2.0))
        .text_color(th.accent)
}

/// Percent-encoding for a query in a URL: letters, digits and the unreserved
/// marks pass, everything else is bytes.
fn url_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// A repeating animation on an invisible element, one timestamp per frame,
/// so tools/pace.sh can read steady-state pacing without the entrance in it.
fn pace_probe() -> impl IntoElement {
    div().absolute().size(px(1.0)).with_animation(
        "pace-probe",
        Animation::new(Duration::from_secs(1)).repeat(),
        |probe, t| {
            let micros = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_micros())
                .unwrap_or(0);
            eprintln!("probe {micros} t={t:.3}");
            probe.opacity(t)
        },
    )
}
