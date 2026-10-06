//! `/speed`: two counter-rotating comets, download on the outer ring and
//! upload on the inner one, around a centre that beats once per round trip.
//! Geometry and pacing are the macOS SpeedGaugeView's; the measurement is the
//! shared look-netspeed crate, run by `crate::speed`.

use std::time::{Duration, Instant};

use gpui::{
    ColorExt, Context, Div, FontWeight, Hsla, Keystroke, PathBuilder, Pixels, Point, Stateful,
    Task, Window, canvas, div, point, prelude::*, px, svg,
};

use super::draw::{Type, paint_circle, paint_dashed_circle, paint_dot, paint_text};
use super::{Commands, KeyOutcome};
use crate::bg;
use crate::glyphs;
use crate::launcher::Launcher;
use crate::speed::{self, FRESH_SECS, FULL_TURN, MAX_MBPS, position_of_bits, position_of_mbps};
use crate::theme::{self, Theme};

pub const ID: &str = "speed";
const SUBTITLE: &str = "Read-only command";
const TICK: Duration = Duration::from_secs(1);
/// How long a chip reads "copied" before going back to the address.
const COPIED_FEEDBACK: Duration = Duration::from_millis(1400);
/// Matches core's own word for a phase that measured nothing.
const UNAVAILABLE: &str = "n/a";
/// Stands in for the public address until it is revealed. Fixed width, so
/// the line doesn't jump when it is.
const MASKED_ADDRESS: &str = "\u{2022}\u{2022}\u{2022}.\u{2022}\u{2022}\u{2022}.\u{2022}\u{2022}\u{2022}.\u{2022}\u{2022}\u{2022}";
const NO_NETWORK: &str = "No network";
const COPIED: &str = "copied";
const LATENCY_CAPTION: &str = "MS LATENCY";
const PRESS_R: &str = "Press R to measure";
const JUST_NOW: &str = "just now";
const SEPARATOR: &str = "  \u{b7}  ";
const ADDRESS_SEPARATOR: &str = "\u{b7}";
const LEGEND: [(&str, Series); 3] = [
    ("DOWN", Series::Download),
    ("UP", Series::Upload),
    ("LATENCY", Series::Latency),
];

const STACK_GAP: f32 = 14.0;
/// The dial takes the height that is going, down to the macOS gaugeMinSize.
const GAUGE_MIN: f32 = 210.0;
/// Dims the standing reading while a fresh one is being measured.
const SUPERSEDED_OPACITY: f32 = 0.5;
const ADDRESS_GAP: f32 = 6.0;
const ADDRESS_ICON: f32 = 14.0;
const LEGEND_GAP: f32 = 28.0;
const LEGEND_ENTRY_GAP: f32 = 7.0;
const LEGEND_DOT: f32 = 7.0;
const LEGEND_LABEL_SPACING: f32 = 1.4;
const VERDICT_GAP: f32 = 3.0;
const VERDICT_SPACING: f32 = 1.0;

/// Room outside the download ring for its tick labels.
const LABEL_ROOM: f32 = 30.0;
/// The upload ring, as a fraction of the download ring.
const INNER_RATIO: f32 = 0.7;
const RING_DASH: [f32; 2] = [2.0, 5.0];
const RING_W: f32 = 1.0;
const TICK_GAP: f32 = 4.0;
const MAJOR_TICK: f32 = 9.0;
const MINOR_TICK: f32 = 4.0;
const MAJOR_TICK_W: f32 = 1.5;
const MINOR_TICK_W: f32 = 1.0;
const LABEL_GAP: f32 = 21.0;
const LABEL_SIZE_DELTA: f32 = -5.0;
/// Ticks are labelled at each decade and marked at the 2.5x and 5x steps.
const MAJOR_MBPS: [f64; 4] = [1.0, 10.0, 100.0, 1000.0];
const MINOR_MBPS: [f64; 6] = [2.5, 5.0, 25.0, 50.0, 250.0, 500.0];
/// A comet's tail grows with the rate: a trickle is a short stub, a fast
/// link nearly laps the ring.
const MIN_TAIL_DEGREES: f32 = 10.0;
const TAIL_DEGREES_SPAN: f32 = 330.0;
const MIN_TAIL_W: f32 = 1.5;
const MIN_TAIL_OPACITY: f32 = 0.18;
const TAIL_OPACITY_SPAN: f32 = 0.5;
const PULSE_START_R: f32 = 8.0;
const PULSE_W: f32 = 1.0;
const PULSE_OPACITY: f32 = 0.35;
/// Rotation is measured from straight up, where the scale starts.
const TOP_OF_DIAL: f32 = -90.0;
const CENTRE_VALUE_SCALE: f32 = 2.2;
const CENTRE_CAPTION_SPACING: f32 = 2.0;
/// The value sits this many caption heights above the centre, the caption
/// this many value heights plus this many of its own below it.
const VALUE_ABOVE_CENTRE: f32 = 0.6;
const CAPTION_BELOW_CENTRE: f32 = 0.42;
const CAPTION_OWN_OFFSET: f32 = 0.4;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Series {
    Download,
    Upload,
    Latency,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Address {
    Lan,
    Wan,
}

impl Address {
    fn label(self) -> &'static str {
        match self {
            Address::Lan => "LAN",
            Address::Wan => "WAN",
        }
    }
}

