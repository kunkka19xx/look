//! Motion, same numbers as src/css/motion.css and the component sheets
//! minus the scales: a CSS transform scales a finished raster, while gpui
//! 0.2.2 can only resize the box, which relayouts the text inside it every
//! frame. So here things fade and rise, by whole pixels, and without the
//! CSS spring: its overshoot in whole pixels is a one pixel bounce at the
//! end. An `svg` may still scale or turn through `with_transformation`.
//!
//! Three gpui mechanisms carry it: `with_animation` keyed by an element id
//! for entrances, which restart when the id first appears; `Transition`
//! for a value that glides to each new goal, such as the selection pill
//! and the caret; and `request_animation_frame` from a custom element for
//! the loops that keep going with motion off.
//!
//! gpui's with_animation hands out a linear 0..1 over the whole duration,
//! so a stagger is a longer animation whose first slice is held at zero.
//! Both switches that turn motion off (`animations_enabled`, the desktop's
//! reduce-motion preference) go through `App::set_reduce_motion`, which
//! every `with_animation` already honours by landing on its last frame.

use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use gpui::App;

/// `LOOK_MOTION_SCALE=6` stretches every duration, so a frame strip taken
/// with a slow grabber still catches an animation mid-flight. Dev only.
static SCALE: LazyLock<f32> = LazyLock::new(|| {
    std::env::var("LOOK_MOTION_SCALE")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|s: &f32| *s > 0.0)
        .unwrap_or(1.0)
});

/// `ms` as a duration, under the dev scale.
pub fn dur(ms: u64) -> Duration {
    Duration::from_secs_f32(ms as f32 / 1000.0 * *SCALE)
}

/// `ms` in seconds, under the dev scale, for the clock-driven effects.
pub fn secs(ms: u64) -> f32 {
    dur(ms).as_secs_f32()
}

pub const ARRIVE_MS: u64 = 340;
pub const SPAWN_MS: u64 = 460;
pub const SPAWN_RISE: f32 = 8.0;
pub const TILE_MS: u64 = 460;
pub const TILE_STAGGER_MS: u64 = 35;
pub const TILE_MAX_STAGGER_MS: u64 = 340;
pub const TILE_RISE: f32 = 10.0;

/// Placeholder from the right, strip from the left, both landing just
/// behind the bar they sit in.
pub const SLIDE_MS: u64 = 800;
pub const SLIDE_DELAY_MS: u64 = 90;
pub const SLIDE_STAGGER_MS: u64 = 40;
pub const SLIDE_MAX_STAGGER_MS: u64 = 280;
pub const PLACEHOLDER_SHIFT: f32 = 20.0;
pub const STRIP_SHIFT: f32 = -18.0;

/// The selection pill's glide between rows, and the icon's zoom when it
/// lands: at 22 px a few percent is a pixel, so the glyph zooms far more
/// than a pill ever would. macOS eases in over 110 ms, then a spring with
/// a 0.3 s response and 0.6 damping takes it back, undershooting a touch
/// and settling over the rest; the whole run is `ZOOM_MS`.
pub const GLIDE_MS: u64 = 300;
pub const ZOOM_MS: u64 = 710;
pub const ZOOM_IN_MS: u64 = 110;
pub const ICON_ZOOM: f32 = 1.24;
const ZOOM_OUT_RESPONSE: f32 = 0.3;
const ZOOM_OUT_DAMPING: f32 = 0.6;
/// The selected row's text sits this much to the right, the macOS
/// `titleShift`, riding the same glide.
pub const TITLE_SHIFT: f32 = 4.0;

/// Glyph bounce, the stand-in for symbolEffect(.bounce).
pub const BOUNCE_MS: u64 = 460;
pub const BOUNCE_SCALE: f32 = 1.18;
pub const BOUNCE_PEAK: f32 = 0.38;

/// A pressed tile dips, 45% of the way through.
pub const PRESS_MS: u64 = 180;
pub const PRESS_PEAK: f32 = 0.45;
pub const SLOT_FADE_MS: u64 = 240;
pub const BAR_GLIDE_MS: u64 = 900;

