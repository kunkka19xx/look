//! The banner slot above the top bar: a transient toast that hides itself,
//! and a sticky notice that stays until dismissed. A toast wins while it is
//! up; when it expires the notice resurfaces, as the webview's `banner.js`.

use std::time::Duration;

use gpui::{Context, Div, FontWeight, Rgba, Task, div, prelude::*, px, svg};

use crate::glyphs;
use crate::launcher::Launcher;
use crate::launchpad::Tone;
use crate::theme::{self, Theme};

const PADDING_Y: f32 = 8.0;
const GAP: f32 = 12.0;
const WASH: f32 = 0.16;
const DISMISS: &str = "Dismiss";
pub const SHORT: f32 = 1.0;
pub const MEDIUM: f32 = 1.2;
pub const LONG: f32 = 1.6;
pub const READ: f32 = 2.0;
/// Matches the macOS info banner for a clipboard action.
pub const CLIP: f32 = 1.1;
/// A refused paste is a sentence to read, not a flash.
pub const PASTE_BLOCKED: f32 = 3.0;

#[derive(Clone)]
pub struct Message {
    pub text: String,
    pub tone: Tone,
}

#[derive(Default)]
pub struct Banner {
    toast: Option<Message>,
    sticky: Option<Message>,
    /// Identifies the toast on screen, so an older timer cannot clear a newer one.
    seq: u64,
    _timer: Option<Task<()>>,
}

impl Banner {
    pub fn show(&mut self, text: String, tone: Tone, seconds: f32, cx: &mut Context<Launcher>) {
        self.seq += 1;
        let seq = self.seq;
        self.toast = Some(Message { text, tone });
        self._timer = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_secs_f32(seconds))
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.banner.seq == seq {
                    this.banner.toast = None;
                    cx.notify();
                }
            });
        }));
        cx.notify();
    }

    /// The sticky notice, or none to clear it.
    pub fn set_sticky(&mut self, message: Option<Message>) {
        self.sticky = message;
    }

    /// The banner as a card above the bar, or nothing. `on_dismiss` runs
    /// when the sticky notice's button is pressed.
    pub fn render(&self, th: &Theme, cx: &mut Context<Launcher>) -> Option<Div> {
        let (message, sticky) = match (&self.toast, &self.sticky) {
            (Some(toast), _) => (toast, false),
            (None, Some(sticky)) => (sticky, true),
            (None, None) => return None,
        };
        let colour = tone_colour(message.tone, th);
        let banner = div()
            .mx(px(theme::CONTENT_PADDING))
            .mt(px(theme::CONTENT_PADDING))
            .px(px(theme::INPUT_PADDING_X))
            .py(px(PADDING_Y))
            .rounded(px(th.bar_radius()))
            .bg(theme::wash(th.card_face(), colour, WASH))
            .border(px(th.border_thickness))
            .border_color(th.border)
            .shadow(th.card_shadow())
            .flex()
            .items_center()
            .gap(px(GAP))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(th.font_size - 1.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(colour)
                    .child(message.text.clone()),
            );
        Some(banner.when(sticky, |el| {
            el.child(
                div()
                    .id("banner-dismiss")
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap(px(GAP / 3.0))
                    .text_size(px(th.font_size - 1.0))
                    .text_color(th.text_muted)
                    .cursor_pointer()
                    .hover(|s| s.text_color(th.text))
                    .on_click(cx.listener(|this, _, _, cx| this.dismiss_health(cx)))
                    .child(
                        svg()
                            .path(glyphs::X_CIRCLE)
                            .size(px(th.font_size))
                            .text_color(th.text_muted),
                    )
                    .child(DISMISS),
            )
        }))
    }
}

pub fn tone_colour(tone: Tone, th: &Theme) -> Rgba {
    match tone {
        Tone::Success => th.success,
        Tone::Info => th.accent,
        Tone::Warning => th.warning,
        Tone::Error => th.danger,
    }
}
