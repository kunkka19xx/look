//! The launcher shell: search bar, launchpad bento while the query is empty,
//! one results card once it is not. Floating layout: no window box, the bar
//! and the cards float on the desktop with the inner gap as every seam.

use std::time::Duration;

use std::cell::RefCell;

use gpui::{
    Animation, AnimationExt, Bounds, Context, Div, Entity, FontWeight, KeyDownEvent, Pixels,
    Render, SharedString, Task, Window, div, prelude::*, px, svg,
};

use linows_backend::host::LauncherWindow;
use linows_backend::launch;
use linows_backend::search::{self as engine, SearchResult};

use crate::blur::{self, BlurRect};
use crate::motion;
use crate::search::{Changed, SearchInput, search_field};
use crate::theme::{self, Theme};
use crate::{Shell, state as app_state};

const RESULT_LIMIT: usize = 8;
const GRID_COLS: usize = 6;
const GRID_ROWS: usize = 3;
const GRID_ROW_H: f32 = 74.0;
const PLACEHOLDER: &str = "Search apps, files, actions";

/// A launchpad cell: caption, placeholder value, column, row, spans.
/// The clock's value is computed; every other value is a stand-in.
struct Tile(&'static str, &'static str, usize, usize, usize, usize);

const CLOCK: &str = "Clock";
const TILES: &[Tile] = &[
    Tile(CLOCK, "", 0, 0, 2, 2),
    Tile("Weather", "24°  Hà Nội", 2, 0, 2, 1),
    Tile("Battery", "87%", 4, 0, 1, 1),
    Tile("Bluetooth", "On", 5, 0, 1, 1),
    Tile("Todo", "3 open", 2, 1, 2, 1),
    Tile("Theme", "Mocha", 4, 1, 1, 1),
    Tile("Wi-Fi", "Up", 5, 1, 1, 1),
    Tile("Lunar", "14/8 Bính Ngọ", 0, 2, 2, 1),
    Tile("Music", "Paused", 2, 2, 2, 1),
    Tile("Network", "1.2 MB/s", 4, 2, 1, 1),
    Tile("Power", "Sleep", 5, 2, 1, 1),
];

pub struct Launcher {
    shell: Shell,
    input: Entity<SearchInput>,
    results: Vec<SearchResult>,
    selected: usize,
    _clock: Task<()>,
}

