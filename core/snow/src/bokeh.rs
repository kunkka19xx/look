//! Out-of-focus string lights as a pure function of time: balls of light
//! that drift upwards and pulse, each a warm halo around a hot core so a
//! painted disc reads as emitted light rather than a stain.

use std::f64::consts::TAU;

use crate::{Rng, finite_size, lerp};

/// One soft light in points from the area's top-left: `alpha` at the
/// centre, fading to nothing at `radius`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Glow {
    pub center: [f32; 2],
    pub radius: f32,
    /// Straight sRGB, each 0 to 1.
    pub colour: [f32; 3],
    pub alpha: f32,
}

const MAX_LIGHTS: usize = 40;
/// Each light is two glows, its halo then its core.
pub const MAX_GLOWS: usize = 2 * MAX_LIGHTS;
const AREA_PER_LIGHT: f64 = 16_000.0;
const RADIUS: (f64, f64) = (12.0, 40.0);
/// Small lights outnumber big ones: size is a uniform draw raised to this.
const SIZE_SKEW: f64 = 1.5;
const ALPHA: (f64, f64) = (0.35, 0.70);
const PULSE: (f64, f64) = (0.6, 1.0);
const PULSE_PERIOD: (f64, f64) = (4.0, 9.0);
/// Points per second upwards.
const RISE: (f64, f64) = (2.0, 6.0);
const SWAY: (f64, f64) = (6.0, 14.0);
const SWAY_PERIOD: (f64, f64) = (8.0, 14.0);
/// The core's share of the halo's radius, how much brighter, and its colour.
const CORE_SHARE: f64 = 0.45;
const CORE_BOOST: f64 = 1.25;
const CORE_COLOUR: [f64; 3] = [1.0, 0.96, 0.86];
const EDGE_MARGIN: f64 = RADIUS.1 + SWAY.1;
const FIELD_SEED: u64 = 0xB0_CE4;
/// Halo colours by where a uniform draw lands: mostly gold, then warm
/// white, amber and the odd red.
const PALETTE: [(f64, [f64; 3]); 4] = [
    (0.50, [1.0, 0.80, 0.38]),
    (0.75, [1.0, 0.92, 0.72]),
    (0.93, [1.0, 0.66, 0.30]),
    (1.0, [0.95, 0.26, 0.26]),
];

/// Every light of a `width` x `height` area at `seconds`, halo then core.
pub fn bokeh(width: f64, height: f64, seconds: f64) -> impl Iterator<Item = Glow> {
    let lights = if finite_size(width, height) {
        ((width * height / AREA_PER_LIGHT).round() as usize).min(MAX_LIGHTS)
    } else {
        0
    };
    let seconds = if seconds.is_finite() { seconds } else { 0.0 };
    (0..lights).flat_map(move |index| light(index, width, height, seconds))
}

fn light(index: usize, width: f64, height: f64, seconds: f64) -> [Glow; 2] {
    let mut rng = Rng::stream(FIELD_SEED, index);
    let radius = lerp(RADIUS, rng.unit().powf(SIZE_SKEW));
    let (start_x, start_y) = (rng.unit(), rng.unit());
    let rise = lerp(RISE, rng.unit());
    let sway = lerp(SWAY, rng.unit());
    let sway_period = lerp(SWAY_PERIOD, rng.unit());
    let pulse_period = lerp(PULSE_PERIOD, rng.unit());
    let phase = rng.unit() * TAU;
    let brightness = lerp(ALPHA, rng.unit());
    let pick = rng.unit();
    let colour = PALETTE
        .iter()
        .find(|(upto, _)| pick < *upto)
        .map_or(PALETTE[0].1, |(_, colour)| *colour);

    let span_y = height + 2.0 * EDGE_MARGIN;
    let y = (start_y * span_y - rise * seconds).rem_euclid(span_y) - EDGE_MARGIN;
    let x = start_x * width + sway * (TAU * seconds / sway_period + phase).sin();
    let wave = ((TAU * seconds / pulse_period + phase).sin() + 1.0) / 2.0;
    let alpha = brightness * lerp(PULSE, wave);
    let glow = |radius: f64, colour: [f64; 3], alpha: f64| Glow {
        center: [x as f32, y as f32],
        radius: radius as f32,
        colour: colour.map(|c| c as f32),
        alpha: alpha.min(1.0) as f32,
    };
    [
        glow(radius, colour, alpha),
        glow(radius * CORE_SHARE, CORE_COLOUR, alpha * CORE_BOOST),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lights_are_halos_with_brighter_cores_within_the_cap() {
        let glows: Vec<Glow> = bokeh(840.0, 560.0, 2.0).collect();
        assert!(!glows.is_empty() && glows.len().is_multiple_of(2));
        for pair in glows.chunks(2) {
            assert!(pair[1].radius < pair[0].radius && pair[1].alpha >= pair[0].alpha);
        }
        assert_eq!(bokeh(1e5, 1e5, 0.0).count(), MAX_GLOWS);
        assert_eq!(bokeh(0.0, 560.0, 0.0).count(), 0);
    }
}