/// Which ring a comet runs on and how it is drawn. Download runs clockwise
/// with its tail behind; upload runs the other way.
struct Comet {
    colour: Hsla,
    head_r: f32,
    tail_w_span: f32,
    trailing: bool,
}

const DOWNLOAD_HEAD_R: f32 = 5.5;
const DOWNLOAD_TAIL_W_SPAN: f32 = 5.0;
const UPLOAD_HEAD_R: f32 = 4.5;
const UPLOAD_TAIL_W_SPAN: f32 = 4.0;

#[derive(Default)]
pub struct Panel {
    /// The public address starts hidden: it identifies the connection, and
    /// this panel is a screenshot away from anywhere.
    reveals_public: bool,
    copied: Option<Address>,
    _copied_timer: Option<Task<()>>,
    _tick: Option<Task<()>>,
}

impl Panel {
    pub fn enter(&mut self, cx: &mut Context<Launcher>) {
        self.reveals_public = false;
        speed::lock().enter();
        // The elapsed count and a landed reading, when the dial is still.
        self._tick = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        }));
    }

    pub fn leave(&mut self) {
        self._tick = None;
        self._copied_timer = None;
        self.copied = None;
        speed::lock().motion.pause();
    }

    pub fn key(&mut self, ks: &Keystroke, cx: &mut Context<Launcher>) -> KeyOutcome {
        if ks.modifiers.control || ks.modifiers.alt || ks.modifiers.platform {
            return KeyOutcome::Pass;
        }
        match ks.key.as_str() {
            "r" => speed::lock().start(),
            "e" => self.reveals_public = !self.reveals_public,
            _ => return KeyOutcome::Pass,
        }
        cx.notify();
        KeyOutcome::Consumed
    }

    fn copy(&mut self, kind: Address, address: String, cx: &mut Context<Launcher>) {
        bg::fetch(
            cx,
            move || linows_backend::clipboard::copy_to_clipboard(&address),
            move |this, outcome, cx| {
                if outcome.is_ok() {
                    this.commands.speed.mark_copied(kind, cx);
                }
            },
        );
    }

    fn mark_copied(&mut self, kind: Address, cx: &mut Context<Launcher>) {
        self.copied = Some(kind);
        self._copied_timer = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(COPIED_FEEDBACK).await;
            let _ = this.update(cx, |this, cx| {
                this.commands.speed.copied = None;
                cx.notify();
            });
        }));
    }
}

/// What one render shows, read under the lock in one go.
struct View {
    download: f64,
    upload: f64,
    latency_ms: Option<f64>,
    latency_level: &'static str,
    values: [String; 3],
    verdict: Option<String>,
    carrier: Option<String>,
    public_ip: Option<String>,
    local: Option<String>,
    status: String,
    error: Option<String>,
    running: bool,
}

