use crate::QueryEngine;
use crate::config::*;
use crate::normalize::normalize_for_search;
use crate::query::ParsedQuery;
use crate::scoring::{
    ScoredMatch, contains_match_score, default_browse_score, finalize_top_k,
    is_system_settings_candidate, kind_bias, looks_like_settings_query, path_depth_penalty,
    path_match_score, push_top_k, query_kind_penalty,
};
use look_indexing::{Candidate, CandidateKind};
use look_matching::{fuzzy_quality_bonus_prepared, fuzzy_score_prepared, prepare_query};
use look_ranking::rank_score;
use regex::RegexBuilder;
use std::collections::BinaryHeap;
use std::time::{SystemTime, UNIX_EPOCH};

const RERANK_POOL_MULTIPLIER: usize = 4;
const RERANK_TOP_N: usize = 80;
const RERANK_MIN_QUERY_CHARS: usize = 3;
const REGEX_SIZE_LIMIT_BYTES: usize = 1024 * 1024;
const SCORE_ALIAS_TITLE_MATCH: i64 = 1_520;
const SCORE_ALIAS_SUBTITLE_MATCH: i64 = 1_260;

fn top_limit(mut ranked: Vec<(Candidate, i64)>, limit: usize) -> Vec<(Candidate, i64)> {
    ranked.truncate(limit);
    ranked
}

