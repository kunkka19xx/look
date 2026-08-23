pub mod action;
pub mod config;
pub mod config_path;
pub mod index;
mod normalize;
mod platform;
mod query;
pub mod result;
mod scoring;
mod search;
pub mod url_history;

pub use action::{ActionKind, LaunchAction};
use config::RuntimeConfig;
use look_indexing::{Candidate, CandidateIdKind, CandidateKind};
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

/// Structured natural-language file query (see `QueryEngine::search_files`).
/// The shell parses free text into this via `core/ai/files`.
#[derive(Debug, Default)]
pub struct FileFilter {
    /// Free text for name matching ("resume", "taxes"); empty = no name filter.
    pub terms: String,
    /// Categories to keep: pdf, image, screenshot, movie, audio, document,
    /// spreadsheet, presentation, folder, archive. Empty = any file/folder.
    pub categories: Vec<String>,
    /// Modified-time range (unix seconds), inclusive; None = no time filter.
    pub start: Option<i64>,
    pub end: Option<i64>,
    /// Folder hints to scope to: downloads, desktop, documents.
    pub locations: Vec<String>,
}

/// Which fallback produced a non-empty file-recall result (see
/// `QueryEngine::relaxations`); None = the strict query matched.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FileSearchRelaxation {
    WidenedWindow,
    DroppedTerms,
    DroppedTermsWidenedWindow,
}

pub struct FileSearchOutcome {
    pub results: Vec<LaunchResult>,
    pub relaxation: Option<FileSearchRelaxation>,
}

struct IndexedCandidate {
    candidate: Candidate,
    // Search-normalized fields are precomputed once at load time so the query loop
    // does not allocate per candidate/per keystroke.
    title_search: String,
    subtitle_search: Option<String>,
    path_search: String,
    /// Matched but never shown. A source row is found by the block that made it,
    /// and the block's words cannot live in the subtitle, which the row may
    /// have declared for itself.
    keywords_search: Option<String>,
}

#[derive(Default)]
pub struct QueryEngine {
    candidates: Vec<IndexedCandidate>,
    search_aliases: HashMap<String, Vec<String>>,
    /// Per-block score offset from a declared `bias`, keyed by block id. Kept
    /// beside the candidates rather than on them: the bias belongs to the
    /// declaration, so editing it must take effect without reindexing every row
    /// the block produced.
    source_biases: HashMap<String, i64>,
}

impl QueryEngine {
    pub fn new(candidates: Vec<Candidate>) -> Self {
        let runtime_config = RuntimeConfig::default();
        Self::new_with_config(candidates, &runtime_config)
    }

    pub fn new_with_config(candidates: Vec<Candidate>, config: &RuntimeConfig) -> Self {
        let declared = index::declared_blocks();
        let block_names: HashMap<&str, &str> = declared
            .iter()
            .map(|block| (block.id.as_str(), block.name.as_str()))
            .collect();
        // Build an in-memory search index up front (hot path reads only).
        let candidates = candidates
            .into_iter()
            .map(|candidate| IndexedCandidate::new(candidate, &block_names))
            .collect();
        let mut search_aliases = config.search_aliases.clone();
        // A block's `aliases` are extra words that should find its rows. They
        // point at the block name, which every one of its rows carries in its
        // search keywords.
        for block in &declared {
            for alias in &block.aliases {
                let key = normalize_for_search(alias);
                if key.is_empty() {
                    continue;
                }
                search_aliases
                    .entry(key)
                    .or_default()
                    .push(normalize_for_search(&block.name));
            }
        }

        let source_biases = declared
            .iter()
            .filter(|block| block.bias != 0)
            .map(|block| (block.id.clone(), block.bias))
            .collect();

        Self {
            candidates,
            search_aliases,
            source_biases,
        }
    }