impl View {
    fn read() -> Self {
        let speed = speed::lock();
        let running = speed.running();
        let reading = speed.reading();
        let status = if running {
            format!("Measuring, {}s", speed.elapsed_secs())
        } else {
            reading.map_or_else(
                || PRESS_R.to_string(),
                |r| format!("Measured {}, press R to run again", age(r.measured_at_unix)),
            )
        };
        let text = |field: fn(&speed::SpeedReading) -> &str| {
            reading.map_or_else(|| UNAVAILABLE.to_string(), |r| field(r).to_string())
        };
        Self {
            download: reading.map_or(0.0, |r| r.download_bits_per_second),
            upload: reading.map_or(0.0, |r| r.upload_bits_per_second),
            latency_ms: reading.and_then(|r| r.latency_ms),
            latency_level: reading.map_or("unknown", |r| r.latency_level),
            values: [
                text(|r| &r.download_display),
                text(|r| &r.upload_display),
                text(|r| &r.latency_display),
            ],
            verdict: reading.filter(|_| !running).map(|r| {
                format!(
                    "{} \u{b7} latency {}",
                    r.download_verdict, r.latency_verdict
                )
                .to_uppercase()
            }),
            carrier: reading.and_then(|r| {
                let parts: Vec<String> = [
                    r.provider.clone(),
                    r.location.clone(),
                    r.download_source.as_deref().map(|s| format!("via {s}")),
                ]
                .into_iter()
                .flatten()
                .collect();
                (!parts.is_empty()).then(|| parts.join(SEPARATOR))
            }),
            public_ip: reading.and_then(|r| r.public_ip.clone()),
            local: speed.local_address().map(str::to_string),
            status,
            error: speed.error().map(str::to_string),
            running,
        }
    }
}

/// "just now" inside the reuse window, then the largest whole unit.
fn age(measured_at_unix: i64) -> String {
    let seconds = speed::now_unix() - measured_at_unix;
    if seconds < FRESH_SECS {
        return JUST_NOW.to_string();
    }
    let (count, unit) = match seconds {
        s if s < 3600 => (s / 60, "minute"),
        s if s < 86400 => (s / 3600, "hour"),
        s => (s / 86400, "day"),
    };
    match (count, unit) {
        (1, "day") => "yesterday".to_string(),
        (1, unit) => format!("1 {unit} ago"),
        (n, unit) => format!("{n} {unit}s ago"),
    }
}

pub fn panel(frame: &Commands, th: &Theme, cx: &mut Context<Launcher>) -> Div {
    let state = &frame.speed;
    let view = View::read();
    let dim = |el: Div| el.when(view.running, |el| el.opacity(SUPERSEDED_OPACITY));
    let mono = |delta: f32| {
        div()
            .font_family(th.mono_family.clone())
            .text_size(px(th.font_size + delta))
    };
    let legend = dim(div().flex().items_center().gap(px(LEGEND_GAP)).children(
        LEGEND
            .iter()
            .zip(view.values.iter())
            .map(|((label, series), value)| {
                let colour = match series {
                    Series::Download => th.success,
                    Series::Upload => th.accent,
                    Series::Latency => th.warning,
                };
                div()
                    .flex()
                    .items_center()
                    .gap(px(LEGEND_ENTRY_GAP))
                    .child(div().size(px(LEGEND_DOT)).rounded_full().bg(colour))
                    .child(
                        mono(-2.0)
                            .letter_spacing(px(LEGEND_LABEL_SPACING))
                            .text_color(th.text_muted)
                            .child(*label),
                    )
                    .child(
                        mono(2.0)
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(th.text)
                            .child(value.clone()),
                    )
            }),
    ));
    let verdict = div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(VERDICT_GAP))
        .child(
            div()
                .text_size(px(th.font_size - 1.0))
                .text_color(th.text_secondary)
                .child(view.status.clone()),
        )
        .children(view.verdict.clone().map(|text| {
            mono(-3.0)
                .letter_spacing(px(VERDICT_SPACING))
                .text_color(th.text_muted)
                .child(text)
        }))
        .children(view.error.clone().map(|text| {
            div()
                .text_size(px(th.font_size - 1.0))
                .text_color(th.danger)
                .child(text)
        }));
    let carrier = view.carrier.clone().map(|text| {
        mono(-3.0)
            .max_w_full()
            .truncate()
            .text_color(th.text_muted)
            .child(text)
    });
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .child(
            frame
                .bar(th)
                .child(div().flex_1().text_color(th.text_secondary).child(SUBTITLE))
                .child(frame.pill(th)),
        )
        .child(
            frame
                .content()
                .items_center()
                .justify_center()
                .gap(px(STACK_GAP))
                .overflow_hidden()
                .child(addresses(state, &view, th, cx))
                .child(dim(div()
                    .w_full()
                    .flex_1()
                    .min_h(px(GAUGE_MIN))
                    .child(gauge(&view, th))))
                .child(legend)
                .child(verdict)
                .children(carrier),
        )
}

