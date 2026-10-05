//! Entrance motion, same numbers as src/css/motion.css and superactions.css.
//! gpui's with_animation hands out a linear 0..1 over the whole duration, so a
//! stagger is a longer animation whose first slice is held at zero.

use std::time::Duration;

pub const ARRIVE_MS: u64 = 340;
pub const ARRIVE_SCALE: f32 = 0.965;
pub const SPAWN_MS: u64 = 460;
pub const SPAWN_RISE: f32 = 8.0;
pub const TILE_MS: u64 = 460;
pub const TILE_STAGGER_MS: u64 = 35;
pub const TILE_MAX_STAGGER_MS: u64 = 340;
pub const TILE_RISE: f32 = 10.0;
pub const TILE_SCALE: f32 = 0.985;

/// cubic-bezier(0.22, 1, 0.36, 1), the settle curve.
pub fn curve(t: f32) -> f32 {
    cubic_bezier(0.22, 1.0, 0.36, 1.0, t)
}

/// cubic-bezier(0.34, 1.4, 0.64, 1), overshoots past 1 like a spring.
pub fn spring(t: f32) -> f32 {
    cubic_bezier(0.34, 1.4, 0.64, 1.0, t)
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
    fn curves_hit_their_endpoints() {
        for f in [curve as fn(f32) -> f32, spring] {
            assert_eq!(f(0.0), 0.0);
            assert_eq!(f(1.0), 1.0);
            assert!(f(0.5) > 0.5, "both curves ease out");
        }
        assert!(curve(0.5) < 1.0, "the settle curve never overshoots");
        assert!(spring(0.6) > 1.0, "the spring overshoots");
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
