use crate::state::{cstr_to_string, default_db_path, refresh_engine_cache};
use look_indexing::{Candidate, CandidateKind};
use look_storage::SqliteStore;
use serde::Deserialize;
use std::collections::HashSet;
use std::os::raw::c_char;

const UWP_ID_PREFIX: &str = "app:uwp:";

#[derive(Deserialize)]
struct UwpAppPayload {
    aumid: String,
    title: String,
}

// Seeds UWP shell:AppsFolder entries into the Rust candidates table so they
// participate in normal ranking (rank_score, kind_bias, recency, use_count) instead
// of going through a parallel C# scoring path. C# UwpAppService enumerates AppsFolder
// once on app start and posts the results here as JSON: [{"aumid": "...", "title": "..."}, ...]
//
// Each entry is upserted as:
//   id    = app:uwp:<AUMID>          - `app:` prefix so look_record_usage accepts it
//   kind  = App
//   title = <DisplayName>            - e.g. "Terminal", "Notepad"
//   path  = shell:AppsFolder\<AUMID> - ShellExecuteService dispatches via explorer.exe
//
// indexed_at_unix_s is set to i64::MAX so QueryEngine::bootstrap_sqlite's
// `delete_stale_candidates(run_started_at)` sweep (core/engine/src/lib.rs:163) leaves
// these rows alone - the Rust app discovery stream never produces UWP entries, so
// without the MAX sentinel they'd be pruned on every index refresh.
pub(crate) fn look_seed_uwp_apps_json_impl(json: *const c_char) -> bool {
    let json = cstr_to_string(json);
    if json.trim().is_empty() {
        return false;
    }

    let entries: Vec<UwpAppPayload> = match serde_json::from_str(&json) {
        Ok(v) => v,
        Err(_) => return false,
    };

    let mut candidates: Vec<Candidate> = Vec::with_capacity(entries.len());
    let mut kept_ids: Vec<String> = Vec::with_capacity(entries.len());
    for e in entries {
        let aumid = e.aumid.trim();
        let title = e.title.trim();
        if aumid.is_empty() || title.is_empty() || !aumid.contains('!') {
            continue;
        }
        let id = format!("{}{}", UWP_ID_PREFIX, aumid);
        kept_ids.push(id.clone());
        // Defaults cover use_count/last_used_at_unix_s (preserved by the ON CONFLICT
        // clause in upsert_candidates_indexed, so re-seeding on every app start
        // doesn't reset launch history) and fs_modified_at_unix_s (an app candidate
        // has no filesystem mtime and isn't part of the recent-files view).
        candidates.push(Candidate {
            id: id.into(),
            kind: CandidateKind::App,
            title: title.into(),
            subtitle: None,
            path: format!("shell:AppsFolder\\{}", aumid).into(),
            ..Default::default()
        });
    }

    if candidates.is_empty() {
        return false;
    }

    let Ok(mut store) = SqliteStore::open(default_db_path()) else {
        return false;
    };

    if store
        .upsert_candidates_indexed(&candidates, Some(i64::MAX))
        .is_err()
    {
        return false;
    }

    // Drop rows for UWP apps that vanished from AppsFolder since the last seed
    // (uninstalls / package renames). Without this, those rows would persist
    // forever - `delete_stale_candidates` skips them because their indexed_at is
    // i64::MAX, and they'd keep showing in search with their old usage weight.
    let keep_set: HashSet<&str> = kept_ids.iter().map(|s| s.as_str()).collect();
    let _ = store.delete_candidates_by_prefix_except(UWP_ID_PREFIX, &keep_set);

    refresh_engine_cache();
    true
}
