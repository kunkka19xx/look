//! Starting and stopping the processes behind `connect` blocks.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::state::{home_dir, request_sources_refresh};

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
    static NEXT_ALLOWED: Mutex<Option<Instant>> = Mutex::new(None);

    // One queued read at a time: the frames arriving while it waits are
    // already in the cache, so they are what it will read.
    if QUEUED.swap(true, Ordering::AcqRel) {
        return;
    }

    let wait = {
        let next = NEXT_ALLOWED.lock().unwrap_or_else(|err| err.into_inner());
        next.map(|at| at.saturating_duration_since(Instant::now()))
            .unwrap_or_default()
            .max(SETTLE)
    };

    thread::spawn(move || {
        let give_up = Instant::now() + Duration::from_secs(60);
        let mut wait = wait;
        loop {
            thread::sleep(wait);
            {
                let mut next = NEXT_ALLOWED.lock().unwrap_or_else(|err| err.into_inner());
                *next = Some(Instant::now() + FLOOR);
            }
            // A refresh already running is not this one: it may have read the
            // cache before these rows reached it, and with lazy indexing no
            // other refresh is coming. Dropping the push would leave the rows
            // unseen until something else happened to ask.
            if request_sources_refresh() || Instant::now() >= give_up {
                QUEUED.store(false, Ordering::Release);
                return;
            }
            wait = BUSY_RETRY;
        }
    });
}
