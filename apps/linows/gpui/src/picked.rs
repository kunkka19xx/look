//! Multi-select: the rows picked with Ctrl+P, the panel that lists them in
//! the preview column, and the badge the compact layout shows instead.

use gpui::{Context, Div, FontWeight, SharedString, div, img, prelude::*, px, svg};

use crate::glyphs;
use crate::icons::{IconRequest, IconStore};
use crate::launcher::Launcher;
use crate::rows::Row;
use crate::theme::{self, Theme};

const HEADER_MARGIN: f32 = 10.0;
const ITEM_GAP: f32 = 4.0;
const ITEM_PADDING_X: f32 = 8.0;
const ITEM_PADDING_Y: f32 = 6.0;
const ITEM_INNER_GAP: f32 = 8.0;
const ITEM_ICON: f32 = 24.0;
const BUTTON_PADDING_X: f32 = 6.0;
const BUTTON_PADDING_Y: f32 = 2.0;
const SHORTCUT_PADDING_X: f32 = 5.0;
const BADGE_PADDING_X: f32 = 8.0;
const BADGE_PADDING_Y: f32 = 3.0;
const OPEN_ALL: &str = "Open all";
const OPEN_ALL_CHORD: &str = "Shift+Enter";
const CLEAR_ALL: &str = "Clear all";
const REMOVE: &str = "\u{d7}";

/// Only files and folders carry a path worth collecting; an app, a setting
/// or a clip would leave the panel listing nonsense.
pub fn pickable(row: &Row) -> bool {
    !row.is_hint() && (row.kind == "file" || row.kind == "folder")
}

fn key(row: &Row) -> String {
    format!("{}|{}", row.kind, row.path)
}

#[derive(Default)]
pub struct Picked {
    items: Vec<Row>,
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
        let wanted = key(row);
        self.items.iter().any(|item| key(item) == wanted)
    }

    /// In if it was out, out if it was in.
    pub fn toggle(&mut self, row: &Row) {
        let wanted = key(row);
        match self.items.iter().position(|item| key(item) == wanted) {
            Some(at) => {
                self.items.remove(at);
            }
            None => self.items.push(row.clone()),
        }
    }

    pub fn remove(&mut self, at: usize) {
        if at < self.items.len() {
            self.items.remove(at);
        }
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// The file paths, what the clipboard gets.
    pub fn paths(&self) -> Vec<String> {
        self.items.iter().map(|item| item.path.clone()).collect()
    }

    /// The panel: a header with the two actions, then one row per item.
    pub fn panel(
        &self,
        icons: &gpui::Entity<IconStore>,
        th: &Theme,
        cx: &mut Context<Launcher>,
    ) -> Div {
        let header = div()
            .mb(px(HEADER_MARGIN))
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_size(px(th.font_size - 1.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(format!("Picked ({})", self.items.len())),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(ITEM_GAP))
                    .child(
                        div()
                            .id("picked-open")
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
                            .child(OPEN_ALL)
                            .child(
                                div()
                                    .px(px(SHORTCUT_PADDING_X))
                                    .py(px(theme::CHIP_PADDING_Y))
                                    .rounded(px(th.chip_radius()))
                                    .bg(th.control_fill)
                                    .text_size(px(th.font_size - 3.0))
                                    .text_color(th.text_muted)
                                    .child(OPEN_ALL_CHORD),
                            ),
                    )
                    .child(
                        div()
                            .id("picked-clear")
                            .px(px(BUTTON_PADDING_X))
                            .py(px(BUTTON_PADDING_Y))
                            .rounded(px(th.chip_radius()))
                            .text_size(px(th.font_size - 2.0))
                            .text_color(th.danger)
                            .cursor_pointer()
                            .hover(|s| s.bg(th.selection_fill))
                            .on_click(cx.listener(|this, _, _, cx| this.clear_picked(cx)))
                            .child(CLEAR_ALL),
                    ),
            );

        let items = self.items.iter().enumerate().map(|(i, item)| {
            let picture = icons.update(cx, |store, cx| {
                store.get(
                    IconRequest {
                        kind: item.kind.clone(),
                        path: item.path.clone(),
                        id: Some(item.id.clone()),
                    },
                    cx,
                )
            });
            let icon = div()
                .size(px(ITEM_ICON))
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(th.chip_radius() / 1.5))
                .map(|el| match picture {
                    Some(image) => el.child(img(image).size(px(ITEM_ICON))),
                    None => el
                        .bg(th.accent)
                        .text_color(th.on_accent)
                        .text_size(px(th.font_size - 4.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(SharedString::from(
                            item.title
                                .chars()
                                .next()
                                .unwrap_or(' ')
                                .to_uppercase()
                                .to_string(),
                        )),
                });
            div()
                .flex()
                .items_center()
                .gap(px(ITEM_INNER_GAP))
                .px(px(ITEM_PADDING_X))
                .py(px(ITEM_PADDING_Y))
                .rounded(px(th.control_radius()))
                .bg(th.control_fill)
                .child(icon)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_size(px(th.font_size - 1.0))
                                .font_weight(FontWeight::MEDIUM)
                                .truncate()
                                .child(item.title.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(th.font_size - 4.0))
                                .text_color(th.text_muted)
                                .truncate()
                                .child(item.path.clone()),
                        ),
                )
                .child(
                    div()
                        .id(("picked-remove", i))
                        .px(px(BUTTON_PADDING_X))
                        .text_color(th.text_muted)
                        .cursor_pointer()
                        .hover(|s| s.text_color(th.danger))
                        .on_click(cx.listener(move |this, _, _, cx| this.remove_picked(i, cx)))
                        .child(REMOVE),
                )
        });

        div()
            .size_full()
            .p(px(theme::CONTENT_PADDING))
            .flex()
            .flex_col()
            .child(header)
            .child(div().flex().flex_col().gap(px(ITEM_GAP)).children(items))
    }

    /// The top bar's count, where the compact layout has no panel.
    pub fn badge(&self, th: &Theme) -> Option<Div> {
        (!self.items.is_empty()).then(|| {
            div()
                .flex_shrink_0()
                .px(px(BADGE_PADDING_X))
                .py(px(BADGE_PADDING_Y))
                .rounded_full()
                .bg(th.selection_fill)
                .text_size(px(theme::CAPTION_SIZE))
                .child(format!("{} picked", self.items.len()))
        })
    }
}

/// The check a picked row wears at its end.
pub fn check(th: &Theme) -> impl IntoElement {
    svg()
        .path(glyphs::CHECK)
        .size(px(theme::SEARCH_ICON))
        .text_color(th.accent)
}
