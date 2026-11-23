//! Falling snow as a pure function of time: a fixed, seeded field laid over
//! any area, so every shell draws the same snowfall without keeping state.
//! [`bokeh`] is the out-of-focus lights behind it.

pub mod bokeh;

use std::f64::consts::TAU;

/// One flake, in points from the area's top-left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Flake {
    pub x: f32,
    pub y: f32,
    /// Half the glyph's size for a crystal.
    pub radius: f32,
    pub alpha: f32,
    /// The index into [`CRYSTALS`] for a crystal, None for a dot.
    pub crystal: Option<usize>,
}

/// The glyphs crystals fall as, in text presentation so they take the snow's colour.
pub const CRYSTALS: [&str; 2] = ["❅", "❆"];
/// How much snow can fall, against the default.
pub const SNOW_AMOUNTS: [f64; 4] = [1.0, 1.25, 1.5, 2.0];
const AMOUNT: (f64, f64) = (SNOW_AMOUNTS[0], SNOW_AMOUNTS[SNOW_AMOUNTS.len() - 1]);
const MAX_DOTS: usize = 180;
const DOTS_PER_CRYSTAL: usize = 30;
const MOST_DOTS: usize = (MAX_DOTS as f64 * AMOUNT.1) as usize;
pub const MAX_FLAKES: usize = MOST_DOTS + MOST_DOTS / DOTS_PER_CRYSTAL;

/// Square points of area per flake at the default amount.
const AREA_PER_FLAKE: f64 = 5_500.0;
// Ranges, farthest flake to nearest.
const RADIUS: (f64, f64) = (0.8, 2.8);
const FALL_SPEED: (f64, f64) = (14.0, 48.0);
const ALPHA: (f64, f64) = (0.35, 0.92);
const SWAY: (f64, f64) = (4.0, 16.0);
const SWAY_PERIOD: (f64, f64) = (3.0, 7.5);
/// Breeze to the right in points per second, felt fully by the nearest flakes.
const WIND_SPEED: f64 = 6.0;
/// Most flakes sit far away: depth is a uniform draw raised to this power.
const DEPTH_SKEW: f64 = 2.0;
/// Crystals fall in the near layer, as glyphs this much larger than a dot.
const CRYSTAL_DEPTH: f64 = 0.7;
const CRYSTAL_SCALE: f64 = 4.5;
/// Room past every edge, so a flake wraps around out of sight.
const EDGE_MARGIN: f64 = RADIUS.1 + SWAY.1;
const FIELD_SEED: u64 = 0x5_EED5_2512;
const CRYSTAL_SEED: u64 = 0xC415_7A15;

/// Every flake of a `width` x `height` area at `seconds`, `amount` times the
/// default snow. Resizing keeps the flakes two sizes share.
pub fn frame(width: f64, height: f64, seconds: f64, amount: f64) -> impl Iterator<Item = Flake> {
    let amount = if amount.is_finite() {
        amount.clamp(AMOUNT.0, AMOUNT.1)
    } else {
        AMOUNT.0
    };
    let dots = if finite_size(width, height) {
        ((width * height / AREA_PER_FLAKE).min(MAX_DOTS as f64) * amount).round() as usize
    } else {
        0
    };
    let seconds = if seconds.is_finite() { seconds } else { 0.0 };
    let field = (0..dots).map(|index| (Rng::stream(FIELD_SEED, index), false));
    let crystals =
        (0..dots / DOTS_PER_CRYSTAL).map(|index| (Rng::stream(CRYSTAL_SEED, index), true));
    field
        .chain(crystals)
        .map(move |(rng, crystal)| place(rng, crystal, width, height, seconds))
}

