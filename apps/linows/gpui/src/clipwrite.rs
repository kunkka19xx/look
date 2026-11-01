//! Clipboard writes run on worker threads, so two issued in quick order can
//! land swapped: pick A, pick B, Ctrl+I, and the `[A]` write may land last,
//! right before the paste chord. Each write takes a ticket as it is issued on
//! the UI thread, and a worker whose ticket is older than the last one landed
//! skips: the newer content is what the user meant.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

/// A clipboard write, run off the UI thread.
pub type Write = Box<dyn FnOnce() -> Result<(), String> + Send>;

/// Wraps `work` so it only runs while no later write has landed.
pub fn ordered(work: Write) -> Write {
    static CLIPBOARD: Sequence = Sequence::new();
    CLIPBOARD.ordered(work)
}

struct Sequence {
    issued: AtomicU64,
    /// The ticket of the last write that ran. Held through the write, so two
    /// workers never write at once either.
    landed: Mutex<u64>,
}

impl Sequence {
    const fn new() -> Self {
        Self {
            issued: AtomicU64::new(0),
            landed: Mutex::new(0),
        }
    }

    fn ordered(&'static self, work: Write) -> Write {
        let ticket = self.issued.fetch_add(1, Ordering::Relaxed) + 1;
        Box::new(move || {
            let mut landed = self.landed.lock().unwrap_or_else(|e| e.into_inner());
            if *landed > ticket {
                return Ok(());
            }
            *landed = ticket;
            work()
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::{Sequence, Write};

    fn counting(hits: &Arc<AtomicUsize>) -> Write {
        let hits = Arc::clone(hits);
        Box::new(move || {
            hits.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
    }

    #[test]
    fn a_write_issued_earlier_skips_once_a_later_one_landed() {
        static SEQ: Sequence = Sequence::new();
        let hits = Arc::new(AtomicUsize::new(0));
        let first = SEQ.ordered(counting(&hits));
        let second = SEQ.ordered(counting(&hits));
        assert_eq!(second(), Ok(()));
        assert_eq!(first(), Ok(()));
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn writes_landing_in_order_all_run() {
        static SEQ: Sequence = Sequence::new();
        let hits = Arc::new(AtomicUsize::new(0));
        let first = SEQ.ordered(counting(&hits));
        let second = SEQ.ordered(counting(&hits));
        assert_eq!(first(), Ok(()));
        assert_eq!(second(), Ok(()));
        assert_eq!(hits.load(Ordering::SeqCst), 2);
    }
}
