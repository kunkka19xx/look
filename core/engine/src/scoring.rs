use crate::config::*;
use look_indexing::{Candidate, CandidateKind};
use std::cmp::Ordering;
use std::collections::BinaryHeap;

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

    let terms: Vec<&str> = query.split_whitespace().collect();
    if terms.is_empty() {
        return None;
    }

    if terms
        .iter()
        .all(|t| title.contains(t) || subtitle.is_some_and(|sub| sub.contains(t)))
    {
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

pub(crate) fn query_kind_penalty(query: &str, candidate: &Candidate) -> i64 {
    let looks_like_settings_query = QUERY_SETTINGS_HINTS
        .iter()
        .any(|token| query.contains(token));

    if looks_like_settings_query {
        match candidate.kind {
            CandidateKind::App => {
                if candidate
                    .subtitle
                    .as_deref()
                    .unwrap_or("")
                    .contains("System Settings")
                {
                    BIAS_SETTINGS_MATCH
                } else {
                    BIAS_APP_ON_SETTINGS_QUERY
                }
            }
            CandidateKind::Folder | CandidateKind::File => BIAS_NON_APP_ON_SETTINGS_QUERY,
        }
    } else {
        0
    }
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

    let frequency = (candidate.use_count as i64).saturating_mul(35);
    let recency = candidate
        .last_used_at_unix_s
        .map(|last| {
            let age_s = (now_unix_s - last).max(0);
            let age_hours = age_s / 3600;
            (2000 - (age_hours * 8)).max(0)
        })
        .unwrap_or(0);

    kind_boost + frequency + recency
}

#[derive(Clone, Debug)]
pub(crate) struct ScoredMatch {
    candidate: Candidate,
    score: i64,
    sort_title: String,
}

impl PartialEq for ScoredMatch {
    fn eq(&self, other: &Self) -> bool {
        self.score == other.score && self.sort_title == other.sort_title
    }
}

impl Eq for ScoredMatch {}

impl ScoredMatch {
    pub(crate) fn new(candidate: Candidate, score: i64) -> Self {
        let sort_title = candidate.title.to_lowercase();
        Self {
            candidate,
            score,
            sort_title,
        }
    }
}

impl Ord for ScoredMatch {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.score.cmp(&other.score) {
            Ordering::Less => Ordering::Greater,
            Ordering::Greater => Ordering::Less,
            Ordering::Equal => self.sort_title.cmp(&other.sort_title),
        }
    }
}

impl PartialOrd for ScoredMatch {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub(crate) fn push_top_k(heap: &mut BinaryHeap<ScoredMatch>, item: ScoredMatch, limit: usize) {
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

pub(crate) fn finalize_top_k(heap: BinaryHeap<ScoredMatch>) -> Vec<(Candidate, i64)> {
    let mut out: Vec<(Candidate, i64)> = heap
        .into_iter()
        .map(|entry| (entry.candidate, entry.score))
        .collect();
    out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.title.cmp(&b.0.title)));
    out
}