pub const RESULTS_IN_MS: u64 = 140;
pub const RESULTS_IN_RISE: f32 = 6.0;
pub const BANNER_MS: u64 = 200;
pub const BANNER_RISE: f32 = -4.0;
pub const CONFIRM_MS: u64 = 180;
pub const CONFIRM_RISE: f32 = 8.0;
pub const CARET_GLIDE_MS: u64 = 105;
/// The caret's blink period, and how long after a move it stays solid.
pub const CARET_BLINK_MS: u64 = 1050;
pub const CARET_SOLID_MS: u64 = 500;
pub const SPIN_MS: u64 = 700;
pub const CHEVRON_MS: u64 = 200;
pub const REJECT_MS: u64 = 400;
pub const REJECT_PEAK: f32 = 0.4;
pub const REJECT_WASH: f32 = 0.28;

/// The desktop's own preference, read once at start.
static DESKTOP_REDUCES: AtomicBool = AtomicBool::new(false);

pub fn set_desktop_reduces(reduces: bool) {
    DESKTOP_REDUCES.store(reduces, Ordering::Relaxed);
}

/// Hand gpui the one switch every animation reads: off when the config
/// says so or the desktop does. Cheap to call every frame; gpui ignores
/// a value it already has.
pub fn sync(animations: bool, cx: &mut App) {
    cx.set_reduce_motion(!animations || DESKTOP_REDUCES.load(Ordering::Relaxed));
}

/// A transition's length: a hair above zero when motion is off, so the
/// value lands at once and nothing divides by nothing.
pub fn transition(ms: u64, cx: &App) -> Duration {
    if cx.reduce_motion() {
        Duration::from_millis(1)
    } else {
        dur(ms)
    }
}

/// cubic-bezier(0.22, 1, 0.36, 1), the settle curve.
pub fn curve(t: f32) -> f32 {
    cubic_bezier(0.22, 1.0, 0.36, 1.0, t)
}

/// Where a rising element sits at progress `t`: `rise` below `rest` at the
/// start, at `rest` when done, on a whole pixel throughout so the text in it
/// never lands between two.
pub fn rise(rest: f32, rise: f32, t: f32) -> f32 {
    (rest + rise * (1.0 - t)).round()
}

/// The icon's scale at `t` of its zoom: an ease-out up to `ICON_ZOOM` for
/// the first `ZOOM_IN_MS`, then a damped spring back to 1, the way the
/// macOS row zooms in and lets go.
pub fn zoom(t: f32) -> f32 {
    let total = ZOOM_MS as f32 / 1000.0;
    let rise = ZOOM_IN_MS as f32 / 1000.0;
    let elapsed = (t.clamp(0.0, 1.0) * total).min(total);
    let gain = ICON_ZOOM - 1.0;
    if elapsed < rise {
        let u = 1.0 - elapsed / rise;
        return 1.0 + gain * (1.0 - u * u * u);
    }
    1.0 + gain * spring(elapsed - rise, ZOOM_OUT_RESPONSE, ZOOM_OUT_DAMPING)
}

/// A SwiftUI spring's distance from rest, from 1 at `t` = 0: `response`
/// is the undamped period, `damping` the fraction of critical.
fn spring(t: f32, response: f32, damping: f32) -> f32 {
    let w0 = std::f32::consts::TAU / response;
    let wd = w0 * (1.0 - damping * damping).sqrt();
    (-damping * w0 * t).exp() * ((wd * t).cos() + (damping * w0 / wd) * (wd * t).sin())
}

/// A hump from 0 up to 1 at `peak` and back to 0 at the end, the shape of
/// every CSS keyframe that scales up and settles: the way up is the
/// settle curve, the way down the same curve reversed.
pub fn hump(t: f32, peak: f32) -> f32 {
    if t <= 0.0 || t >= 1.0 {
        0.0
    } else if t < peak {
        curve(t / peak)
    } else {
        1.0 - curve((t - peak) / (1.0 - peak))
    }
}

/// The caret's blink at `t` of its period: solid for a third, a soft dip
/// to nothing at two thirds, back to solid.
pub fn blink(t: f32) -> f32 {
    let t = t.rem_euclid(1.0);
    if t < 0.35 {
        1.0
    } else if t < 0.67 {
        1.0 - ease_in_out((t - 0.35) / 0.32)
    } else {
        ease_in_out((t - 0.67) / 0.33)
    }
}

fn ease_in_out(t: f32) -> f32 {
    if t < 0.5 {
        2.0 * t * t
    } else {
        let x = -2.0 * t + 2.0;
        1.0 - x * x / 2.0
    }
}

pub fn tile_delay(index: usize) -> Duration {
    dur((index as u64 * TILE_STAGGER_MS).min(TILE_MAX_STAGGER_MS))
}

