use crate::config::*;
use look_indexing::CandidateIdKind;
use look_indexing::{Candidate, CandidateKind};
use std::cmp::Ordering;
use std::collections::BinaryHeap;

const SETTINGS_SUBTITLE_PREFIX: &str = "System Settings";
const BROWSE_USAGE_LOG_SCALE: f64 = 60.0;

const RECENT_LAST_HOUR_BOOST: i64 = 140;
const RECENT_TODAY_BOOST: i64 = 90;
const RECENT_THIS_WEEK_BOOST: i64 = 40;
const RECENT_THIS_MONTH_BOOST: i64 = 12;

const SETTINGS_ON_NON_SETTINGS_QUERY_PENALTY: i64 = -180;

pub(crate) fn contains_match_score(
    query: &str,
    title: &str,
    subtitle: Option<&str>,
) -> Option<i64> {
    if title.contains(query) {
        return Some(SCORE_TITLE_CONTAINS);
    }

    if let Some(sub) = subtitle
        && sub.contains(query)
    {
        return Some(SCORE_SUBTITLE_CONTAINS);
    }

    let mut has_terms = false;
    let all_match = query.split_whitespace().all(|t| {
        has_terms = true;
        title.contains(t) || subtitle.is_some_and(|sub| sub.contains(t))
    });

    if has_terms && all_match {
        return Some(SCORE_TOKEN_ALL_MATCH);
    }

    None
}

pub(crate) fn path_match_score(query: &str, path: &str) -> Option<i64> {
    if !query.contains('/') {
        return None;
    }

    let normalized = query.trim().trim_matches('/');
    if normalized.is_empty() {
        return None;
    }

    if path.contains(normalized) {
        return Some(1_350);
    }

    let tokens: Vec<&str> = normalized
        .split('/')
        .filter(|token| !token.is_empty())
        .collect();
    if tokens.len() < 2 {
        return None;
    }

    let mut cursor = 0usize;
    let mut total_gap = 0usize;
    for token in tokens {
        let remaining = &path[cursor..];
        let found_at = remaining.find(token)?;
        total_gap += found_at;
        cursor += found_at + token.len();
    }

    let penalty = (total_gap as i64).min(250);
    Some(1_050 - penalty)
}

pub(crate) fn kind_bias(candidate: &Candidate) -> i64 {
    match candidate.kind {
        CandidateKind::App => BIAS_APP,
        CandidateKind::Folder => BIAS_FOLDER,
        CandidateKind::File => BIAS_FILE,
    }
}

pub(crate) fn query_kind_penalty_with_settings_flag(
    settings_query: bool,
    candidate: &Candidate,
) -> i64 {
    // Callers can precompute `settings_query` once per search query to avoid
    // repeating hint detection for every candidate in the scoring loop.
    if settings_query {
        match candidate.kind {
            CandidateKind::App => {
                if is_system_settings_candidate(candidate) {
                    BIAS_SETTINGS_MATCH
                } else {
                    BIAS_APP_ON_SETTINGS_QUERY
                }
            }
            CandidateKind::Folder | CandidateKind::File => BIAS_NON_APP_ON_SETTINGS_QUERY,
        }
    } else if is_system_settings_candidate(candidate) {
        SETTINGS_ON_NON_SETTINGS_QUERY_PENALTY
    } else {
        0
    }
}

pub(crate) fn looks_like_settings_query(query: &str) -> bool {
    // `query` comes from normalized input in the search path, so we avoid
    // allocating a lowercased copy here and match hints directly.
    if QUERY_SETTINGS_HINTS.iter().any(|hint| query.contains(hint)) {
        return true;
    }

    for word in query.split_whitespace() {
        if word.len() >= 3
            && QUERY_SETTINGS_HINTS
                .iter()
                .any(|hint| hint.starts_with(word))
        {
            return true;
        }
    }
    false
}

pub(crate) fn is_system_settings_candidate(candidate: &Candidate) -> bool {
    candidate.id.starts_with(CandidateIdKind::PREFIX_SETTING)
        || candidate
            .subtitle
            .as_deref()
            .unwrap_or("")
            .contains(SETTINGS_SUBTITLE_PREFIX)
}

pub(crate) fn path_depth_penalty(candidate: &Candidate) -> i64 {
    match candidate.kind {
        CandidateKind::App => 0,
        CandidateKind::File | CandidateKind::Folder => {
            let depth = candidate
                .path
                .split('/')
                .filter(|part| !part.is_empty())
                .count() as i64;
            -(depth / 2)
        }
    }
}

pub(crate) fn default_browse_score(candidate: &Candidate, now_unix_s: i64) -> i64 {
    let kind_boost = match candidate.kind {
        CandidateKind::App => 600,
        CandidateKind::Folder => 120,
        CandidateKind::File => 0,
    };

    let frequency = ((candidate.use_count as f64 + 1.0).log2() * BROWSE_USAGE_LOG_SCALE) as i64;
    let recency = candidate
        .last_used_at_unix_s
        .map(|last| {
            let age_s = (now_unix_s - last).max(0);
            let age_hours = age_s / 3600;
            browse_recency_boost(age_hours)
        })
        .unwrap_or(0);

    kind_boost + frequency + recency
}

