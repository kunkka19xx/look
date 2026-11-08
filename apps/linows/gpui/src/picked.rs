//! Multi-select: the rows picked with Ctrl+P and the pills that list them
//! over the results, leaving the preview column to the selection.

use gpui::{Context, Div, ScrollHandle, div, img, prelude::*, px, svg};

use crate::controls;
use crate::glyphs;
use crate::icons::{IconRequest, IconStore};
use crate::launcher::Launcher;
use crate::rows::Row;
use crate::theme::{self, Theme};

const STRIP_GAP: f32 = 6.0;
const PILL_GAP: f32 = 6.0;
const PILL_PADDING_L: f32 = 8.0;
const PILL_PADDING_R: f32 = 3.0;
const PILL_PADDING_Y: f32 = 3.0;
const PILL_ICON: f32 = 16.0;
const PILL_MAX_W: f32 = 220.0;
const REMOVE_PADDING: f32 = 4.0;
const BUTTON_PADDING_X: f32 = 6.0;
const BUTTON_PADDING_Y: f32 = 2.0;
const SHORTCUT_PADDING_X: f32 = 5.0;
const OPEN_ALL: &str = "Open all";
const OPEN_ALL_CHORD: &str = "Shift+Enter";
const CLEAR: &str = "Clear";

/// Only files and folders carry a path worth collecting; an app, a setting
/// or a clip would leave the pills listing nonsense.
pub fn pickable(row: &Row) -> bool {
    !row.is_hint() && (row.kind == "file" || row.kind == "folder")
}

fn same(a: &Row, b: &Row) -> bool {
    a.kind == b.kind && a.path == b.path
}

#[derive(Default)]
pub struct Picked {
    items: Vec<Row>,
    scroll: ScrollHandle,
}

impl Picked {
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn items(&self) -> &[Row] {
        &self.items
    }

    pub fn contains(&self, row: &Row) -> bool {
        self.items.iter().any(|item| same(item, row))
    }

    /// In if it was out, out if it was in.
    pub fn toggle(&mut self, row: &Row) {
        match self.items.iter().position(|item| same(item, row)) {
            Some(at) => {
                self.items.remove(at);
            }
            None => {
                self.items.push(row.clone());
                self.scroll.scroll_to_item(self.items.len() - 1);
            }
        }
    }

    pub fn remove(&mut self, at: usize) {
        if at < self.items.len() {
            self.items.remove(at);
        }
    }

    /// The picks a hide kept, back in a fresh launcher.
    pub fn restore(&mut self, items: Vec<Row>) {
        self.items = items;
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// The file paths, what the clipboard gets.
    pub fn paths(&self) -> Vec<String> {
        self.items.iter().map(|item| item.path.clone()).collect()
    }

    /// One pill per pick with its (x), then Open all and Clear; scrolls sideways.
    pub fn strip(
        &self,
        icons: &gpui::Entity<IconStore>,
        th: &Theme,
        cx: &mut Context<Launcher>,
    ) -> Option<Div> {
        if self.items.is_empty() {
            return None;
        }
        let pills: Vec<_> = self
            .items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let picture = icons.update(cx, |store, cx| {
                    store.get(
                        IconRequest {
                            kind: item.kind.clone(),
                            path: item.path.clone(),
                            id: Some(item.id.clone()),
                            size: PILL_ICON,
                        },
                        cx,
                    )
                });
                div()
                    .flex_shrink_0()
                    .max_w(px(PILL_MAX_W))
                    .pl(px(PILL_PADDING_L))
                    .pr(px(PILL_PADDING_R))
                    .py(px(PILL_PADDING_Y))
                    .rounded_full()
                    .bg(th.control_fill)
                    .flex()
                    .items_center()
                    .gap(px(PILL_GAP))
                    .children(picture.map(|image| img(image).size(px(PILL_ICON)).flex_shrink_0()))
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_size(px(th.font_size - 2.0))
                            .text_color(th.text_secondary)
                            .child(item.title.clone()),
                    )
                    .child(
                        div()
                            .id(("picked-remove", i))
                            .flex_shrink_0()
                            .group(controls::REMOVE_GROUP)
                            .p(px(REMOVE_PADDING))
                            .rounded_full()
                            .cursor_pointer()
                            .hover(|s| s.bg(th.selection_fill))
                            .on_click(cx.listener(move |this, _, _, cx| this.remove_picked(i, cx)))
                            .child(controls::remove_icon(th)),
                    )
            })
            .collect();
        let open_all = div()
            .id("picked-open")
            .flex_shrink_0()
            .px(px(BUTTON_PADDING_X))
            .py(px(BUTTON_PADDING_Y))
            .rounded(px(th.chip_radius()))
            .flex()
            .items_center()
            .gap(px(BUTTON_PADDING_X))
            .text_size(px(th.font_size - 2.0))
            .text_color(th.accent)
            .cursor_pointer()
            .hover(|s| s.bg(th.selection_fill))
            .on_click(cx.listener(|this, _, _, cx| this.open_all_picked(cx)))
            .child(format!("{OPEN_ALL} ({})", self.items.len()))
            .child(
                div()
                    .px(px(SHORTCUT_PADDING_X))
                    .py(px(theme::CHIP_PADDING_Y))
                    .rounded(px(th.chip_radius()))
                    .bg(th.control_fill)
                    .text_size(px(th.font_size - 3.0))
                    .text_color(th.text_muted)
                    .child(OPEN_ALL_CHORD),
            );
        let clear = div()
            .id("picked-clear")
            .flex_shrink_0()
            .px(px(BUTTON_PADDING_X))
            .py(px(BUTTON_PADDING_Y))
            .rounded(px(th.chip_radius()))
            .text_size(px(th.font_size - 2.0))
            .text_color(th.danger)
            .cursor_pointer()
            .hover(|s| s.bg(th.selection_fill))
            .on_click(cx.listener(|this, _, _, cx| this.clear_picked(cx)))
            .child(CLEAR);
        Some(
            div()
                .flex_shrink_0()
                .px(px(theme::ROW_PADDING_X))
                .pb(px(STRIP_GAP))
                .flex()
                .items_center()
                .gap(px(STRIP_GAP))
                .child(
                    div()
                        .id("picked-pills")
                        .flex_1()
                        .min_w_0()
                        .overflow_x_scroll()
                        .track_scroll(&self.scroll)
                        .flex()
                        .items_center()
                        .gap(px(PILL_GAP))
                        .children(pills),
                )
                .child(open_all)
                .child(clear),
        )
    }
}

/// The check a picked row wears at its end.
pub fn check(th: &Theme) -> impl IntoElement {
    svg()
        .path(glyphs::CHECK)
        .size(px(theme::SEARCH_ICON))
        .text_color(th.accent)
}
