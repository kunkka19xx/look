//! The banner slot above the top bar: a transient toast that hides itself,
//! and a sticky notice that stays until dismissed. A toast wins while it is
//! up; when it expires the notice resurfaces, as the webview's `banner.js`.

use std::time::Duration;

use gpui::{Animation, AnimationExt, Context, FontWeight, Rgba, Task, div, prelude::*, px, svg};

use crate::glyphs;
use crate::launcher::Launcher;
use crate::launchpad::Tone;
use crate::motion;
use crate::theme::{self, Theme};

const PADDING_Y: f32 = 8.0;
const GAP: f32 = 12.0;
const WASH: f32 = 0.16;
const DOT: f32 = 8.0;
/// Fixed, so a heavy theme border does not turn a toast into a frame.
const BORDER: f32 = 1.0;
/// How much of the tone the border takes.
const BORDER_MIX: f32 = 0.55;
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

    /// A toast is up, rather than the sticky notice or nothing: the home
    /// floats it over the body, the sticky notice takes a place in the flow.
    pub fn showing_toast(&self) -> bool {
        self.toast.is_some()
    }

    /// The banner as a card, or nothing; the caller places it. `on_dismiss`
    /// runs when the sticky notice's button is pressed.
    /// `floating` is the toast over the body: its face is opaque, or the
    /// row under it would read through.
    pub fn render(
        &self,
        th: &Theme,
        floating: bool,
        cx: &mut Context<Launcher>,
    ) -> Option<impl IntoElement> {
        let (message, sticky) = match (&self.toast, &self.sticky) {
            (Some(toast), _) => (toast, false),
            (None, Some(sticky)) => (sticky, true),
            (None, None) => return None,
        };
        let colour = tone_colour(message.tone, th);
        let banner = div()
            .px(px(theme::INPUT_PADDING_X))
            .py(px(PADDING_Y))
            .rounded(px(th.bar_radius()))
            .bg({
                let face = theme::wash(th.card_face(), colour, WASH);
                if floating { theme::opaque(face) } else { face }
            })
            .border(px(BORDER))
            .border_color(theme::mix(colour, th.border, BORDER_MIX))
            .shadow(th.card_shadow())
            .flex()
            .items_center()
            .gap(px(GAP))
            // The tone rides on a dot and the border; the words stay in the
            // theme's text colour so they read on any wash.
            .child(
                div()
                    .size(px(DOT))
                    .flex_shrink_0()
                    .rounded_full()
                    .bg(colour),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(th.font_size))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(th.text)
                    .child(message.text.clone()),
            );
        let seq = self.seq;
        let banner = banner.when(sticky, |el| {
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
        });
        // In from just above, keyed to this showing.
        Some(banner.with_animation(
            ("banner-in", seq),
            Animation::new(motion::dur(motion::BANNER_MS)),
            |el, t| {
                let t = motion::curve(t);
                el.opacity(t)
                    .top(px(motion::rise(0.0, motion::BANNER_RISE, t)))
            },
        ))
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