/// The strip tile at `index` waits the slide delay plus its stagger.
pub fn slide_delay(index: usize) -> Duration {
    Duration::from_millis(
        SLIDE_DELAY_MS + (index as u64 * SLIDE_STAGGER_MS).min(SLIDE_MAX_STAGGER_MS),
    )
}

/// Progress of a `duration` slice that starts `delay` into an animation of
/// `total` length, given the whole animation's linear progress.
pub fn staggered(progress: f32, total: Duration, delay: Duration, duration: Duration) -> f32 {
    let elapsed = progress * total.as_secs_f32() - delay.as_secs_f32();
    (elapsed / duration.as_secs_f32()).clamp(0.0, 1.0)
}

fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, x: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let bezier = |p1: f32, p2: f32, t: f32| {
        let u = 1.0 - t;
        3.0 * u * u * t * p1 + 3.0 * u * t * t * p2 + t * t * t
    };
    // Newton on x(t) = x; five steps are exact to well under a pixel.
    let mut t = x;
    for _ in 0..5 {
        let u = 1.0 - t;
        let dx = 3.0 * u * u * x1 + 6.0 * u * t * (x2 - x1) + 3.0 * t * t * (1.0 - x2);
        if dx.abs() < 1e-6 {
            break;
        }
        t -= (bezier(x1, x2, t) - x) / dx;
        t = t.clamp(0.0, 1.0);
    }
    bezier(y1, y2, t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_curve_eases_out_and_never_overshoots() {
        assert_eq!(curve(0.0), 0.0);
        assert_eq!(curve(1.0), 1.0);
        assert!(curve(0.5) > 0.5);
        for step in 0..=100 {
            let t = curve(step as f32 / 100.0);
            assert!((0.0..=1.0).contains(&t), "overshoot at step {step}: {t}");
        }
    }

    #[test]
    fn rise_lands_on_whole_pixels_and_at_rest() {
        assert_eq!(rise(100.0, 10.0, 0.0), 110.0);
        assert_eq!(rise(100.0, 10.0, 1.0), 100.0);
        let mid = rise(100.5, 10.0, 0.37);
        assert_eq!(mid, mid.round());
    }

    #[test]
    fn stagger_holds_then_runs() {
        let total = Duration::from_millis(800);
        let delay = Duration::from_millis(300);
        let dur = Duration::from_millis(500);
        assert_eq!(staggered(0.0, total, delay, dur), 0.0);
        assert_eq!(staggered(0.375, total, delay, dur), 0.0);
        assert!((staggered(0.6875, total, delay, dur) - 0.5).abs() < 1e-4);
        assert_eq!(staggered(1.0, total, delay, dur), 1.0);
    }

    #[test]
    fn the_zoom_peaks_after_the_rise_dips_under_rest_and_settles() {
        let rise = ZOOM_IN_MS as f32 / ZOOM_MS as f32;
        assert_eq!(zoom(0.0), 1.0);
        assert!((zoom(rise) - ICON_ZOOM).abs() < 1e-3);
        let under = (0..100)
            .map(|i| zoom(rise + (1.0 - rise) * i as f32 / 100.0))
            .fold(1.0_f32, f32::min);
        assert!(under < 1.0 && under > 0.95, "undershoot {under}");
        assert!((zoom(1.0) - 1.0).abs() < 0.01);
    }

    #[test]
    fn the_hump_peaks_where_asked_and_rests_at_both_ends() {
        assert_eq!(hump(0.0, 0.26), 0.0);
        assert!((hump(0.26, 0.26) - 1.0).abs() < 1e-5);
        assert_eq!(hump(1.0, 0.26), 0.0);
        assert!(hump(0.1, 0.26) > 0.0 && hump(0.1, 0.26) < 1.0);
    }

    #[test]
    fn the_blink_is_solid_then_dips_then_returns() {
        assert_eq!(blink(0.0), 1.0);
        assert_eq!(blink(0.3), 1.0);
        assert!(blink(0.67) < 0.01);
        assert!((blink(1.0) - 1.0).abs() < 1e-5);
        assert!(blink(0.5) > 0.0 && blink(0.5) < 1.0);
    }

    #[test]
    fn strip_tiles_wait_their_turn_up_to_the_cap() {
        assert_eq!(slide_delay(0), Duration::from_millis(90));
        assert_eq!(slide_delay(2), Duration::from_millis(170));
        assert_eq!(slide_delay(20), Duration::from_millis(370));
    }
}
