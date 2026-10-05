//! The query path: ranked results from the engine, usage recording, and the
//! reload that refreshes everything the config feeds.

use crate::state::AppState;
use look_engine::config::RuntimeConfig;
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

/// camelCase for the frontend. Every field predating it is one lowercase word,
/// so the rename only reaches the ones added since.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub path: String,
    pub score: i64,
    /// What the row declared, which beats its block's icon.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// When the row was last opened through Look, for the preview's "Last used".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_used_at_unix_s: Option<i64>,
}

#[derive(Serialize)]
pub struct SearchPayload {
    pub count: usize,
    pub results: Vec<SearchResult>,
}

#[derive(Serialize)]
pub struct UsageResult {
    pub ok: bool,
    pub error: Option<String>,
}

const DEFAULT_SEARCH_LIMIT: u32 = 40;
const MAX_SEARCH_LIMIT: u32 = 100;
pub fn search(state: &AppState, query: &str, limit: u32) -> SearchPayload {
    let max = if limit == 0 {
        DEFAULT_SEARCH_LIMIT
    } else {
        limit.min(MAX_SEARCH_LIMIT)
    } as usize;

    let scored = state.with_engine(|engine| engine.search_scored(query, max));

    let results: Vec<SearchResult> = scored
        .into_iter()
        .map(|(candidate, score)| SearchResult {
            id: candidate.id.to_string(),
            kind: candidate.kind.as_str().to_string(),
            title: candidate.title.to_string(),
            subtitle: candidate.subtitle.as_deref().map(str::to_string),
            path: candidate.path.to_string(),
            score,
            icon: candidate.icon.as_deref().map(str::to_string),
            last_used_at_unix_s: candidate.last_used_at_unix_s,
        })
        .collect();

    SearchPayload {
        count: results.len(),
        results,
    }
}
pub fn record_usage(state: &AppState, candidate_id: &str, action: &str) -> UsageResult {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    if action.parse::<look_engine::UsageAction>().is_err() {
        return UsageResult {
            ok: false,
            error: Some(format!("Invalid action: {action}")),
        };
    }

    // A drilled row is never written to `candidates`, which the `usage_events`
    // foreign key requires, so there is nothing to record. Reporting a failure
    // would show a storage error for behaving as designed.
    if look_engine::sources::is_drilled_row(candidate_id) {
        return UsageResult {
            ok: true,
            error: None,
        };
    }

    let found = state.with_engine_mut(|engine| engine.record_usage_in_memory(candidate_id, now));

    if found {
        let db_path = crate::state::default_db_path();
        if let Ok(store) = look_storage::SqliteStore::open(&db_path) {
            let _ = store.record_usage_event(candidate_id, action);
        }
    }

    UsageResult {
        ok: found,
        error: if found {
            None
        } else {
            Some(format!("Candidate not found: {candidate_id}"))
        },
    }
}

/// Reload is the one refresh gesture: config, then the `run` blocks, then the
/// index.
///
/// A user's `run` block is a process, so the shell keeps this off its request
/// thread. Returns what the blocks did, so a script that broke says so rather
/// than quietly producing no rows. The shell re-binds its hotkey afterwards,
/// since the config may have changed it.
pub fn reload_config(state: &AppState) -> look_engine::sources::RefreshOutcome {
    // The engine caches the parsed `~/.look/config` across calls (skips a disk
    // read on every refresh). When the user explicitly reloads, drop the cache
    // so the next bootstrap picks up their edits.
    RuntimeConfig::invalidate_cache();
    crate::clipboard::reload_from_config();
    // Before the index pass, never after: the pass reads the rows these blocks
    // write, and the other order indexes the previous run's.
    let sources = look_engine::sources::refresh_run_blocks();
    // Run rows land where no watcher covers, so nothing else marks them dirty.
    if sources.changed {
        state.force_index_refresh();
    } else {
        state.request_index_refresh();
    }
    sources
}
