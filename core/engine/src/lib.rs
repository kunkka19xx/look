pub mod action;
pub mod config;
pub mod index;
mod normalize;
mod platform;
mod query;
pub mod result;
mod scoring;
mod search;

pub use action::{ActionKind, LaunchAction};
use config::RuntimeConfig;
use look_indexing::{Candidate, CandidateKind};
use look_storage::{SearchSettings, SqliteStore, StorageError};
use normalize::normalize_for_search;
pub use result::{LaunchResult, LaunchResultAction};
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const INDEX_UPSERT_CHUNK_SIZE: usize = 512;
const USAGE_RETENTION_DAYS: i64 = 90;
const MAX_USAGE_EVENT_ROWS: usize = 50_000;

struct IndexedCandidate {
    candidate: Candidate,
    // Search-normalized fields are precomputed once at load time so the query loop
    // does not allocate per candidate/per keystroke.
    title_search: String,
    subtitle_search: Option<String>,
    path_search: String,
}

#[derive(Default)]
pub struct QueryEngine {
    candidates: Vec<IndexedCandidate>,
    search_aliases: HashMap<String, Vec<String>>,
}

impl QueryEngine {
    pub fn new(candidates: Vec<Candidate>) -> Self {
        let runtime_config = RuntimeConfig::default();
        Self::new_with_config(candidates, &runtime_config)
    }

    pub fn new_with_config(candidates: Vec<Candidate>, config: &RuntimeConfig) -> Self {
        // Build an in-memory search index up front (hot path reads only).
        let candidates = candidates.into_iter().map(IndexedCandidate::new).collect();
        Self {
            candidates,
            search_aliases: config.search_aliases.clone(),
        }
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<LaunchResult> {
        let scored = self.search_scored(query, limit);
        scored
            .into_iter()
            .map(|(candidate, score)| LaunchResult::from((&candidate, score)))
            .collect()
    }

    pub fn record_usage_in_memory(&mut self, candidate_id: &str, used_at_unix_s: i64) -> bool {
        if let Some(indexed) = self
            .candidates
            .iter_mut()
            .find(|c| c.candidate.id.as_ref() == candidate_id)
        {
            indexed.candidate.use_count = indexed.candidate.use_count.saturating_add(1);
            indexed.candidate.last_used_at_unix_s = Some(used_at_unix_s);
            return true;
        }
        false
    }

    pub fn demo_seed() -> Self {
        Self::new(Self::demo_candidates())
    }

    pub fn demo_candidates() -> Vec<Candidate> {
        vec![
            Candidate::new(
                "app:safari",
                CandidateKind::App,
                "Safari",
                "/Applications/Safari.app",
            ),
            Candidate::new(
                "app:vscode",
                CandidateKind::App,
                "Visual Studio Code",
                "/Applications/Visual Studio Code.app",
            ),
            Candidate::new(
                "file:notes",
                CandidateKind::File,
                "Notes.txt",
                "/Users/user/Documents/Notes.txt",
            ),
            Candidate::new(
                "folder:docs",
                CandidateKind::Folder,
                "Documents",
                "/Users/user/Documents",
            ),
        ]
    }

    pub fn from_sqlite(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let runtime_config = RuntimeConfig::load();
        let store = SqliteStore::open(path)?;
        let candidates = store.load_candidates(None)?;
        if candidates.is_empty() {
            return Ok(Self::new_with_config(
                Self::demo_candidates(),
                &runtime_config,
            ));
        }

        Ok(Self::new_with_config(candidates, &runtime_config))
    }

    pub fn bootstrap_sqlite(path: impl AsRef<Path>) -> Result<(), StorageError> {
        let mut store = SqliteStore::open(path)?;
        let runtime_config = RuntimeConfig::load();
        let run_started_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| StorageError::Data(format!("system time error: {err}")))?
            .as_secs() as i64;
        let existing = store.load_candidates(None)?;
        let should_replace = looks_like_demo_seed(&existing);
        if should_replace {
            // Clear demo rows first, then progressively stream real candidates.
            store.replace_candidates(&[])?;
        }

        let (rx, producer_handle) = index::discover_candidates_stream(&runtime_config).into_parts();
        let mut seen = HashSet::new();
        let mut chunk = Vec::with_capacity(INDEX_UPSERT_CHUNK_SIZE);
        let mut discovered_count = 0usize;
        for candidate in rx {
            if !seen.insert(candidate.id.clone()) {
                continue;
            }
            discovered_count += 1;
            chunk.push(candidate);
            if chunk.len() >= INDEX_UPSERT_CHUNK_SIZE {
                store.upsert_candidates_indexed(&chunk, Some(run_started_at))?;
                chunk.clear();
            }
        }

        if !chunk.is_empty() {
            store.upsert_candidates_indexed(&chunk, Some(run_started_at))?;
        }

        if let Err(err) = producer_handle.join() {
            eprintln!("look index: producer worker panicked: {err:?}");
        }

        if discovered_count > 0 {
            let _ = store.delete_stale_candidates(run_started_at)?;
        }

        let usage_cutoff = run_started_at.saturating_sub(USAGE_RETENTION_DAYS * 24 * 3600);
        let _ = store.prune_usage_events_older_than(usage_cutoff)?;
        let _ = store.prune_usage_events_to_max(MAX_USAGE_EVENT_ROWS)?;

        Ok(())
    }

