//! Starting and stopping the processes behind `connect` blocks.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::state::{home_dir, refresh_sources_now};

/// Starts every declared connector, and restarts any that died. Called at
/// launch and on reload, since a reload is how a newly installed manifest is
/// noticed.
pub(crate) fn start() {
    let Some(home) = home_dir() else {
        return;
    };
    look_engine::connectors::start(&home, on_rows());
}

/// Respawns any connector that died. Called when the launcher opens, which is
/// what "restarted on next use" means in practice.
pub(crate) fn restart_dead() {
    look_engine::connectors::restart_dead(on_rows());
}

pub(crate) fn stop() {
    look_engine::connectors::stop();
}

/// How long a burst is allowed to settle before its rows are read, and the
/// floor between two reads however often rows arrive.
const SETTLE: Duration = Duration::from_millis(300);
const FLOOR: Duration = Duration::from_secs(2);
/// How soon to try again when another refresh holds the slot.
const BUSY_RETRY: Duration = Duration::from_millis(250);

/// Only the declared sources are re-read. Marking the whole index dirty would
/// make the next launcher open walk the home directory again.
fn on_rows() -> look_engine::connectors::OnRows {
    Arc::new(|_block: &str| schedule_read())
}

/// Reads the sources soon, not now, and not more than once per `FLOOR`.
///
/// A read is not free: it re-parses every declaration and re-walks every `dir`
/// block, which is the right price for a reload and the wrong one per frame. A
/// connector that pushes a hundred rows in a burst costs one read, and one that
/// pushes constantly costs one read every `FLOOR` instead of one per frame.
///
/// The cost is bounded rather than removed, so a connector should still push
/// when its data changes rather than on a timer.
fn schedule_read() {
    static QUEUED: AtomicBool = AtomicBool::new(false);
    static ARRIVED: AtomicBool = AtomicBool::new(false);
    static NEXT_ALLOWED: Mutex<Option<Instant>> = Mutex::new(None);

    // Before the queue check, so rows landing mid-read still count.
    ARRIVED.store(true, Ordering::Release);
    if QUEUED.swap(true, Ordering::AcqRel) {
        return;
    }

    thread::spawn(move || {
        let mut wait = until_allowed(&NEXT_ALLOWED);
        let mut busy = 0_u32;
        loop {
            thread::sleep(wait);

            // Before the walk, never after: a push during it sets this again.
            ARRIVED.store(false, Ordering::Release);

            // A busy slot or a failed walk: waited out, not given up on, since
            // nothing else is coming to ask and a deadline would lose the rows.
            if !refresh_sources_now() {
                busy += 1;
                wait = if busy > 8 { FLOOR } else { BUSY_RETRY };
                ARRIVED.store(true, Ordering::Release);
                continue;
            }
            busy = 0;

            {
                let mut next = NEXT_ALLOWED.lock().unwrap_or_else(|err| err.into_inner());
                *next = Some(Instant::now() + FLOOR);
            }
            // Released before the check
            QUEUED.store(false, Ordering::Release);
            if !ARRIVED.load(Ordering::Acquire) {
                return;
            }
            if QUEUED.swap(true, Ordering::AcqRel) {
                return;
            }
            wait = until_allowed(&NEXT_ALLOWED);
        }
    });
}

/// How long until another read is allowed, never less than the settle.
fn until_allowed(next: &Mutex<Option<Instant>>) -> Duration {
    let next = next.lock().unwrap_or_else(|err| err.into_inner());
    next.map(|at| at.saturating_duration_since(Instant::now()))
        .unwrap_or_default()
        .max(SETTLE)
}