impl QueryEngine {
    fn has_term_boundary_match(haystack: &str, term: &str) -> bool {
        if term.is_empty() {
            return false;
        }

        for (start, _) in haystack.match_indices(term) {
            let end = start + term.len();
            let left_ok = haystack[..start]
                .chars()
                .next_back()
                .is_none_or(|ch| !ch.is_alphanumeric());
            let right_ok = haystack[end..]
                .chars()
                .next()
                .is_none_or(|ch| !ch.is_alphanumeric());
            if left_ok && right_ok {
                return true;
            }
        }

        false
    }
    fn alias_terms_for_query<'a>(
        &'a self,
        normalized_query: &str,
        kind_filter: Option<&CandidateKind>,
    ) -> Option<&'a Vec<String>> {
        if normalized_query.is_empty() {
            return None;
        }

        if let Some(kind) = kind_filter
            && *kind != CandidateKind::App
        {
            return None;
        }

        self.search_aliases.get(normalized_query)
    }

    fn alias_match_score(
        alias_terms: &[String],
        title_search: &str,
        subtitle_search: Option<&str>,
    ) -> Option<i64> {
        let mut best = None;
        for term in alias_terms {
            if Self::has_term_boundary_match(title_search, term) {
                best = Some(best.unwrap_or(0).max(SCORE_ALIAS_TITLE_MATCH));
            }

            if subtitle_search.is_some_and(|subtitle| Self::has_term_boundary_match(subtitle, term))
            {
                best = Some(best.unwrap_or(0).max(SCORE_ALIAS_SUBTITLE_MATCH));
            }
        }
        best
    }

    fn kind_matches(
        candidate: &crate::IndexedCandidate,
        kind_filter: Option<&CandidateKind>,
    ) -> bool {
        kind_filter.is_none_or(|kind| &candidate.candidate.kind == kind)
    }

    fn search_empty_query(
        &self,
        kind_filter: Option<&CandidateKind>,
        limit: usize,
    ) -> Vec<(Candidate, i64)> {
        let now_unix_s = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let mut top = BinaryHeap::new();
        for candidate in self
            .candidates
            .iter()
            .filter(|candidate| Self::kind_matches(candidate, kind_filter))
        {
            let score = default_browse_score(&candidate.candidate, now_unix_s);
            push_top_k(
                &mut top,
                ScoredMatch::new(candidate.candidate.clone(), score),
                limit,
            );
        }

        finalize_top_k(top)
    }

    fn search_regex_query(
        &self,
        raw_query: Option<&String>,
        kind_filter: Option<&CandidateKind>,
        limit: usize,
    ) -> Vec<(Candidate, i64)> {
        let Some(regex) = raw_query.and_then(|pattern| {
            RegexBuilder::new(pattern)
                .case_insensitive(true)
                .size_limit(REGEX_SIZE_LIMIT_BYTES)
                .build()
                .ok()
        }) else {
            return vec![];
        };

        let mut top = BinaryHeap::new();
        for candidate in self
            .candidates
            .iter()
            .filter(|candidate| Self::kind_matches(candidate, kind_filter))
        {
            let title_match = regex.is_match(&candidate.candidate.title);
            let path_match = regex.is_match(&candidate.candidate.path);
            let subtitle_match = candidate
                .candidate
                .subtitle
                .as_ref()
                .is_some_and(|subtitle| regex.is_match(subtitle));

            if !(title_match || path_match || subtitle_match) {
                continue;
            }

            let regex_score = match (title_match, path_match, subtitle_match) {
                (true, true, _) => SCORE_REGEX_TITLE_AND_PATH,
                (true, false, _) => SCORE_REGEX_TITLE_ONLY,
                (false, true, _) => SCORE_REGEX_PATH_ONLY,
                (false, false, true) => SCORE_REGEX_SUBTITLE_ONLY,
                _ => SCORE_REGEX_PATH_ONLY,
            };

            let final_score = regex_score
                + kind_bias(&candidate.candidate)
                + path_depth_penalty(&candidate.candidate);
            push_top_k(
                &mut top,
                ScoredMatch::new(candidate.candidate.clone(), final_score),
                limit,
            );
        }

        finalize_top_k(top)
    }

    fn search_text_query(
        &self,
        normalized_query: &str,
        kind_filter: Option<&CandidateKind>,
        limit: usize,
    ) -> Vec<(Candidate, i64)> {
        let prepared_query = prepare_query(normalized_query);
        let mut top = BinaryHeap::new();
        let has_path_hint = normalized_query.contains('/');
        let settings_query = looks_like_settings_query(normalized_query);
        let pool_limit = (limit.saturating_mul(RERANK_POOL_MULTIPLIER)).max(RERANK_TOP_N);
        let alias_terms = self.alias_terms_for_query(normalized_query, kind_filter);

        for candidate in self
            .candidates
            .iter()
            .filter(|candidate| Self::kind_matches(candidate, kind_filter))
        {
            // Use precomputed normalized strings from IndexedCandidate.
            // This avoids normalize_for_search allocations in the hot loop.
            let title_score = fuzzy_score_prepared(&prepared_query, &candidate.title_search);
            let subtitle_search =
                if !settings_query && is_system_settings_candidate(&candidate.candidate) {
                    None
                } else {
                    candidate.subtitle_search.as_deref()
                };
            let subtitle_score = subtitle_search
                .as_ref()
                .and_then(|subtitle| fuzzy_score_prepared(&prepared_query, subtitle))
                .map(|value| value / 2);
            let contains_score =
                contains_match_score(normalized_query, &candidate.title_search, subtitle_search);
            let path_score = if has_path_hint {
                path_match_score(normalized_query, &candidate.path_search)
            } else {
                None
            };
            let alias_subtitle_search = if is_system_settings_candidate(&candidate.candidate) {
                candidate.subtitle_search.as_deref()
            } else {
                subtitle_search
            };
            let alias_score = alias_terms.and_then(|terms| {
                if candidate.candidate.kind != CandidateKind::App {
                    return None;
                }
                Self::alias_match_score(terms, &candidate.title_search, alias_subtitle_search)
            });

            let base = [
                title_score,
                subtitle_score,
                contains_score,
                path_score,
                alias_score,
            ]
            .into_iter()
            .flatten()
            .max();

            let Some(base) = base else {
                continue;
            };

            let final_score = rank_score(
                base,
                normalized_query,
                &candidate.candidate,
                &candidate.title_search,
            ) + kind_bias(&candidate.candidate)
                + query_kind_penalty(normalized_query, &candidate.candidate)
                + path_depth_penalty(&candidate.candidate);
            push_top_k(
                &mut top,
                ScoredMatch::new(candidate.candidate.clone(), final_score),
                pool_limit,
            );
        }

        let mut ranked = finalize_top_k(top);

        if normalized_query.chars().count() < RERANK_MIN_QUERY_CHARS {
            return top_limit(ranked, limit);
        }

        // Two-stage retrieval:
        // 1) fast scorer over full candidate set
        // 2) quality rerank only on top-N candidates to keep latency predictable
        let rerank_count = ranked.len().min(RERANK_TOP_N);
        for entry in ranked.iter_mut().take(rerank_count) {
            let rerank_title = normalize_for_search(&entry.0.title);
            entry.1 += fuzzy_quality_bonus_prepared(&prepared_query, &rerank_title);
        }

        ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.title.cmp(&b.0.title)));
        top_limit(ranked, limit)
    }

    pub fn search_scored(&self, query: &str, limit: usize) -> Vec<(Candidate, i64)> {
        if limit == 0 {
            return vec![];
        }

        let parsed_query = ParsedQuery::from_input(query);
        let kind_filter = parsed_query.kind_filter.as_ref();
        if parsed_query.normalized_query.is_empty() && !parsed_query.is_regex {
            return self.search_empty_query(kind_filter, limit);
        }

        if parsed_query.is_regex {
            return self.search_regex_query(parsed_query.raw_query.as_ref(), kind_filter, limit);
        }

        self.search_text_query(&parsed_query.normalized_query, kind_filter, limit)
    }
}

#[cfg(test)]
mod tests {
    use super::QueryEngine;

    #[test]
    fn term_boundary_match_rejects_inner_substring() {
        assert!(!QueryEngine::has_term_boundary_match(
            "archive utility",
            "arc"
        ));
    }

    #[test]
    fn term_boundary_match_accepts_full_token() {
        assert!(QueryEngine::has_term_boundary_match("arc browser", "arc"));
        assert!(QueryEngine::has_term_boundary_match("arc-browser", "arc"));
    }
}