fn browse_recency_boost(age_hours: i64) -> i64 {
    match age_hours {
        0 => RECENT_LAST_HOUR_BOOST,
        1..=24 => RECENT_TODAY_BOOST - ((age_hours - 1) / 4),
        25..=168 => RECENT_THIS_WEEK_BOOST - ((age_hours - 25) / 12),
        169..=720 => RECENT_THIS_MONTH_BOOST - ((age_hours - 169) / 72),
        _ => 0,
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ScoredMatch<'a> {
    idx: u32,
    score: i64,
    // Borrowed title powers the BinaryHeap tiebreak without cloning the Candidate.
    // Lifetime is scoped to the candidate slice, which outlives the heap.
    title: &'a str,
}

impl PartialEq for ScoredMatch<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.score == other.score && self.title == other.title
    }
}

impl Eq for ScoredMatch<'_> {}

impl<'a> ScoredMatch<'a> {
    pub(crate) fn new(idx: u32, score: i64, title: &'a str) -> Self {
        Self { idx, score, title }
    }
}

impl Ord for ScoredMatch<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.score.cmp(&other.score) {
            Ordering::Less => Ordering::Greater,
            Ordering::Greater => Ordering::Less,
            Ordering::Equal => self.title.cmp(other.title),
        }
    }
}

impl PartialOrd for ScoredMatch<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub(crate) fn push_top_k<'a>(
    heap: &mut BinaryHeap<ScoredMatch<'a>>,
    item: ScoredMatch<'a>,
    limit: usize,
) {
    if heap.len() < limit {
        heap.push(item);
        return;
    }

    if let Some(worst) = heap.peek()
        && item < *worst
    {
        let _ = heap.pop();
        heap.push(item);
    }
}

