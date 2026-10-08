//! The confirm bar: one question over the bottom edge, answered with Y or N
//! (Enter and Escape too), or the two buttons. While it is up it owns the
//! keys. Used by the destructive chords: emptying the trash, hiding an app.

use gpui::{Animation, AnimationExt, Context, FontWeight, div, prelude::*, px, svg};

use crate::launcher::Launcher;
use crate::motion;
use crate::theme::{self, Theme};

const PADDING_X: f32 = 14.0;
const PADDING_Y: f32 = 10.0;
const GAP: f32 = 14.0;
const ICON: f32 = 26.0;
const BUTTON_GAP: f32 = 6.0;
const BUTTON_PADDING_X: f32 = 16.0;
const BUTTON_PADDING_Y: f32 = 6.0;
const BORDER_MIX: f32 = 0.85;
const YES_LABEL: &str = "Y / Yes";
const NO_LABEL: &str = "N / No";

/// What runs on yes; the bar is gone by then.
pub type OnYes = Box<dyn FnOnce(&mut Launcher, &mut Context<Launcher>)>;

pub struct Confirm {
    pub title: String,
    pub detail: String,
    pub glyph: Option<&'static str>,
    pub on_yes: Option<OnYes>,
}

impl Confirm {
    /// The bar as a card at the bottom of the window, rising in; `seq`
    /// keys the entrance to this asking.
    pub fn render(&self, seq: u64, th: &Theme, cx: &mut Context<Launcher>) -> impl IntoElement {
        let button = |id: &'static str, label: &'static str, yes: bool| {
            div()
                .id(id)
                .px(px(BUTTON_PADDING_X))
                .py(px(BUTTON_PADDING_Y))
                .rounded_full()
                .text_size(px(th.font_size - 1.0))
                .font_weight(FontWeight::SEMIBOLD)
                .cursor_pointer()
                .map(|el| {
                    if yes {
                        el.bg(th.danger).text_color(th.on_accent)
                    } else {
                        el.bg(th.control_fill).text_color(th.text)
                    }
                })
                .hover(|s| s.opacity(0.85))
                .on_click(cx.listener(move |this, _, _, cx| this.settle_confirm(yes, cx)))
                .child(label)
        };
        div()
            .absolute()
            .left(px(theme::CONTENT_PADDING))
            .right(px(theme::CONTENT_PADDING))
            .bottom(px(theme::CONTENT_PADDING))
            .px(px(PADDING_X))
            .py(px(PADDING_Y))
            .rounded(px(th.control_radius()))
            .bg(th.card_face())
            .border(px(1.0))
            .border_color(theme::mix(th.danger, th.border, BORDER_MIX))
            .shadow(th.card_shadow())
            .flex()
            .items_center()
            .gap(px(GAP))
            .when_some(self.glyph, |el, glyph| {
                el.child(svg().path(glyph).size(px(ICON)).text_color(th.danger))
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(self.title.clone()),
                    )
                    .when(!self.detail.is_empty(), |col| {
                        col.child(
                            div()
                                .text_size(px(th.font_size - 2.0))
                                .text_color(th.text_muted)
                                .child(self.detail.clone()),
                        )
                    }),
            )
            .child(
                div()
                    .flex()
                    .gap(px(BUTTON_GAP))
                    .child(button("confirm-yes", YES_LABEL, true))
                    .child(button("confirm-no", NO_LABEL, false)),
            )
            .with_animation(
                ("confirm-in", seq),
                Animation::new(motion::dur(motion::CONFIRM_MS)),
                |bar, t| {
                    let t = motion::curve(t);
                    bar.opacity(t).bottom(px(motion::rise(
                        theme::CONTENT_PADDING,
                        -motion::CONFIRM_RISE,
                        t,
                    )))
                },
            )
    }
}