    pub fn build_web_search_url(query: &str, settings: SearchSettings) -> Option<String> {
        let normalized_query = query.trim();
        if !settings.web_search_enabled || normalized_query.is_empty() {
            return None;
        }

        Some(
            settings
                .web_search_engine
                .build_search_url(normalized_query),
        )
    }
}

impl IndexedCandidate {
    fn new(candidate: Candidate) -> Self {
        // Normalize once; reuse for fuzzy/contains/path scoring.
        let title_search = normalize_for_search(&candidate.title);
        let subtitle_search = candidate
            .subtitle
            .as_ref()
            .map(|subtitle| normalize_for_search(subtitle));
        let path_search = normalize_for_search(&candidate.path);
        Self {
            candidate,
            title_search,
            subtitle_search,
            path_search,
        }
    }
}

fn looks_like_demo_seed(candidates: &[Candidate]) -> bool {
    if candidates.len() > 6 {
        return false;
    }

    let ids: HashSet<&str> = candidates.iter().map(|c| c.id.as_ref()).collect();
    (ids.contains("app:safari") || ids.contains("app.safari"))
        && (ids.contains("app:vscode") || ids.contains("app.vscode"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scoring::default_browse_score;

    fn sample_engine() -> QueryEngine {
        QueryEngine::new(vec![
            Candidate::new(
                "app:safari",
                CandidateKind::App,
                "Safari",
                "/Applications/Safari.app",
            ),
            Candidate::new(
                "app:vscode",
                CandidateKind::App,
                "Visual Studio Code",
                "/Applications/Visual Studio Code.app",
            ),
            Candidate::new(
                "file:notes",
                CandidateKind::File,
                "Notes.txt",
                "/Users/test/Documents/Notes.txt",
            ),
            Candidate::new(
                "folder:docs",
                CandidateKind::Folder,
                "Documents",
                "/Users/test/Documents",
            ),
        ])
    }

    #[test]
    fn app_prefix_filters_to_apps() {
        let engine = sample_engine();
        let results = engine.search_scored("a\"saf", 10);
        assert!(
            results
                .iter()
                .all(|(candidate, _)| candidate.kind == CandidateKind::App)
        );
        assert!(
            results
                .iter()
                .any(|(candidate, _)| candidate.id.as_ref() == "app:safari")
        );
    }

    #[test]
    fn file_prefix_filters_to_files() {
        let engine = sample_engine();
        let results = engine.search_scored("f\"notes", 10);
        assert!(
            results
                .iter()
                .all(|(candidate, _)| candidate.kind == CandidateKind::File)
        );
        assert_eq!(
            results.first().map(|(candidate, _)| candidate.id.as_ref()),
            Some("file:notes")
        );
    }

    #[test]
    fn directory_prefix_filters_to_folders() {
        let engine = sample_engine();
        let results = engine.search_scored("d\"doc", 10);
        assert!(
            results
                .iter()
                .all(|(candidate, _)| candidate.kind == CandidateKind::Folder)
        );
        assert_eq!(
            results.first().map(|(candidate, _)| candidate.id.as_ref()),
            Some("folder:docs")
        );
    }

    #[test]
    fn regex_prefix_matches_by_pattern() {
        let engine = sample_engine();
        let results = engine.search_scored("r\"^Visual.*Code$", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.id.as_ref(), "app:vscode");
    }

    #[test]
    fn regex_prefix_returns_empty_on_invalid_pattern() {
        let engine = sample_engine();
        let results = engine.search_scored("r\"([", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn vietnamese_diacritics_query_matches_ascii_titles() {
        let engine = QueryEngine::new(vec![Candidate::new(
            "app:terminal",
            CandidateKind::App,
            "Terminal",
            "/System/Applications/Utilities/Terminal.app",
        )]);

        let results = engine.search_scored("tẻrminal", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.id.as_ref(), "app:terminal");
    }

    #[test]
    fn keychain_query_matches_keychain_access_app() {
        let engine = QueryEngine::new(vec![
            Candidate::new(
                "app:keychain",
                CandidateKind::App,
                "Keychain Access",
                "/System/Library/CoreServices/Applications/Keychain Access.app",
            ),
            Candidate::new(
                "app:archive",
                CandidateKind::App,
                "Archive Utility",
                "/System/Library/CoreServices/Applications/Archive Utility.app",
            ),
        ]);

        let results = engine.search_scored("keychain", 10);
        assert_eq!(
            results.first().map(|(candidate, _)| candidate.id.as_ref()),
            Some("app:keychain")
        );
    }

    #[test]
    fn empty_query_prioritizes_recent_and_frequent_apps() {
        let mut frequent_app = Candidate::new(
            "app.frequent",
            CandidateKind::App,
            "Frequent",
            "/Applications/Frequent.app",
        );
        frequent_app.use_count = 25;
        frequent_app.last_used_at_unix_s = Some(4_102_444_800);

        let mut less_used_app = Candidate::new(
            "app.less",
            CandidateKind::App,
            "Less",
            "/Applications/Less.app",
        );
        less_used_app.use_count = 1;

        let folder = Candidate::new(
            "folder:docs",
            CandidateKind::Folder,
            "Documents",
            "/Users/test/Documents",
        );

        let file = Candidate::new(
            "file:notes",
            CandidateKind::File,
            "Notes.txt",
            "/Users/test/Documents/Notes.txt",
        );

        let engine = QueryEngine::new(vec![file, folder, less_used_app, frequent_app]);
        let results = engine.search_scored("", 4);
        let ordered_ids: Vec<&str> = results
            .iter()
            .map(|(candidate, _)| candidate.id.as_ref())
            .collect();

        assert_eq!(ordered_ids[0], "app.frequent");
        assert_eq!(ordered_ids[1], "app.less");
        assert!(
            ordered_ids.iter().position(|id| *id == "folder:docs")
                < ordered_ids.iter().position(|id| *id == "file:notes")
        );
    }

    #[test]
    fn empty_query_can_prioritize_frequent_settings_entries() {
        let now = 1_775_462_400; // 2026-04-06 16:00:00 UTC

        let mut display_setting = Candidate::new(
            "setting:com.apple.displays-settings.extension",
            CandidateKind::App,
            "Display",
            "x-apple.systempreferences:com.apple.displays-settings.extension",
        );
        display_setting.subtitle = Some("System Settings display monitor".into());
        display_setting.use_count = 16;
        display_setting.last_used_at_unix_s = Some(now - 60 * 60 * 20);

        let mut newly_opened_app = Candidate::new(
            "app.new",
            CandidateKind::App,
            "Newly Opened",
            "/Applications/Newly Opened.app",
        );
        newly_opened_app.use_count = 1;
        newly_opened_app.last_used_at_unix_s = Some(now);

        assert!(
            default_browse_score(&display_setting, now)
                > default_browse_score(&newly_opened_app, now)
        );

        let engine = QueryEngine::new(vec![newly_opened_app, display_setting]);
        let results = engine.search_scored("", 10);
        assert_eq!(
            results.first().map(|(candidate, _)| candidate.id.as_ref()),
            Some("setting:com.apple.displays-settings.extension")
        );
    }

    #[test]
    fn empty_query_prefers_more_recent_app_when_usage_is_equal() {
        // Use actual current time so test works regardless of when it runs
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let mut display_setting = Candidate::new(
            "setting:com.apple.displays-settings.extension",
            CandidateKind::App,
            "Display",
            "x-apple.systempreferences:com.apple.displays-settings.extension",
        );
        display_setting.subtitle = Some("System Settings display monitor".into());
        display_setting.use_count = 1;
        display_setting.last_used_at_unix_s = Some(now - 60 * 60 * 12); // 12 hours ago

        let mut newly_opened_app = Candidate::new(
            "app.new",
            CandidateKind::App,
            "Newly Opened",
            "/Applications/Newly Opened.app",
        );
        newly_opened_app.use_count = 1;
        newly_opened_app.last_used_at_unix_s = Some(now); // Just now

        assert!(
            default_browse_score(&newly_opened_app, now)
                > default_browse_score(&display_setting, now)
        );

        let engine = QueryEngine::new(vec![display_setting, newly_opened_app]);
        let results = engine.search_scored("", 10);
        assert_eq!(
            results.first().map(|(candidate, _)| candidate.id.as_ref()),
            Some("app.new")
        );
    }

    #[test]
    fn slash_path_query_matches_nested_path_segments() {
        let engine = QueryEngine::new(vec![
            Candidate::new(
                "file.repo.readme",
                CandidateKind::File,
                "README.md",
                "/Users/test/Documents/git/books-pc/README.md",
            ),
            Candidate::new(
                "file.other",
                CandidateKind::File,
                "todo.txt",
                "/Users/test/Downloads/todo.txt",
            ),
        ]);

        let results = engine.search_scored("git/books-pc", 10);
        assert!(!results.is_empty());
        assert_eq!(results[0].0.id.as_ref(), "file.repo.readme");
    }

    #[test]
    fn ambiguous_query_ingo_prefers_relevant_file_over_settings_alias_noise() {
        let mut settings = Candidate::new(
            "setting:network",
            CandidateKind::App,
            "Network",
            "x-apple.systempreferences:com.apple.preference.network",
        );
        settings.subtitle =
            Some("System Settings settings network ethernet dns proxy vpn notifications".into());

        let file = Candidate::new(
            "file.concurrency",
            CandidateKind::File,
            "Concurrency in Go.pdf",
            "/Users/test/Documents/books/Concurrency in Go.pdf",
        );

        let engine = QueryEngine::new(vec![settings, file]);
        let results = engine.search_scored("ingo", 10);
        assert_eq!(
            results.first().map(|(candidate, _)| candidate.id.as_ref()),
            Some("file.concurrency")
        );
    }

    #[test]
    fn settings_prefix_query_sett_prioritizes_system_settings_entry() {
        let mut settings_app = Candidate::new(
            "setting:general",
            CandidateKind::App,
            "General",
            "x-apple.systempreferences:com.apple.preference.general",
        );
        settings_app.subtitle = Some("System Settings settings general".into());

        let settings_folder = Candidate::new(
            "folder.settings",
            CandidateKind::Folder,
            "settings",
            "/Users/test/Documents/settings",
        );

        let engine = QueryEngine::new(vec![settings_folder, settings_app]);
        let results = engine.search_scored("sett", 10);
        assert_eq!(
            results.first().map(|(candidate, _)| candidate.id.as_ref()),
            Some("setting:general")
        );
    }

    #[test]
    fn alias_note_promotes_matching_app_results() {
        let mut config = RuntimeConfig::default();
        config
            .search_aliases
            .insert("note".to_string(), vec!["notion".to_string()]);

        let app = Candidate::new(
            "app.notion",
            CandidateKind::App,
            "Notion",
            "/Applications/Notion.app",
        );
        let file = Candidate::new(
            "file.note",
            CandidateKind::File,
            "notes.txt",
            "/Users/test/Documents/notes.txt",
        );

        let engine = QueryEngine::new_with_config(vec![app, file], &config);
        let results = engine.search_scored("note", 10);
        assert_eq!(
            results.first().map(|(candidate, _)| candidate.id.as_ref()),
            Some("app.notion")
        );
    }

    #[test]
    fn alias_is_not_applied_for_file_scope_queries() {
        let mut config = RuntimeConfig::default();
        config
            .search_aliases
            .insert("note".to_string(), vec!["notion".to_string()]);

        let app = Candidate::new(
            "app.notion",
            CandidateKind::App,
            "Notion",
            "/Applications/Notion.app",
        );
        let file = Candidate::new(
            "file.note",
            CandidateKind::File,
            "notes.txt",
            "/Users/test/Documents/notes.txt",
        );

        let engine = QueryEngine::new_with_config(vec![app, file], &config);
        let results = engine.search_scored("f\"note", 10);
        assert!(
            results
                .iter()
                .all(|(candidate, _)| candidate.kind == CandidateKind::File)
        );
        assert_eq!(
            results.first().map(|(candidate, _)| candidate.id.as_ref()),
            Some("file.note")
        );
    }
    #[test]
    fn alias_brow_does_not_promote_archive_for_arc_term() {
        let mut config = RuntimeConfig::default();
        config
            .search_aliases
            .insert("brow".to_string(), vec!["arc".to_string()]);

        let mut archive = Candidate::new(
            "app.archive",
            CandidateKind::App,
            "Archive Utility",
            "/System/Library/CoreServices/Applications/Archive Utility.app",
        );
        archive.use_count = 2_000;

        let arc = Candidate::new(
            "app.arc",
            CandidateKind::App,
            "Arc",
            "/Applications/Arc.app",
        );

        let engine = QueryEngine::new_with_config(vec![archive, arc], &config);
        let results = engine.search_scored("brow", 10);
        assert_eq!(
            results.first().map(|(candidate, _)| candidate.id.as_ref()),
            Some("app.arc")
        );
    }

    #[test]
    fn alias_can_match_system_settings_subtitle_terms() {
        let mut config = RuntimeConfig::default();
        config
            .search_aliases
            .insert("update".to_string(), vec!["software update".to_string()]);

        let mut settings = Candidate::new(
            "setting:update",
            CandidateKind::App,
            "General",
            "x-apple.systempreferences:com.apple.preference.general",
        );
        settings.subtitle = Some("System Settings software update".into());

        let app = Candidate::new(
            "app.updates",
            CandidateKind::App,
            "General Helper",
            "/Applications/General Helper.app",
        );

        let engine = QueryEngine::new_with_config(vec![app, settings], &config);
        let results = engine.search_scored("update", 10);
        assert_eq!(
            results.first().map(|(candidate, _)| candidate.id.as_ref()),
            Some("setting:update")
        );
    }
}