/// Both addresses, since they answer different questions: LAN is what you
/// reach this machine on, WAN is what the far end of the test sees.
fn addresses(state: &Panel, view: &View, th: &Theme, cx: &mut Context<Launcher>) -> Div {
    let line = div()
        .flex()
        .items_center()
        .gap(px(ADDRESS_GAP))
        .font_family(th.mono_family.clone())
        .text_size(px(th.font_size - 2.0))
        .text_color(th.text_muted)
        .whitespace_nowrap()
        .child(
            svg()
                .path(glyphs::GLOBE)
                .size(px(ADDRESS_ICON))
                .text_color(th.text_muted),
        );
    let line = match (&view.local, &view.public_ip) {
        (None, None) => line.child(NO_NETWORK),
        _ => line,
    };
    let line = match &view.local {
        Some(local) => line.child(chip(state, Address::Lan, local, local, th, cx)),
        None => line,
    };
    match &view.public_ip {
        Some(public) => {
            let shown = if state.reveals_public {
                public.as_str()
            } else {
                MASKED_ADDRESS
            };
            let reveal = div()
                .id("speed-reveal")
                .flex()
                .items_center()
                .cursor_pointer()
                .hover(|s| s.text_color(th.text))
                .on_click(cx.listener(|this, _, _, cx| {
                    let panel = &mut this.commands.speed;
                    panel.reveals_public = !panel.reveals_public;
                    cx.notify();
                }))
                .child(
                    svg()
                        .path(if state.reveals_public {
                            glyphs::EYE_OFF
                        } else {
                            glyphs::EYE
                        })
                        .size(px(ADDRESS_ICON)),
                );
            line.when(view.local.is_some(), |el| el.child(ADDRESS_SEPARATOR))
                .child(chip(state, Address::Wan, public, shown, th, cx))
                .child(reveal)
        }
        None => line,
    }
}

/// One address, click to copy. A masked public address still copies in full:
/// hiding it is about what the screen shows, not what you can take with you.
/// The confirmation is an overlay rather than a swapped label, so it cannot
/// shift the line under it.
fn chip(
    state: &Panel,
    kind: Address,
    address: &str,
    shown: &str,
    th: &Theme,
    cx: &mut Context<Launcher>,
) -> Stateful<Div> {
    let copied = state.copied == Some(kind);
    let address = address.to_string();
    div()
        .id(kind.label())
        .relative()
        .cursor_pointer()
        .hover(|s| s.text_color(th.text))
        .on_click(cx.listener(move |this, _, _, cx| {
            this.commands.speed.copy(kind, address.clone(), cx);
        }))
        .child(
            div()
                .when(copied, |el| el.opacity(0.0))
                .child(format!("{} {shown}", kind.label())),
        )
        .when(copied, |el| {
            el.child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(th.success)
                    .child(COPIED),
            )
        })
}

/// The dial's geometry for one canvas size.
struct Dial {
    centre: Point<Pixels>,
    download_r: f32,
    upload_r: f32,
}

impl Dial {
    fn fit(size: gpui::Size<Pixels>, centre: Point<Pixels>) -> Self {
        let download_r =
            (f32::from(size.width).min(f32::from(size.height)) / 2.0 - LABEL_ROOM).max(1.0);
        Self {
            centre,
            download_r,
            upload_r: download_r * INNER_RATIO,
        }
    }

    fn point(&self, radius: f32, degrees: f32) -> Point<Pixels> {
        let radians = (degrees + TOP_OF_DIAL).to_radians();
        point(
            self.centre.x + px(radius * radians.cos()),
            self.centre.y + px(radius * radians.sin()),
        )
    }

