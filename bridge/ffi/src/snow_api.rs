use std::mem::MaybeUninit;

use look_snow::bokeh::bokeh;
use look_snow::frame;

/// Per flake: x, y, radius, alpha, and the crystal glyph's index or -1.
const FLOATS_PER_FLAKE: usize = 5;
const NO_CRYSTAL: f32 = -1.0;
/// Per glow: x, y, radius, r, g, b, alpha.
const FLOATS_PER_GLOW: usize = 7;

/// `out` as whole records of `N` floats; none for a null pointer.
fn records<'a, const N: usize>(out: *mut f32, capacity: usize) -> &'a mut [[MaybeUninit<f32>; N]] {
    if out.is_null() {
        return &mut [];
    }
    // SAFETY: the caller owns `capacity` writable floats at `out`.
    let out = unsafe { std::slice::from_raw_parts_mut(out.cast::<MaybeUninit<f32>>(), capacity) };
    out.as_chunks_mut::<N>().0
}

pub(crate) fn look_snow_frame_impl(
    width: f64,
    height: f64,
    seconds: f64,
    amount: f64,
    out: *mut f32,
    capacity: usize,
) -> usize {
    records::<FLOATS_PER_FLAKE>(out, capacity)
        .iter_mut()
        .zip(frame(width, height, seconds, amount))
        .map(|(slot, flake)| {
            let crystal = flake.crystal.map_or(NO_CRYSTAL, |glyph| glyph as f32);
            *slot = [flake.x, flake.y, flake.radius, flake.alpha, crystal].map(MaybeUninit::new);
        })
        .count()
}

pub(crate) fn look_bokeh_frame_impl(
    width: f64,
    height: f64,
    seconds: f64,
    out: *mut f32,
    capacity: usize,
) -> usize {
    records::<FLOATS_PER_GLOW>(out, capacity)
        .iter_mut()
        .zip(bokeh(width, height, seconds))
        .map(|(slot, glow)| {
            let ([x, y], [r, g, b]) = (glow.center, glow.colour);
            *slot = [x, y, glow.radius, r, g, b, glow.alpha].map(MaybeUninit::new);
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_whole_records_up_to_capacity() {
        let mut buffer = vec![0.0f32; 3 * FLOATS_PER_FLAKE + 1];
        let written =
            look_snow_frame_impl(840.0, 560.0, 1.0, 1.0, buffer.as_mut_ptr(), buffer.len());
        let first = frame(840.0, 560.0, 1.0, 1.0).next().expect("a flake");
        assert_eq!(written, 3);
        assert_eq!(&buffer[..4], &[first.x, first.y, first.radius, first.alpha]);
        assert_eq!(
            look_bokeh_frame_impl(840.0, 560.0, 1.0, std::ptr::null_mut(), 8),
            0
        );
    }
}
