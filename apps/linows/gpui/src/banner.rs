//! The banner slot above the top bar: a transient toast that hides itself,
//! and a sticky notice that stays until dismissed. A toast wins while it is
//! up; when it expires the notice resurfaces, as the webview's `banner.js`.

use std::time::Duration;

use gpui::{Animation, AnimationExt, Context, FontWeight, Rgba, Task, div, prelude::*, px};

use crate::launcher::Launcher;
use crate::launchpad::Tone;
use crate::motion;
use crate::theme::{self, Theme};

/// The macOS capsule: its padding, the gap before its buttons, and the
/// buttons' own padding.
const PADDING_X: f32 = 10.0;
const PADDING_Y: f32 = 6.0;
const GAP: f32 = 8.0;
const BUTTON_PADDING_X: f32 = 8.0;
const BUTTON_PADDING_Y: f32 = 3.0;
const BUTTON_HOVER_OPACITY: f32 = 0.85;
/// Kept off each side of the row it sits in, so it follows the window's width.
const SIDE_INSET: f32 = 40.0;
const DISMISS: &str = "Dismiss";
const COPY: &str = "Copy";
const COPIED: &str = "Copied";
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
    /// The full text behind a summarised line, offered on a Copy button. A
    /// toast that carries one also gets a close button: it is there to be
    /// read, not glanced at.
    pub copy: Option<String>,
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
        self.show_message(
            Message {
                text,
                tone,
                copy: None,
            },
            seconds,
            cx,
        );
    }

    pub fn show_message(&mut self, message: Message, seconds: f32, cx: &mut Context<Launcher>) {
        self.seq += 1;
        let seq = self.seq;
        self.toast = Some(message);
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

    /// The close button on a toast: gone now, and its timer has nothing to do.
    fn dismiss_toast(&mut self) {
        self.seq += 1;
        self.toast = None;
        self._timer = None;
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

    /// The banner as a capsule in its tone, or nothing; the caller places
    /// it. `on_dismiss` runs when the sticky notice's button is pressed.
    pub fn render(&self, th: &Theme, cx: &mut Context<Launcher>) -> Option<impl IntoElement> {
        let (message, sticky) = match (&self.toast, &self.sticky) {
            (Some(toast), _) => (toast, false),
            (None, Some(sticky)) => (sticky, true),
            (None, None) => return None,
        };
        let fill = tone_colour(message.tone, th);
        let ink = theme::on_fill(fill);
        let banner = div()
            .flex_1()
            .mx(px(SIDE_INSET))
            .px(px(PADDING_X))
            .py(px(PADDING_Y))
            .rounded_full()
            .bg(fill)
            .flex()
            .items_center()
            .gap(px(GAP))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(th.font_size - 1.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(ink)
                    .child(message.text.clone()),
            );
        let seq = self.seq;
        let button = |id: &'static str, label: &'static str| {
            div()
                .id(id)
                .flex_shrink_0()
                .px(px(BUTTON_PADDING_X))
                .py(px(BUTTON_PADDING_Y))
                .rounded_full()
                .bg(th.control_fill)
                .text_size(px(th.font_size - 2.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(ink)
                .cursor_pointer()
                .hover(|s| s.opacity(BUTTON_HOVER_OPACITY))
                .child(label)
        };
        let copy = (!sticky).then(|| message.copy.clone()).flatten();
        let banner = banner.when_some(copy, |el, text| {
            el.child(button("banner-copy", COPY).on_click(
                cx.listener(move |this, _, _, cx| this.copy_text(text.clone(), COPIED.into(), cx)),
            ))
            .child(
                button("banner-close", DISMISS).on_click(cx.listener(|this, _, _, cx| {
                    this.banner.dismiss_toast();
                    cx.notify();
                })),
            )
        });
        let banner = banner.when(sticky, |el| {
            el.child(
                button("banner-dismiss", DISMISS)
                    .on_click(cx.listener(|this, _, _, cx| this.dismiss_health(cx))),
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