    fn arc(&self, builder: &mut PathBuilder, radius: f32, from: f32, to: f32) {
        builder.move_to(self.point(radius, from));
        builder.arc_to(
            point(px(radius), px(radius)),
            px(0.0),
            to - from > FULL_TURN / 2.0,
            true,
            self.point(radius, to),
        );
    }
}

/// Theme colours the dial draws with, resolved once per render.
struct Palette {
    ring: Hsla,
    major_tick: Hsla,
    label: Hsla,
    download: Hsla,
    upload: Hsla,
    latency: Hsla,
}

impl Palette {
    fn of(th: &Theme, latency_level: &str) -> Self {
        Self {
            ring: theme::hsla_of(th.border),
            major_tick: theme::hsla_of(th.text_secondary),
            label: theme::hsla_of(th.text_muted),
            download: theme::hsla_of(th.success),
            upload: theme::hsla_of(th.accent),
            latency: theme::hsla_of(match latency_level {
                "bad" => th.danger,
                "warn" => th.warning,
                "good" => th.success,
                _ => th.text_muted,
            }),
        }
    }
}

fn gauge(view: &View, th: &Theme) -> impl IntoElement {
    let palette = Palette::of(th, view.latency_level);
    let mono = th.mono_family.clone();
    let font_size = th.font_size;
    let (download, upload, latency_ms) = (view.download, view.upload, view.latency_ms);
    let latency_value = view.values[2]
        .split(' ')
        .next()
        .unwrap_or(UNAVAILABLE)
        .to_string();
    canvas(
        move |_, _, _| (),
        move |bounds, (), window, cx| {
            let dial = Dial::fit(bounds.size, bounds.center());
            // A still dial never eases, so the reading is placed directly;
            // so is the first frame, as macOS does on appear.
            let moving = !cx.reduce_motion();
            let (download_angle, upload_angle, pulse, shown_download, shown_upload) = {
                let mut speed = speed::lock();
                let motion = &mut speed.motion;
                if !moving || motion.is_unstarted() {
                    motion.snap(download, upload);
                }
                if moving {
                    motion.advance(Instant::now(), download, upload, latency_ms);
                }
                (
                    motion.download_angle,
                    motion.upload_angle,
                    motion.pulse,
                    motion.shown_download,
                    motion.shown_upload,
                )
            };
            paint_scale(window, cx, &dial, &palette, &mono, font_size);
            if latency_ms.is_some() {
                paint_pulse(window, &dial, &palette, pulse);
            }
            paint_comet(
                window,
                &dial,
                dial.download_r,
                download_angle,
                shown_download,
                &Comet {
                    colour: palette.download,
                    head_r: DOWNLOAD_HEAD_R,
                    tail_w_span: DOWNLOAD_TAIL_W_SPAN,
                    trailing: true,
                },
            );
            paint_comet(
                window,
                &dial,
                dial.upload_r,
                upload_angle,
                shown_upload,
                &Comet {
                    colour: palette.upload,
                    head_r: UPLOAD_HEAD_R,
                    tail_w_span: UPLOAD_TAIL_W_SPAN,
                    trailing: false,
                },
            );
            paint_centre(
                window,
                cx,
                &dial,
                &palette,
                &mono,
                font_size,
                &latency_value,
            );
            if moving {
                window.request_animation_frame();
            }
        },
    )
    .size_full()
}

