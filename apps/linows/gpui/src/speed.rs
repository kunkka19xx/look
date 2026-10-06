//! The `/speed` state that outlives the panel and the window: the last
//! reading, the run in flight, and the dial's motion. The measurement has no
//! cancel, so a run started from the panel finishes on its own thread and the
//! next open rejoins it; a second one is never started beside it, since two
//! measurements compete for the bandwidth they are each trying to measure.

use std::sync::{Mutex, MutexGuard};
use std::thread;
use std::time::Instant;

use linows_backend::netspeed;
pub use linows_backend::netspeed::SpeedReading;

/// How recent a reading has to be for opening the panel to reuse it rather
/// than spend bandwidth on a fresh run. The panel calls anything inside this
/// window "just now", so the two read as one decision.
pub const FRESH_SECS: i64 = 60;

/// Both rings carry the same log scale: one full turn from 0 to 1 Gbps, so a
/// slow link and a fibre line are both somewhere readable on the dial.
pub const MAX_MBPS: f64 = 1000.0;
const BITS_PER_MEGABIT: f64 = 1_000_000.0;
pub const FULL_TURN: f32 = 360.0;
/// A comet's pace grows with its rate, so the dial reads fast before you
/// read a number.
const MIN_DEGREES_PER_SECOND: f32 = 10.0;
const DEGREES_PER_SECOND_SPAN: f32 = 420.0;
/// The ping ring travels out and back once per round trip, at a pace scaled
/// from the measured latency.
const PULSE_SECONDS_PER_MS: f32 = 1.0 / 220.0;
const MIN_PULSE_SECONDS: f32 = 0.35;
/// Rate at which a shown value closes the gap to a measured one, per second,
/// applied through exp so the ease is the same on any refresh rate.
const EASING: f32 = 6.0;
/// Ceiling on a frame's delta, so a stalled window doesn't fling the comets.
const MAX_FRAME_SECONDS: f32 = 0.05;

static STATE: Mutex<Speed> = Mutex::new(Speed::new());

pub fn lock() -> MutexGuard<'static, Speed> {
    STATE.lock().unwrap_or_else(|p| p.into_inner())
}

/// Where a rate sits on the ring, 0 at the top going clockwise, 1 at the end
/// of the scale.
pub fn position_of_mbps(mbps: f64) -> f32 {
    ((1.0 + mbps.max(0.0)).log10() / (1.0 + MAX_MBPS).log10()).min(1.0) as f32
}

pub fn position_of_bits(bits_per_second: f64) -> f32 {
    position_of_mbps(bits_per_second.max(0.0) / BITS_PER_MEGABIT)
}

fn degrees_per_second(bits_per_second: f64) -> f32 {
    MIN_DEGREES_PER_SECOND + position_of_bits(bits_per_second) * DEGREES_PER_SECOND_SPAN
}

/// One frame of the dial: where each comet is, how far the pulse has gone,
/// and the rates the tails are shaped for, easing toward the reading.
#[derive(Default)]
pub struct Motion {
    pub download_angle: f32,
    pub upload_angle: f32,
    pub pulse: f32,
    pub shown_download: f64,
    pub shown_upload: f64,
    last_frame: Option<Instant>,
}

impl Motion {
    const fn new() -> Self {
        Self {
            download_angle: 0.0,
            upload_angle: 0.0,
            pulse: 0.0,
            shown_download: 0.0,
            shown_upload: 0.0,
            last_frame: None,
        }
    }

    pub fn advance(&mut self, now: Instant, download: f64, upload: f64, latency_ms: Option<f64>) {
        let delta = self
            .last_frame
            .map_or(0.0, |last| now.duration_since(last).as_secs_f32())
            .min(MAX_FRAME_SECONDS);
        self.last_frame = Some(now);
        if delta <= 0.0 {
            return;
        }
        let ease = 1.0 - (-EASING * delta).exp();
        self.shown_download += (download - self.shown_download) * f64::from(ease);
        self.shown_upload += (upload - self.shown_upload) * f64::from(ease);
        self.download_angle =
            (self.download_angle + degrees_per_second(self.shown_download) * delta) % FULL_TURN;
        self.upload_angle = (self.upload_angle - degrees_per_second(self.shown_upload) * delta
            + FULL_TURN)
            % FULL_TURN;
        let period =
            (latency_ms.unwrap_or(0.0) as f32 * PULSE_SECONDS_PER_MS).max(MIN_PULSE_SECONDS);
        self.pulse = (self.pulse + delta / period) % 1.0;
    }

