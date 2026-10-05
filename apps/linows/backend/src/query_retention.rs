//! Whether a summon starts from a blank query. The hide stamps the clock, the
//! show reads it against `query_retention_seconds`; every shell's hide and show
//! pass through here so the rule is spelled once.

use std::sync::Mutex;
use std::time::{Duration, SystemTime};

/// Wall clock: `Instant` stops during suspend, so a launcher hidden overnight
/// would report only the minutes the machine was awake.
static LAST_HIDDEN_AT: Mutex<Option<SystemTime>> = Mutex::new(None);

pub fn mark_hidden_now() {
    let mut hidden_at = LAST_HIDDEN_AT.lock().unwrap_or_else(|p| p.into_inner());
    // A repeat dismissal must not restart the clock. A show consumes the stamp,
    // so `None` means on screen - `is_visible` would not, under layer shell.
    hidden_at.get_or_insert_with(SystemTime::now);
}

pub fn query_clear_decision_after_show(show_succeeded: bool) -> Option<bool> {
    if !show_succeeded {
        return None;
    }
    let hidden_at = LAST_HIDDEN_AT
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .take();
    let Some(hidden_at) = hidden_at else {
        return Some(false);
    };
    let timeout_secs = crate::config::query_retention_seconds();
    // A backwards clock reads as no time passed, and so keeps the query.
    Some(query_retention_expired(
        hidden_at.elapsed().unwrap_or_default(),
        timeout_secs,
    ))
}

fn query_retention_expired(hidden_for: Duration, timeout_secs: i64) -> bool {
    timeout_secs >= 0 && hidden_for >= Duration::from_secs(timeout_secs as u64)
}

#[cfg(test)]
mod tests {
    use super::{query_clear_decision_after_show, query_retention_expired};
    use std::time::{Duration, SystemTime};

    #[test]
    fn failed_show_reaches_no_decision() {
        assert_eq!(query_clear_decision_after_show(false), None);
    }

    #[test]
    fn show_with_no_recorded_hide_keeps_the_query() {
        assert_eq!(query_clear_decision_after_show(true), Some(false));
    }

    #[test]
    fn a_repeat_hide_keeps_the_first_timestamp() {
        let mut hidden_at = Some(SystemTime::UNIX_EPOCH);
        hidden_at.get_or_insert_with(SystemTime::now);
        assert_eq!(hidden_at, Some(SystemTime::UNIX_EPOCH));
    }

    #[test]
    fn query_retention_preserves_query_before_boundary() {
        assert!(!query_retention_expired(Duration::from_secs(3), 5));
        assert!(!query_retention_expired(Duration::from_millis(4_999), 5));
    }

    #[test]
    fn query_retention_clears_at_and_after_boundary() {
        assert!(query_retention_expired(Duration::from_secs(5), 5));
        assert!(query_retention_expired(Duration::from_secs(8), 5));
    }

    #[test]
    fn query_retention_negative_one_never_clears() {
        assert!(!query_retention_expired(Duration::from_secs(5), -1));
        assert!(!query_retention_expired(Duration::from_secs(60), -1));
    }
}