fn place(mut rng: Rng, crystal: bool, width: f64, height: f64, seconds: f64) -> Flake {
    let depth = rng.unit().powf(DEPTH_SKEW);
    let depth = if crystal {
        lerp((CRYSTAL_DEPTH, 1.0), depth)
    } else {
        depth
    };
    let (start_x, start_y) = (rng.unit(), rng.unit());
    let sway = lerp(SWAY, rng.unit());
    let period = lerp(SWAY_PERIOD, rng.unit());
    let phase = rng.unit() * TAU;
    let glyph = rng.unit();

    let span_x = width + 2.0 * EDGE_MARGIN;
    let span_y = height + 2.0 * EDGE_MARGIN;
    let drift = WIND_SPEED * depth * seconds;
    let swing = sway * (TAU * seconds / period + phase).sin();
    let radius = lerp(RADIUS, depth);
    let crystal =
        crystal.then(|| ((glyph * CRYSTALS.len() as f64) as usize).min(CRYSTALS.len() - 1));
    Flake {
        x: ((start_x * span_x + drift).rem_euclid(span_x) - EDGE_MARGIN + swing) as f32,
        y: ((start_y * span_y + lerp(FALL_SPEED, depth) * seconds).rem_euclid(span_y) - EDGE_MARGIN)
            as f32,
        radius: (if crystal.is_some() {
            radius * CRYSTAL_SCALE
        } else {
            radius
        }) as f32,
        alpha: lerp(ALPHA, depth) as f32,
        crystal,
    }
}

pub(crate) fn lerp((from, to): (f64, f64), t: f64) -> f64 {
    from + (to - from) * t
}

pub(crate) fn finite_size(width: f64, height: f64) -> bool {
    width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0
}

/// SplitMix64, one stream per seed and index.
pub(crate) struct Rng(u64);

impl Rng {
    const GAMMA: u64 = 0x9E37_79B9_7F4A_7C15;
    const MIX: (u64, u64) = (0xBF58_476D_1CE4_E5B9, 0x94D0_49BB_1331_11EB);

    pub(crate) fn stream(seed: u64, index: usize) -> Self {
        Self(seed ^ (index as u64).wrapping_mul(Self::GAMMA))
    }

    /// Uniform in `[0, 1)`, from the top 53 bits.
    pub(crate) fn unit(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(Self::GAMMA);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(Self::MIX.0);
        z = (z ^ (z >> 27)).wrapping_mul(Self::MIX.1);
        ((z ^ (z >> 31)) >> (u64::BITS - f64::MANTISSA_DIGITS)) as f64
            / (1u64 << f64::MANTISSA_DIGITS) as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: f64 = 840.0;
    const H: f64 = 560.0;

    #[test]
    fn count_follows_area_and_amount_within_bounds() {
        let count = |w, h, amount| frame(w, h, 0.0, amount).count();
        let base = count(W, H, AMOUNT.0);
        assert!(base > 0 && count(W * 2.0, H, AMOUNT.0) > base);
        assert!(
            count(W, H, AMOUNT.1) > base && count(W, H, AMOUNT.1 * 10.0) == count(W, H, AMOUNT.1)
        );
        assert_eq!(count(W * 100.0, H * 100.0, AMOUNT.1), MAX_FLAKES);
        assert_eq!(count(0.0, H, AMOUNT.0) + count(f64::NAN, H, AMOUNT.0), 0);
    }

    #[test]
    fn flakes_stay_near_the_area_and_fall() {
        let margin = (EDGE_MARGIN + SWAY.1) as f32;
        for seconds in [0.0, 1_000.0, 8.0e8] {
            for flake in frame(W, H, seconds, AMOUNT.0) {
                assert!(flake.x > -margin && flake.x < W as f32 + margin);
                assert!(flake.y > -margin && flake.y < H as f32 + margin);
            }
        }
        let before = frame(W, H, 10.0, AMOUNT.0);
        let after = frame(W, H, 10.05, AMOUNT.0);
        assert!(
            before
                .zip(after)
                .filter(|(a, _)| a.y < H as f32)
                .all(|(a, b)| b.y > a.y)
        );
    }

    #[test]
    fn crystals_are_rare_extras_after_the_dots() {
        let flakes: Vec<Flake> = frame(W, H, 0.0, AMOUNT.0).collect();
        let crystals = flakes.iter().filter(|f| f.crystal.is_some()).count();
        assert!(crystals > 0 && crystals * DOTS_PER_CRYSTAL <= flakes.len());
        assert!(
            flakes[..flakes.len() - crystals]
                .iter()
                .all(|f| f.crystal.is_none())
        );
    }
}