impl Launcher {
    pub fn new(shell: Shell, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(SearchInput::new);
        cx.subscribe(&input, |this, _, _: &Changed, cx| {
            this.selected = 0;
            this.refresh(cx);
        })
        .detach();
        let focus_handle = input.read(cx).focus_handle.clone();
        window.focus(&focus_handle, cx);
        blur::attach_window(window);

        let clock = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        });

        Self {
            shell,
            input,
            results: Vec::new(),
            selected: 0,
            _clock: clock,
        }
    }

    pub fn set_query(&mut self, text: &str, cx: &mut Context<Self>) {
        self.input.update(cx, |input, cx| input.set_text(text, cx));
    }

    /// Run the query again: a keystroke, or the index finishing a refresh.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        let query = self.input.read(cx).committed();
        self.results = if query.trim().is_empty() {
            Vec::new()
        } else {
            engine::search(app_state(), &query, RESULT_LIMIT as u32).results
        };
        self.selected = self.selected.min(self.results.len().saturating_sub(1));
        cx.notify();
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let ks = &event.keystroke;
        let handled = match ks.key.as_str() {
            "escape" => {
                self.shell.hide();
                true
            }
            "enter" => {
                self.open_selected();
                true
            }
            "down" => self.move_selection(1, cx),
            "up" => self.move_selection(-1, cx),
            "n" if ks.modifiers.control => self.move_selection(1, cx),
            "p" if ks.modifiers.control => self.move_selection(-1, cx),
            "backspace" => self.edit(cx, SearchInput::backspace),
            "delete" => self.edit(cx, SearchInput::delete),
            "left" => self.edit(cx, SearchInput::left),
            "right" => self.edit(cx, SearchInput::right),
            "home" => self.edit(cx, SearchInput::home),
            "end" => self.edit(cx, SearchInput::end),
            "u" if ks.modifiers.control => self.edit(cx, SearchInput::clear),
            _ => false,
        };
        if handled {
            cx.stop_propagation();
        }
    }

    fn edit(
        &mut self,
        cx: &mut Context<Self>,
        op: fn(&mut SearchInput, &mut Context<SearchInput>),
    ) -> bool {
        self.input.update(cx, op);
        true
    }

    fn move_selection(&mut self, delta: isize, cx: &mut Context<Self>) -> bool {
        if !self.results.is_empty() {
            let len = self.results.len() as isize;
            self.selected = (self.selected as isize + delta).rem_euclid(len) as usize;
            cx.notify();
        }
        true
    }

    /// Enter: the backend opens the row and hides the launcher through the
    /// shell; the usage event follows the same spelling the webview uses.
    fn open_selected(&mut self) {
        let Some(row) = self.results.get(self.selected) else {
            return;
        };
        let action = match row.kind.as_str() {
            "app" => "open_app",
            "folder" => "open_folder",
            _ => "open_file",
        };
        match launch::open_path(
            &self.shell,
            row.path.clone(),
            Some(&row.kind),
            Some(&row.id),
        ) {
            Ok(()) => {
                engine::record_usage(app_state(), &row.id, action);
            }
            Err(err) => eprintln!("open {}: {err}", row.title),
        }
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
                    .path("icons/search.svg")
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

    fn bento(&self, th: &Theme) -> impl IntoElement {
        let gap = th.inner_gap;
        let inner_w = theme::WINDOW_W - 2.0 * theme::CONTENT_PADDING;
        let cell_w = (inner_w - gap * (GRID_COLS as f32 - 1.0)) / GRID_COLS as f32;
        let grid_h = GRID_ROW_H * GRID_ROWS as f32 + gap * (GRID_ROWS as f32 - 1.0);
        let clock = clock_text();

        let tiles = TILES.iter().enumerate().map(|(i, tile)| {
            let &Tile(caption, value, col, row, col_span, row_span) = tile;
            let x = col as f32 * (cell_w + gap);
            let y = row as f32 * (GRID_ROW_H + gap);
            let w = col_span as f32 * cell_w + (col_span as f32 - 1.0) * gap;
            let h = row_span as f32 * GRID_ROW_H + (row_span as f32 - 1.0) * gap;
            let large = col_span == 2 && row_span == 2;
            let value = if caption == CLOCK {
                clock.clone()
            } else {
                value.to_owned()
            };

            let delay = motion::tile_delay(i);
            let duration = Duration::from_millis(motion::TILE_MS);
            let total = delay + duration;

            card(div(), th)
                .absolute()
                .p(px(theme::TILE_PADDING))
                .flex()
                .flex_col()
                .justify_between()
                .bg(th.tile_face())
                .rounded(px(th.tile_radius()))
                .child(caption_text(caption.to_uppercase(), th))
                .child(
                    div()
                        .text_size(px(if large {
                            theme::TILE_VALUE_SIZE_LARGE
                        } else {
                            theme::TILE_VALUE_SIZE
                        }))
                        .font_weight(FontWeight::BOLD)
                        .truncate()
                        .child(value),
                )
                .with_animation(("tile", i), Animation::new(total), move |tile, progress| {
                    let t = motion::spring(motion::staggered(progress, total, delay, duration));
                    let scale = motion::TILE_SCALE + (1.0 - motion::TILE_SCALE) * t;
                    let (sw, sh) = (w * scale, h * scale);
                    tile.opacity(t.min(1.0))
                        .left(px(x + (w - sw) / 2.0))
                        .top(px(y + (h - sh) / 2.0 + motion::TILE_RISE * (1.0 - t)))
                        .w(px(sw))
                        .h(px(sh))
                })
        });

        let radius = th.tile_radius();
        div()
            .relative()
            .mx(px(theme::CONTENT_PADDING))
            .mt(px(gap))
            .h(px(grid_h))
            .on_children_prepainted(move |cards, _, _| mark_cards(&cards, radius))
            .children(tiles)
    }

    fn results(&self, composing: bool, th: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let rows = self.results.iter().enumerate().map(|(i, row)| {
            let selected = i == self.selected;
            let initial: String = row.title.chars().take(1).collect();
            let detail = row.subtitle.clone().unwrap_or_else(|| row.path.clone());
            div()
                .id(("row", i))
                .h(px(theme::ROW_HEIGHT))
                .px(px(theme::ROW_PADDING_X))
                .flex()
                .items_center()
                .gap(px(theme::SEARCH_GAP))
                .rounded(px(th.control_radius()))
                .when(selected, |el| el.bg(th.selection_fill))
                .on_click(cx.listener(move |this, _, _, _| {
                    this.selected = i;
                    this.open_selected();
                }))
                .child(
                    div()
                        .size(px(theme::ICON_CHIP))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(th.chip_radius()))
                        .bg(th.accent_wash())
                        .text_color(th.accent)
                        .font_weight(FontWeight::BOLD)
                        .child(initial),
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
                        .when(!detail.is_empty(), |col| {
                            col.child(muted_text(detail, th).truncate())
                        }),
                )
                .when(selected, |el| el.child(hint_chip("Enter", th)))
        });

        let hint = if composing {
            "Composing with fcitx5"
        } else if self.results.is_empty() {
            "No match, Esc closes"
        } else {
            "Up/Down to pick, Enter to open"
        };

        // One floating card: rows, then the hint as the card's own footer.
        let radius = th.tile_radius();
        let card = card(div(), th)
            .mx(px(theme::CONTENT_PADDING))
            .mt(px(th.inner_gap))
            .px(px(theme::ROW_INSET))
            .pt(px(theme::ROW_INSET))
            .pb(px(theme::HINT_INSET_BOTTOM))
            .flex()
            .flex_col()
            .gap(px(theme::ROW_SPACING))
            .rounded(px(radius))
            .children(rows)
            .child(
                muted_text(hint, th)
                    .px(px(theme::ROW_PADDING_X))
                    .pt(px(theme::HINT_INSET))
                    .pb(px(theme::HINT_INSET_BOTTOM)),
            );
        div()
            .flex()
            .flex_col()
            .on_children_prepainted(move |cards, _, _| mark_cards(&cards, radius))
            .child(card)
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
        let show_bento = input.text().is_empty();
        let composing = input.is_composing();
        let focus_handle = input.focus_handle.clone();

        let body = div()
            .size_full()
            .flex()
            .flex_col()
            .child(self.top_bar(&th))
            .child(if show_bento {
                self.bento(&th).into_any_element()
            } else {
                self.results(composing, &th, cx).into_any_element()
            });

        // gpui 0.2.2 has no element scale, so the arrive scale is an inset.
        let arrive = Duration::from_millis(motion::ARRIVE_MS);
        let inset_x = theme::WINDOW_W * (1.0 - motion::ARRIVE_SCALE) / 2.0;
        let inset_y = theme::WINDOW_H * (1.0 - motion::ARRIVE_SCALE) / 2.0;

        div()
            .when(pace_probe_wanted(), |root| root.child(pace_probe()))
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

fn caption_text(text: String, th: &Theme) -> Div {
    div()
        .text_size(px(theme::CAPTION_SIZE))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(th.text_muted)
        .child(text)
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

/// `LOOK_PACE_PROBE=1`: one timestamp per frame for tools/pace.sh.
fn pace_probe_wanted() -> bool {
    std::env::var_os("LOOK_PACE_PROBE").is_some()
}

fn clock_text() -> String {
    use chrono::Timelike;
    let now = chrono::Local::now();
    format!("{:02}:{:02}", now.hour(), now.minute())
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