/// The rings, the ticks and their labels. Nothing here moves.
fn paint_scale(
    window: &mut Window,
    cx: &mut gpui::App,
    dial: &Dial,
    palette: &Palette,
    mono: &str,
    font_size: f32,
) {
    for radius in [dial.download_r, dial.upload_r] {
        paint_dashed_circle(
            window,
            dial.centre,
            radius,
            RING_W,
            &RING_DASH,
            palette.ring,
        );
    }
    let outer_r = dial.download_r + TICK_GAP;
    let tick = |window: &mut Window, mbps: f64, length: f32, width: f32, colour: Hsla| {
        let degrees = position_of_mbps(mbps) * FULL_TURN;
        let mut line = PathBuilder::stroke(px(width));
        line.move_to(dial.point(outer_r, degrees));
        line.line_to(dial.point(outer_r + length, degrees));
        if let Ok(path) = line.build() {
            window.paint_path(path, colour);
        }
        degrees
    };
    for mbps in MINOR_MBPS {
        tick(window, mbps, MINOR_TICK, MINOR_TICK_W, palette.ring);
    }
    let label = Type {
        size: font_size + LABEL_SIZE_DELTA,
        family: mono,
        weight: FontWeight::NORMAL,
        colour: palette.label,
        spacing: 0.0,
    };
    for mbps in MAJOR_MBPS {
        let degrees = tick(window, mbps, MAJOR_TICK, MAJOR_TICK_W, palette.major_tick);
        let at = dial.point(outer_r + LABEL_GAP, degrees);
        let text = if mbps >= MAX_MBPS {
            "1G".to_string()
        } else {
            format!("{}", mbps as u32)
        };
        paint_text(window, cx, &text, &label, f32::from(at.x), f32::from(at.y));
    }
}

/// One expanding ring per round trip: the wait made visible.
fn paint_pulse(window: &mut Window, dial: &Dial, palette: &Palette, progress: f32) {
    let outward = if progress < 0.5 {
        progress * 2.0
    } else {
        (1.0 - progress) * 2.0
    };
    let radius = PULSE_START_R + outward * (dial.download_r - PULSE_START_R);
    let colour = palette.latency.opacity((1.0 - outward) * PULSE_OPACITY);
    paint_circle(window, dial.centre, radius, PULSE_W, colour);
}

fn paint_comet(
    window: &mut Window,
    dial: &Dial,
    radius: f32,
    angle: f32,
    rate: f64,
    style: &Comet,
) {
    let position = position_of_bits(rate);
    let tail_degrees = MIN_TAIL_DEGREES + position * TAIL_DEGREES_SPAN;
    let leading = if style.trailing {
        angle - tail_degrees
    } else {
        angle
    };
    let width = MIN_TAIL_W + position * style.tail_w_span;
    let colour = style
        .colour
        .opacity(MIN_TAIL_OPACITY + position * TAIL_OPACITY_SPAN);
    let mut tail = PathBuilder::stroke(px(width));
    dial.arc(&mut tail, radius, leading, leading + tail_degrees);
    if let Ok(path) = tail.build() {
        window.paint_path(path, colour);
    }
    // Round caps, which a built path has no option for.
    for end in [leading, leading + tail_degrees] {
        paint_dot(window, dial.point(radius, end), width / 2.0, colour);
    }
    paint_dot(
        window,
        dial.point(radius, angle),
        style.head_r,
        style.colour,
    );
}

/// The latency number alone; the caption underneath carries the unit.
fn paint_centre(
    window: &mut Window,
    cx: &mut gpui::App,
    dial: &Dial,
    palette: &Palette,
    mono: &str,
    font_size: f32,
    value: &str,
) {
    let value_size = font_size * CENTRE_VALUE_SCALE;
    let caption_size = font_size + LABEL_SIZE_DELTA;
    let (cx_, cy) = (f32::from(dial.centre.x), f32::from(dial.centre.y));
    paint_text(
        window,
        cx,
        value,
        &Type {
            size: value_size,
            family: mono,
            weight: FontWeight::MEDIUM,
            colour: palette.latency,
            spacing: 0.0,
        },
        cx_,
        cy - caption_size * VALUE_ABOVE_CENTRE,
    );
    paint_text(
        window,
        cx,
        LATENCY_CAPTION,
        &Type {
            size: caption_size,
            family: mono,
            weight: FontWeight::NORMAL,
            colour: palette.label,
            spacing: CENTRE_CAPTION_SPACING,
        },
        cx_,
        cy + value_size * CAPTION_BELOW_CENTRE + caption_size * CAPTION_OWN_OFFSET,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn age_reads_like_the_webview() {
        let now = speed::now_unix();
        assert_eq!(age(now - 10), JUST_NOW);
        assert_eq!(age(now - 90), "1 minute ago");
        assert_eq!(age(now - 600), "10 minutes ago");
        assert_eq!(age(now - 7200), "2 hours ago");
        assert_eq!(age(now - 86400), "yesterday");
        assert_eq!(age(now - 3 * 86400), "3 days ago");
    }
}