    /// A still dial never eases, so a reading has to be placed directly.
    pub fn snap(&mut self, download: f64, upload: f64) {
        self.shown_download = download;
        self.shown_upload = upload;
    }

    pub fn is_unstarted(&self) -> bool {
        self.last_frame.is_none()
    }

    /// The next frame starts a fresh delta instead of one spanning the gap.
    pub fn pause(&mut self) {
        self.last_frame = None;
    }
}

pub struct Speed {
    reading: Option<SpeedReading>,
    loaded: bool,
    running: bool,
    error: Option<String>,
    run_started: Option<Instant>,
    local_address: Option<String>,
    pub motion: Motion,
}

impl Speed {
    const fn new() -> Self {
        Self {
            reading: None,
            loaded: false,
            running: false,
            error: None,
            run_started: None,
            local_address: None,
            motion: Motion::new(),
        }
    }

    /// The panel opened: the LAN address is refreshed, and a run starts
    /// unless the reading is recent enough to reuse.
    pub fn enter(&mut self) {
        if !self.loaded {
            self.reading = netspeed::last_reading();
            self.loaded = true;
        }
        self.local_address = netspeed::local_ipv4();
        if !self.is_fresh() {
            self.start();
        }
    }

    fn is_fresh(&self) -> bool {
        self.reading
            .as_ref()
            .is_some_and(|reading| now_unix() - reading.measured_at_unix < FRESH_SECS)
    }

    /// One run at a time; the thread writes the outcome back here.
    pub fn start(&mut self) {
        if self.running {
            return;
        }
        self.running = true;
        self.error = None;
        self.run_started = Some(Instant::now());
        let spawned = thread::Builder::new().name("look-speed".into()).spawn(|| {
            let outcome = netspeed::speed_test();
            let mut speed = lock();
            speed.running = false;
            match outcome {
                Ok(reading) => {
                    netspeed::remember(&reading);
                    speed.reading = Some(reading);
                }
                Err(message) => speed.error = Some(message),
            }
        });
        if let Err(err) = spawned {
            self.running = false;
            self.error = Some(format!("Speed test failed: {err}"));
        }
    }

    pub fn reading(&self) -> Option<&SpeedReading> {
        self.reading.as_ref()
    }

    pub fn running(&self) -> bool {
        self.running
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn local_address(&self) -> Option<&str> {
        self.local_address.as_deref()
    }

    /// Seconds the run in flight has taken so far.
    pub fn elapsed_secs(&self) -> u64 {
        self.run_started.map_or(0, |at| at.elapsed().as_secs())
    }
}

pub fn now_unix() -> i64 {
    chrono::Local::now().timestamp()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn the_scale_runs_one_turn_to_a_gigabit() {
        assert_eq!(position_of_mbps(0.0), 0.0);
        assert!((position_of_mbps(MAX_MBPS) - 1.0).abs() < 1e-6);
        assert_eq!(position_of_mbps(5000.0), 1.0);
        assert!(position_of_mbps(10.0) < position_of_mbps(100.0));
    }

    #[test]
    fn motion_eases_toward_the_reading_and_wraps() {
        let mut motion = Motion::new();
        let start = Instant::now();
        motion.advance(start, 100.0, 50.0, Some(20.0));
        assert_eq!(
            motion.shown_download, 0.0,
            "the first frame only starts the clock"
        );
        for i in 1..=200 {
            motion.advance(
                start + Duration::from_millis(16 * i),
                100.0,
                50.0,
                Some(20.0),
            );
        }
        assert!((motion.shown_download - 100.0).abs() < 0.5);
        assert!((0.0..FULL_TURN).contains(&motion.download_angle));
        assert!((0.0..FULL_TURN).contains(&motion.upload_angle));
        assert!((0.0..1.0).contains(&motion.pulse));
    }

    #[test]
    fn a_stalled_frame_is_capped() {
        let mut motion = Motion::new();
        let start = Instant::now();
        motion.advance(start, 0.0, 0.0, None);
        motion.advance(start + Duration::from_secs(60), 0.0, 0.0, None);
        assert!(motion.download_angle <= MIN_DEGREES_PER_SECOND * MAX_FRAME_SECONDS + 1e-4);
    }
}
