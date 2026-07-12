//! C-ABI wrappers over `look_storage`'s URL history, so the macOS Swift shell
//! records launcher-opened URLs and queries them back through the same look.db
//! linows uses. Direct-store access (own connection, own table), mirroring
//! `todo_api`; both endpoints are best-effort and panic-safe at `lib.rs`.

use crate::state::{cstr_to_string, default_db_path, store_json_allocation};
use look_engine::config::SCORE_TITLE_CONTAINS;
use look_indexing::{Candidate, CandidateKind};
use look_ranking::rank_score;
use look_storage::{SqliteStore, UrlHistoryEntry};
use serde::Serialize;
use std::ffi::CString;
use std::os::raw::c_char;

const JSON_EMPTY_ARRAY: &str = "[]";

/// Wire shape of a URL-history row. `look_storage::UrlHistoryEntry` is not
/// `Serialize` (that crate has no serde dep), so map it here. `score` is the
/// frecency rank so the UI can place recent URLs among local results by the same
/// math the engine uses for apps/files, not an arbitrary hit-count threshold.
#[derive(Serialize)]
struct UrlHistoryDto {
    url: String,
    title: Option<String>,
    hit_count: u64,
    last_used_at_unix_s: i64,
    score: i64,
}

/// Ranks a remembered URL for `query` with the exact `rank_score` the engine
/// applies to matched candidates: a title-contains base plus log-scaled
/// frequency (`hit_count`) and a recency bonus. `recent_urls` only returns
/// substring matches, so the base is always the title-contains score.
fn score_entry(entry: &UrlHistoryEntry, query: &str) -> i64 {
    let mut candidate = Candidate::new(&entry.url, CandidateKind::File, &entry.url, &entry.url);
    candidate.use_count = entry.hit_count;
    candidate.last_used_at_unix_s = Some(entry.last_used_at_unix_s);
    let title_lower = entry.url.to_lowercase();
    rank_score(SCORE_TITLE_CONTAINS, query, &candidate, &title_lower)
}

/// Scores each entry for `query` and returns the DTOs in frecency order (highest
/// score first, most-recent breaking ties).
fn ranked_dtos(entries: Vec<UrlHistoryEntry>, query: &str) -> Vec<UrlHistoryDto> {
    let mut dtos: Vec<UrlHistoryDto> = entries
        .into_iter()
        .map(|e| {
            let score = score_entry(&e, query);
            UrlHistoryDto {
                url: e.url,
                title: e.title,
                hit_count: e.hit_count,
                last_used_at_unix_s: e.last_used_at_unix_s,
                score,
            }
        })
        .collect();
    dtos.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then(b.last_used_at_unix_s.cmp(&a.last_used_at_unix_s))
    });
    dtos
}

/// Records that `url` was opened. Returns false on empty input or any store
/// failure; the caller treats recording as fire-and-forget.
pub(crate) fn look_record_url_hit_impl(url: *const c_char) -> bool {
    let url = cstr_to_string(url);
    if url.trim().is_empty() {
        return false;
    }
    let Ok(store) = SqliteStore::open(default_db_path()) else {
        return false;
    };
    store.record_url_hit(&url).is_ok()
}

/// JSON array of up to `limit` remembered URLs matching `query` (most-recent
/// first), or `[]` on any failure.
pub(crate) fn look_recent_urls_json_impl(query: *const c_char, limit: u32) -> *mut c_char {
    let query = cstr_to_string(query);
    let query_norm = query.trim().to_lowercase();
    let json = SqliteStore::open(default_db_path())
        .ok()
        .and_then(|store| store.recent_urls(&query, limit as usize).ok())
        .map(|entries| ranked_dtos(entries, &query_norm))
        .and_then(|dtos| serde_json::to_string(&dtos).ok())
        .unwrap_or_else(|| JSON_EMPTY_ARRAY.to_string());
    let cstring =
        CString::new(json).unwrap_or_else(|_| CString::new(JSON_EMPTY_ARRAY).expect("valid"));
    store_json_allocation(cstring)
}
