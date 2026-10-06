//! Entrance motion, same numbers as src/css/motion.css and superactions.css
//! minus the scales: a CSS transform scales a finished raster, while gpui
//! 0.2.2 can only resize the box, which relayouts the text inside it every
//! frame. So here things fade and rise, by whole pixels, and without the CSS
//! spring: its overshoot in whole pixels is a one pixel bounce at the end.
//! gpui's with_animation hands out a linear 0..1 over the whole duration, so a
//! stagger is a longer animation whose first slice is held at zero.

use std::time::Duration;

pub const ARRIVE_MS: u64 = 340;
pub const SPAWN_MS: u64 = 460;
pub const SPAWN_RISE: f32 = 8.0;
pub const TILE_MS: u64 = 460;
pub const TILE_STAGGER_MS: u64 = 35;
pub const TILE_MAX_STAGGER_MS: u64 = 340;
pub const TILE_RISE: f32 = 10.0;

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

pub fn tile_delay(index: usize) -> Duration {
    Duration::from_millis((index as u64 * TILE_STAGGER_MS).min(TILE_MAX_STAGGER_MS))
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
}