    /// The declared bias for the block that produced `candidate`, or 0.
    pub(crate) fn source_bias(&self, candidate: &Candidate) -> i64 {
        if self.source_biases.is_empty() {
            return 0;
        }
        CandidateIdKind::source_id_of(&candidate.id)
            .and_then(|id| self.source_biases.get(id))
            .copied()
            .unwrap_or(0)
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<LaunchResult> {
        let scored = self.search_scored(query, limit);
        scored
            .into_iter()
            .map(|(candidate, score)| LaunchResult::from((&candidate, score)))
            .collect()
    }

    /// Natural-language file recall: files/folders filtered by type, modified
    /// time, and location, ranked most-recent-first. Runs against Look's own
    /// index (fast, no Spotlight); the shell parses the query into a `FileFilter`.
    /// An empty result relaxes the query progressively (see `relaxations`) so
    /// near-misses beat an empty panel; `relaxation` reports which fallback
    /// produced the results so the shell can say so instead of silently showing
    /// something other than what was asked.
    pub fn search_files(&self, filter: &FileFilter, limit: usize) -> FileSearchOutcome {
        let mut indices = self.search_files_indices(filter, limit);
        let mut relaxation = None;
        if indices.is_empty() {
            for (relaxed, kind) in Self::relaxations(filter) {
                indices = self.search_files_indices(&relaxed, limit);
                if !indices.is_empty() {
                    relaxation = Some(kind);
                    break;
                }
            }
        }
        let results = self
            .drop_source_shadowed(indices)
            .into_iter()
            .map(|(idx, score)| {
                LaunchResult::from((&self.candidates[idx as usize].candidate, score))
            })
            .collect();
        FileSearchOutcome {
            results,
            relaxation,
        }
    }

    /// Fallbacks for an empty file-recall result, ordered so the least user
    /// intent is lost first. Human time memory is fuzzy ("downloaded last
    /// week" often means "a week or so ago"), so the window doubles backward
    /// while the terms - the strongest signal of WHAT - are kept. Terms drop
    /// only after that: an unrecognized glue word the parser didn't strip
    /// ("files added to ...") lands in terms and would otherwise filter
    /// everything out. Type and location filters are never relaxed.
    ///
    /// Terms are kept when they are the ONLY filter: dropping them leaves a
    /// query with no constraint at all, which matches the entire index. That
    /// is not a near miss, it is every file the user owns, and it lets a
    /// misread query ("bitcoin price") claim the panel with unrelated rows.
    fn relaxations(filter: &FileFilter) -> Vec<(FileFilter, FileSearchRelaxation)> {
        let widened_window = match (filter.start, filter.end) {
            (Some(start), Some(end)) if end > start => Some((start - (end - start), end)),
            _ => None,
        };
        let has_terms = !filter.terms.trim().is_empty();
        let has_other_filter = !filter.categories.is_empty()
            || !filter.locations.is_empty()
            || filter.start.is_some()
            || filter.end.is_some();

        let build = |terms: &str, window: Option<(i64, i64)>| FileFilter {
            terms: terms.into(),
            categories: filter.categories.clone(),
            start: window.map(|w| w.0).or(filter.start),
            end: window.map(|w| w.1).or(filter.end),
            locations: filter.locations.clone(),
        };

        let mut steps = Vec::new();
        if let Some(window) = widened_window {
            steps.push((
                build(&filter.terms, Some(window)),
                FileSearchRelaxation::WidenedWindow,
            ));
        }
        if has_terms && has_other_filter {
            steps.push((build("", None), FileSearchRelaxation::DroppedTerms));
            if let Some(window) = widened_window {
                steps.push((
                    build("", Some(window)),
                    FileSearchRelaxation::DroppedTermsWidenedWindow,
                ));
            }
        }
        steps
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
        let runtime_config = RuntimeConfig::load_cached();
        let store = SqliteStore::open(path)?;
        let candidates = store.load_candidates(None)?;
        Ok(Self::new_with_config(candidates, &runtime_config))
    }

    pub fn bootstrap_sqlite(path: impl AsRef<Path>) -> Result<(), StorageError> {
        Self::bootstrap_sqlite_scoped(path, BootstrapScope::ALL)
    }

    /// Like `bootstrap_sqlite`, but only re-walks the sources selected by `scope`
    /// and only prunes stale rows whose candidate id matches one of those sources.
    /// Used by the file watcher so that, e.g., a change inside an apps directory
    /// does not force a full rescan of every file root.
    pub fn bootstrap_sqlite_scoped(
        path: impl AsRef<Path>,
        scope: BootstrapScope,
    ) -> Result<(), StorageError> {
        let mut store = SqliteStore::open(path)?;
        let runtime_config = RuntimeConfig::load_cached();
        let run_started_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| StorageError::Data(format!("system time error: {err}")))?
            .as_secs() as i64;
        if store.is_demo_seeded()? {
            // Clear demo rows first, then progressively stream real candidates.
            store.replace_candidates(&[])?;
        }

        let (rx, producer_handle) =
            index::discover_candidates_stream_scoped(&runtime_config, scope).into_parts();
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

        // Stale-row sweep. The `ALL` branch keeps the "discovered something"
        // guard as a crash-shaped failsafe - if a full bootstrap silently
        // produced zero candidates we'd rather leave the DB alone than wipe
        // every row. Scoped paths are different: when the watcher fires an
        // `APPS_ONLY` refresh, "zero discovered" is the legitimate "user just
        // uninstalled their last app in this root" outcome, and we must still
        // sweep the matching prefixes or the deleted row lingers forever
        // (only an `ALL` refresh would otherwise catch it).
        // Prune by the `seen` set rather than the old "indexed_at < run_started"
        // sweep: the change-detecting upsert (see specs/indexing-scale.md) no
        // longer bumps indexed_at on unchanged rows, so only "not seen this scan"
        // reliably means "gone". delete_unseen_candidates keeps the indexed_at<run
        // guard to preserve i64::MAX pinned rows. `seen` is already collected above
        // for dedup, so this reuses it.
        // TODO(indexing-scale Direction A): this still required a full walk to
        // build `seen`. Event-driven incremental indexing (watcher paths) would
        // delete only the paths the watcher reported removed.
        let prefixes = scope.id_prefixes();
        if scope.is_all() {
            if discovered_count > 0 {
                let _ = store.delete_unseen_candidates(&seen, run_started_at, &[])?;
            }
        } else if !prefixes.is_empty() {
            let _ = store.delete_unseen_candidates(&seen, run_started_at, &prefixes)?;
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
    fn new(candidate: Candidate, block_names: &HashMap<&str, &str>) -> Self {
        // Normalize once; reuse for fuzzy/contains/path scoring.
        let title_search = normalize_for_search(&candidate.title);
        let subtitle_search = candidate
            .subtitle
            .as_ref()
            .map(|subtitle| normalize_for_search(subtitle));
        let path_search = normalize_for_search(&candidate.path);
        // Both the name the user reads and the id they wrote in the file: either
        // is what they will type to reach the block's rows.
        let keywords_search = CandidateIdKind::source_id_of(&candidate.id).map(|block_id| {
            match block_names.get(block_id) {
                Some(name) => normalize_for_search(&format!("{name} {block_id}")),
                None => normalize_for_search(block_id),
            }
        });
        Self {
            candidate,
            title_search,
            subtitle_search,
            path_search,
            keywords_search,
        }
    }
}

/// Selects which discovery sources `bootstrap_sqlite_scoped` should re-walk.
/// Each source maps 1:1 to a candidate id prefix; only candidates with those
/// prefixes are eligible for the post-walk stale sweep.
#[derive(Debug, Clone, Copy)]
pub struct BootstrapScope {
    pub apps: bool,
    pub files: bool,
    pub settings: bool,
    pub sources: bool,
}

impl BootstrapScope {
    pub const ALL: Self = Self {
        apps: true,
        files: true,
        settings: true,
        sources: true,
    };
    pub const APPS_ONLY: Self = Self {
        apps: true,
        files: false,
        settings: false,
        sources: false,
    };
    pub const FILES_ONLY: Self = Self {
        apps: false,
        files: true,
        settings: false,
        sources: false,
    };
    pub const SOURCES_ONLY: Self = Self {
        apps: false,
        files: false,
        settings: false,
        sources: true,
    };

    pub fn is_all(&self) -> bool {
        self.apps && self.files && self.settings && self.sources
    }

    pub fn is_empty(&self) -> bool {
        !(self.apps || self.files || self.settings || self.sources)
    }

    pub(crate) fn id_prefixes(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.apps {
            out.push(CandidateIdKind::PREFIX_APP);
        }
        if self.files {
            // file:* and folder:* are both produced by the files walker.
            out.push(CandidateIdKind::PREFIX_FILE);
            out.push(CandidateIdKind::PREFIX_FOLDER);
        }
        if self.settings {
            out.push(CandidateIdKind::PREFIX_SETTING);
        }
        if self.sources {
            out.push(CandidateIdKind::PREFIX_SOURCE);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scoring::default_browse_score;

    fn file_candidate(id: &str, name: &str, path: &str, modified: i64) -> Candidate {
        let mut c = Candidate::new(id, CandidateKind::File, name, path);
        c.fs_modified_at_unix_s = Some(modified);
        c
    }

    #[test]
    fn file_search_filters_by_window_and_location() {
        let now = 1_754_000_000;
        let engine = QueryEngine::new(vec![
            file_candidate(
                "file:a",
                "in-window.pdf",
                "/Users/u/Downloads/in-window.pdf",
                now - 86_400,
            ),
            file_candidate(
                "file:b",
                "old.pdf",
                "/Users/u/Downloads/old.pdf",
                now - 40 * 86_400,
            ),
            file_candidate(
                "file:c",
                "elsewhere.pdf",
                "/Users/u/Desktop/elsewhere.pdf",
                now - 86_400,
            ),
        ]);
        let filter = FileFilter {
            start: Some(now - 7 * 86_400),
            end: Some(now),
            locations: vec!["downloads".into()],
            ..Default::default()
        };
        let outcome = engine.search_files(&filter, 10);
        assert_eq!(outcome.results.len(), 1);
        assert_eq!(outcome.results[0].title, "in-window.pdf");
        assert!(outcome.relaxation.is_none());
    }

    #[test]
    fn empty_windowed_file_search_widens_backward_once() {
        // Newest download is 10 days old; a strict 7-day "last week" window
        // misses it. The retry doubles the window backward (14 days) and finds
        // it; a far older file stays excluded.
        let now = 1_754_000_000;
        let engine = QueryEngine::new(vec![
            file_candidate(
                "file:a",
                "recent-ish.dmg",
                "/Users/u/Downloads/recent-ish.dmg",
                now - 10 * 86_400,
            ),
            file_candidate(
                "file:b",
                "ancient.dmg",
                "/Users/u/Downloads/ancient.dmg",
                now - 60 * 86_400,
            ),
        ]);
        let filter = FileFilter {
            start: Some(now - 7 * 86_400),
            end: Some(now),
            ..Default::default()
        };
        let outcome = engine.search_files(&filter, 10);
        assert_eq!(outcome.results.len(), 1);
        assert_eq!(outcome.results[0].title, "recent-ish.dmg");
        assert_eq!(
            outcome.relaxation,
            Some(FileSearchRelaxation::WidenedWindow)
        );
    }

    #[test]
    fn unmatched_terms_drop_before_giving_up() {
        // An unrecognized glue word ("files added to desktop" with "added"
        // surviving as a term) must degrade to the location listing, not to an
        // empty panel.
        let now = 1_754_000_000;
        let engine = QueryEngine::new(vec![file_candidate(
            "file:a",
            "notes.md",
            "/Users/u/Desktop/notes.md",
            now - 86_400,
        )]);
        let filter = FileFilter {
            terms: "added".into(),
            locations: vec!["desktop".into()],
            ..Default::default()
        };
        let outcome = engine.search_files(&filter, 10);
        assert_eq!(outcome.results.len(), 1);
        assert_eq!(outcome.results[0].title, "notes.md");
        assert_eq!(outcome.relaxation, Some(FileSearchRelaxation::DroppedTerms));
    }

    #[test]
    fn terms_only_recall_never_degrades_to_the_whole_index() {
        // "bitcoin price" reaches file recall as terms with nothing else.
        // Dropping them would leave no filter at all and return every file,
        // which reads as an answer and hides the real one.
        let now = 1_754_000_000;
        let engine = QueryEngine::new(vec![
            file_candidate("file:a", "clip.mp4", "/Users/u/Documents/clip.mp4", now),
            file_candidate("file:b", "notes.md", "/Users/u/Desktop/notes.md", now),
        ]);
        let filter = FileFilter {
            terms: "bitcoin price".into(),
            ..Default::default()
        };
        let outcome = engine.search_files(&filter, 10);
        assert!(outcome.results.is_empty());
        assert_eq!(outcome.relaxation, None);
    }

    #[test]
    fn window_widens_before_terms_drop() {
        // "resume pdf last week" where the resume is 10 days old: widening the
        // window (keeping the term) must win over dropping the term and
        // returning every recent pdf.
        let now = 1_754_000_000;
        let engine = QueryEngine::new(vec![
            file_candidate(
                "file:a",
                "resume.pdf",
                "/Users/u/Documents/resume.pdf",
                now - 10 * 86_400,
            ),
            file_candidate(
                "file:b",
                "other.pdf",
                "/Users/u/Documents/other.pdf",
                now - 2 * 86_400,
            ),
        ]);
        let filter = FileFilter {
            terms: "resume".into(),
            start: Some(now - 7 * 86_400),
            end: Some(now),
            ..Default::default()
        };
        let outcome = engine.search_files(&filter, 10);
        assert_eq!(outcome.results.len(), 1);
        assert_eq!(outcome.results[0].title, "resume.pdf");
        assert_eq!(
            outcome.relaxation,
            Some(FileSearchRelaxation::WidenedWindow)
        );
    }

    #[test]
    fn screenshot_category_does_not_filter_its_siblings() {
        // "screenshots and pdfs": the screenshot PATH heuristic must gate only
        // its own category, or every pdf without the word is dropped.
        let now = 1_754_000_000;
        let engine = QueryEngine::new(vec![
            file_candidate(
                "file:a",
                "Screenshot 1.png",
                "/Users/u/Desktop/Screenshot 1.png",
                now,
            ),
            file_candidate("file:b", "invoice.pdf", "/Users/u/Desktop/invoice.pdf", now),
            file_candidate("file:c", "holiday.png", "/Users/u/Desktop/holiday.png", now),
        ]);
        let filter = FileFilter {
            categories: vec!["screenshot".into(), "pdf".into()],
            ..Default::default()
        };
        let titles: Vec<String> = engine
            .search_files(&filter, 10)
            .results
            .into_iter()
            .map(|r| r.title)
            .collect();
        assert!(titles.contains(&"invoice.pdf".to_string()), "{titles:?}");
        assert!(
            titles.contains(&"Screenshot 1.png".to_string()),
            "{titles:?}"
        );
        // A plain image is neither a screenshot nor a pdf.
        assert!(!titles.contains(&"holiday.png".to_string()), "{titles:?}");
    }

    #[test]
    fn screenshot_alone_still_requires_the_path() {
        let now = 1_754_000_000;
        let engine = QueryEngine::new(vec![
            file_candidate(
                "file:a",
                "Screenshot 1.png",
                "/Users/u/Desktop/Screenshot 1.png",
                now,
            ),
            file_candidate("file:c", "holiday.png", "/Users/u/Desktop/holiday.png", now),
        ]);
        let filter = FileFilter {
            categories: vec!["screenshot".into()],
            ..Default::default()
        };
        let outcome = engine.search_files(&filter, 10);
        assert_eq!(outcome.results.len(), 1);
        assert_eq!(outcome.results[0].title, "Screenshot 1.png");
    }

    #[test]
    fn bootstrap_scope_all_includes_every_source() {
        let s = BootstrapScope::ALL;
        assert!(s.is_all());
        assert!(!s.is_empty());
        assert!(s.apps && s.files && s.settings && s.sources);
    }

    #[test]
    fn bootstrap_scope_apps_only_picks_only_app_prefix() {
        let prefixes = BootstrapScope::APPS_ONLY.id_prefixes();
        assert_eq!(prefixes, vec![CandidateIdKind::PREFIX_APP]);
        assert!(!BootstrapScope::APPS_ONLY.is_all());
        assert!(!BootstrapScope::APPS_ONLY.is_empty());
    }

    #[test]
    fn bootstrap_scope_files_only_includes_file_and_folder_prefixes() {
        // The files walker emits both `file:` and `folder:` candidates, so a
        // scoped delete for files must sweep both - otherwise renamed/removed
        // folders linger forever.
        let prefixes = BootstrapScope::FILES_ONLY.id_prefixes();
        assert_eq!(
            prefixes,
            vec![CandidateIdKind::PREFIX_FILE, CandidateIdKind::PREFIX_FOLDER]
        );
    }

    #[test]
    fn bootstrap_scope_all_yields_every_prefix() {
        let prefixes = BootstrapScope::ALL.id_prefixes();
        assert_eq!(
            prefixes,
            vec![
                CandidateIdKind::PREFIX_APP,
                CandidateIdKind::PREFIX_FILE,
                CandidateIdKind::PREFIX_FOLDER,
                CandidateIdKind::PREFIX_SETTING,
                CandidateIdKind::PREFIX_SOURCE,
            ]
        );
    }

    #[test]
    fn bootstrap_scope_sources_only_sweeps_only_source_rows() {
        // A source refresh must never prune an app, a file, or a setting, and a
        // file refresh must never prune a source's rows.
        assert_eq!(
            BootstrapScope::SOURCES_ONLY.id_prefixes(),
            vec![CandidateIdKind::PREFIX_SOURCE]
        );
        assert!(
            !BootstrapScope::FILES_ONLY
                .id_prefixes()
                .contains(&CandidateIdKind::PREFIX_SOURCE)
        );
    }

    #[test]
    fn bootstrap_scope_empty_is_detectable() {
        let s = BootstrapScope {
            apps: false,
            files: false,
            settings: false,
            sources: false,
        };
        assert!(s.is_empty());
        assert!(!s.is_all());
        assert!(s.id_prefixes().is_empty());
    }

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
