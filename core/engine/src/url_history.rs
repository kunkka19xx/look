//! Frecency ranking for launcher-opened URL history (url-history spec).
//!
//! `look_storage::recent_urls` returns raw rows; this module scores them with
//! the exact `rank_score` the engine applies to matched candidates, so a
//! remembered URL rises among local results by the same math as apps/files,
//! not an arbitrary hit-count threshold. Lives in the engine (not per shell)
//! so macOS (via bridge/ffi) and linows rank identically.

use crate::config::SCORE_TITLE_CONTAINS;
use look_indexing::{Candidate, CandidateKind};
use look_ranking::rank_score;
use look_storage::UrlHistoryEntry;
use serde::Serialize;

/// Wire shape of a scored URL-history row. `look_storage::UrlHistoryEntry` is
/// not `Serialize` (that crate has no serde dep), so it is mapped here; both
/// shells serialise this struct as-is.
#[derive(Debug, Serialize)]
pub struct ScoredUrlEntry {
    pub url: String,
    pub title: Option<String>,
    pub hit_count: u64,
    pub last_used_at_unix_s: i64,
    pub score: i64,
}

/// Ranks a remembered URL for `query`: a title-contains base plus log-scaled
/// frequency (`hit_count`) and a recency bonus. `recent_urls` only returns
/// substring matches, so the base is always the title-contains score.
fn score_entry(entry: &UrlHistoryEntry, query: &str) -> i64 {
    let mut candidate = Candidate::new(&entry.url, CandidateKind::File, &entry.url, &entry.url);
    candidate.use_count = entry.hit_count;
    candidate.last_used_at_unix_s = Some(entry.last_used_at_unix_s);
    let title_lower = entry.url.to_lowercase();
    rank_score(SCORE_TITLE_CONTAINS, query, &candidate, &title_lower)
}

/// Scores each entry for `query` (normalised here so callers cannot diverge)
/// and returns them in frecency order: highest score first, most-recent
/// breaking ties.
pub fn ranked_url_history(entries: Vec<UrlHistoryEntry>, query: &str) -> Vec<ScoredUrlEntry> {
    let query_norm = query.trim().to_lowercase();
    let mut scored: Vec<ScoredUrlEntry> = entries
        .into_iter()
        .map(|e| {
            let score = score_entry(&e, &query_norm);
            ScoredUrlEntry {
                url: e.url,
                title: e.title,
                hit_count: e.hit_count,
                last_used_at_unix_s: e.last_used_at_unix_s,
                score,
            }
        })
        .collect();
    scored.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then(b.last_used_at_unix_s.cmp(&a.last_used_at_unix_s))
    });
    scored
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(url: &str, hit_count: u64, last_used_at_unix_s: i64) -> UrlHistoryEntry {
        UrlHistoryEntry {
            url: url.to_string(),
            title: None,
            hit_count,
            last_used_at_unix_s,
        }
    }

    #[test]
    fn ranks_frequent_and_recent_first() {
        let now = 1_700_000_000;
        let ranked = ranked_url_history(
            vec![
                entry("https://github.com/old", 1, now - 60 * 60 * 24 * 30),
                entry("https://github.com", 40, now),
            ],
            "GitHub ",
        );
        assert_eq!(ranked[0].url, "https://github.com");
        assert!(ranked[0].score > ranked[1].score);
    }

    #[test]
    fn breaks_score_ties_by_recency() {
        let now = 1_700_000_000;
        let ranked = ranked_url_history(
            vec![
                entry("https://a.com", 2, now - 30),
                entry("https://b.com", 2, now),
            ],
            "com",
        );
        assert_eq!(ranked[0].url, "https://b.com");
    }
}