pub(crate) fn finalize_top_k(heap: BinaryHeap<ScoredMatch<'_>>) -> Vec<(u32, i64)> {
    // Returns (candidate_index, score) pairs sorted best-first. Callers materialize
    // Candidates by indexing into their candidate slice, so only the final top-K
    // are cloned — the heap itself never held a Candidate.
    let mut out: Vec<(u32, i64, &str)> = heap
        .into_iter()
        .map(|entry| (entry.idx, entry.score, entry.title))
        .collect();
    out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.2.cmp(b.2)));
    out.into_iter()
        .map(|(idx, score, _)| (idx, score))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(title: &str, path: &str) -> Candidate {
        Candidate::new(
            &format!("app.{}", title.to_lowercase()),
            CandidateKind::App,
            title,
            path,
        )
    }

    fn file(title: &str, path: &str) -> Candidate {
        Candidate::new(
            &format!("file.{}", title.to_lowercase()),
            CandidateKind::File,
            title,
            path,
        )
    }

    fn folder(title: &str, path: &str) -> Candidate {
        Candidate::new(
            &format!("folder.{}", title.to_lowercase()),
            CandidateKind::Folder,
            title,
            path,
        )
    }

    #[test]
    fn contains_match_title_scores_higher_than_subtitle() {
        let title_score = contains_match_score("safari", "safari browser", None).unwrap();
        let subtitle_score =
            contains_match_score("safari", "other", Some("safari browser")).unwrap();
        assert!(title_score > subtitle_score);
    }

    #[test]
    fn contains_match_returns_none_when_no_match() {
        assert!(contains_match_score("xyz", "safari", Some("browser")).is_none());
    }

    #[test]
    fn contains_match_multi_token_all_present() {
        let score = contains_match_score("visual code", "visual studio code", None);
        assert!(score.is_some());
    }

    #[test]
    fn contains_match_multi_token_partial_returns_none() {
        assert!(contains_match_score("visual xyz", "visual studio code", None).is_none());
    }

    #[test]
    fn path_match_requires_slash() {
        assert!(path_match_score("safari", "/Applications/Safari.app").is_none());
    }

    #[test]
    fn path_match_exact_substring() {
        let score = path_match_score("git/books-pc", "/Users/test/git/books-pc/README.md");
        assert!(score.is_some());
        assert!(score.unwrap() > 1_000);
    }

    #[test]
    fn path_match_multi_segment_fuzzy() {
        let score = path_match_score("Users/Documents", "/Users/test/Documents/notes.txt");
        assert!(score.is_some());
    }

    #[test]
    fn path_match_single_segment_returns_none() {
        assert!(path_match_score("test/", "/some/path").is_none());
    }

    #[test]
    fn kind_bias_apps_higher_than_files() {
        let a = app("Safari", "/Applications/Safari.app");
        let f = file("notes.txt", "/Users/test/notes.txt");
        assert!(kind_bias(&a) > kind_bias(&f));
    }

    #[test]
    fn path_depth_penalty_apps_exempt() {
        let a = app("Safari", "/Applications/Deeply/Nested/Safari.app");
        assert_eq!(path_depth_penalty(&a), 0);
    }

    #[test]
    fn path_depth_penalty_increases_with_depth() {
        let shallow = file("a.txt", "/Users/a.txt");
        let deep = file("b.txt", "/Users/test/Documents/nested/deep/b.txt");
        assert!(path_depth_penalty(&shallow) > path_depth_penalty(&deep));
    }

    #[test]
    fn default_browse_score_prefers_frequent_apps() {
        let mut frequent = app("Safari", "/Applications/Safari.app");
        frequent.use_count = 50;
        frequent.last_used_at_unix_s = Some(1_700_000_000);

        let unused = app("Chess", "/Applications/Chess.app");

        let now = 1_700_000_100;
        assert!(default_browse_score(&frequent, now) > default_browse_score(&unused, now));
    }

    #[test]
    fn push_top_k_respects_limit() {
        let titles: Vec<String> = (0..10).map(|i| format!("App{i}")).collect();
        let mut heap = BinaryHeap::new();
        for (i, title) in titles.iter().enumerate() {
            push_top_k(
                &mut heap,
                ScoredMatch::new(i as u32, (i as i64) * 100, title.as_str()),
                3,
            );
        }
        assert_eq!(heap.len(), 3);

        let results = finalize_top_k(heap);
        assert!(results[0].1 >= results[1].1);
        assert!(results[1].1 >= results[2].1);
    }

    #[test]
    fn query_kind_penalty_settings_hints() {
        let settings_app = Candidate {
            id: "app:settings".into(),
            kind: CandidateKind::App,
            title: "System Settings".into(),
            subtitle: Some("System Settings".into()),
            path: "/System/Applications/System Settings.app".into(),
            use_count: 0,
            last_used_at_unix_s: None,
        };
        let regular_app = app("Safari", "/Applications/Safari.app");
        let folder = folder("network", "/Users/test/network");

        let settings_score = query_kind_penalty_with_settings_flag(true, &settings_app);
        let app_score = query_kind_penalty_with_settings_flag(true, &regular_app);
        let folder_score = query_kind_penalty_with_settings_flag(true, &folder);

        assert!(settings_score > app_score);
        assert!(app_score > folder_score);
    }

    #[test]
    fn query_kind_penalty_demotes_settings_for_non_settings_queries() {
        let settings_app = Candidate {
            id: "setting:network".into(),
            kind: CandidateKind::App,
            title: "Network".into(),
            subtitle: Some("System Settings network".into()),
            path: "x-apple.systempreferences:com.apple.preference.network".into(),
            use_count: 0,
            last_used_at_unix_s: None,
        };

        assert!(query_kind_penalty_with_settings_flag(false, &settings_app) < 0);
    }

    #[test]
    fn query_kind_penalty_precomputed_flag_matches_detected_query_kind() {
        let settings_app = Candidate {
            id: "setting:network".into(),
            kind: CandidateKind::App,
            title: "Network".into(),
            subtitle: Some("System Settings network".into()),
            path: "x-apple.systempreferences:com.apple.preference.network".into(),
            use_count: 0,
            last_used_at_unix_s: None,
        };
        let regular_app = app("Safari", "/Applications/Safari.app");
        let regular_file = file("notes.txt", "/Users/test/notes.txt");

        let settings_like = "network";
        let non_settings_like = "ingo";

        assert_eq!(
            query_kind_penalty_with_settings_flag(
                looks_like_settings_query(settings_like),
                &settings_app
            ),
            query_kind_penalty_with_settings_flag(true, &settings_app)
        );
        assert_eq!(
            query_kind_penalty_with_settings_flag(
                looks_like_settings_query(settings_like),
                &regular_app
            ),
            query_kind_penalty_with_settings_flag(true, &regular_app)
        );
        assert_eq!(
            query_kind_penalty_with_settings_flag(
                looks_like_settings_query(non_settings_like),
                &settings_app,
            ),
            query_kind_penalty_with_settings_flag(false, &settings_app)
        );
        assert_eq!(
            query_kind_penalty_with_settings_flag(
                looks_like_settings_query(non_settings_like),
                &regular_file,
            ),
            query_kind_penalty_with_settings_flag(false, &regular_file)
        );
    }

    #[test]
    fn looks_like_settings_query_accepts_short_prefixes() {
        assert!(looks_like_settings_query("sett"));
        assert!(looks_like_settings_query("priv"));
        assert!(!looks_like_settings_query("ingo"));
    }

    #[test]
    fn default_browse_score_recency_tiers_prefer_more_recent_candidates() {
        let now = 1_775_462_400;
        let mut candidate = app("Recent", "/Applications/Recent.app");
        candidate.use_count = 1;

        let mut last_hour = candidate.clone();
        last_hour.last_used_at_unix_s = Some(now - 10 * 60);

        let mut today = candidate.clone();
        today.last_used_at_unix_s = Some(now - 6 * 3600);

        let mut week = candidate.clone();
        week.last_used_at_unix_s = Some(now - 3 * 24 * 3600);

        let mut old = candidate;
        old.last_used_at_unix_s = Some(now - 60 * 24 * 3600);

        assert!(default_browse_score(&last_hour, now) > default_browse_score(&today, now));
        assert!(default_browse_score(&today, now) > default_browse_score(&week, now));
        assert!(default_browse_score(&week, now) > default_browse_score(&old, now));
    }
}
