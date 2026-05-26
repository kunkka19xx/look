pub struct PreparedQuery<'a> {
    raw: &'a str,
    chars: Vec<char>,
}

const MAX_DP_QUERY_LEN: usize = 64;
const MATCH_BASE: i64 = 10;
const WORD_BOUNDARY_BONUS: i64 = 8;
const LEADING_BONUS_MAX_INDEX: usize = 5;
const LEADING_BONUS_STEP: i64 = 2;
const CONSECUTIVE_BONUS_STEP: i64 = 6;
const MAX_GAP_PENALTY: i64 = 20;

impl PreparedQuery<'_> {
    fn raw(&self) -> &str {
        self.raw
    }

    pub fn len(&self) -> usize {
        self.chars.len()
    }

    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }
}

#[inline]
pub fn prepare_query(query: &str) -> PreparedQuery<'_> {
    PreparedQuery {
        raw: query,
        chars: query.chars().collect(),
    }
}

pub fn fuzzy_score(query: &str, title: &str) -> Option<i64> {
    let prepared = prepare_query(query);
    fuzzy_score_prepared(&prepared, title)
}

#[inline]
pub fn fuzzy_score_prepared(query: &PreparedQuery<'_>, title: &str) -> Option<i64> {
    let q = query.raw();
    let t = title;

    if t == q {
        return Some(2_000);
    }

    if t.starts_with(q) {
        return Some(1_500 - (t.len() as i64 - q.len() as i64).max(0));
    }

    // Bounded DP gives better alignment quality for normal launcher queries.
    // Very long queries fall back to greedy to keep worst-case CPU predictable.
    if query.len() > MAX_DP_QUERY_LEN {
        return greedy_subsequence_score(query, t);
    }

    fuzzy_score_dp_prepared(query, t)
}

fn greedy_subsequence_score(query: &PreparedQuery<'_>, title: &str) -> Option<i64> {
    let mut qi = 0usize;
    let mut score = 0i64;

    for ch in title.chars() {
        if qi < query.len() && ch == query.chars[qi] {
            qi += 1;
            score += MATCH_BASE;
        }
    }

    if qi == query.len() { Some(score) } else { None }
}

#[derive(Clone, Copy)]
struct MatchState {
    score: i64,
    last_match_idx: usize,
    consecutive: i64,
}

fn fuzzy_score_dp_prepared(query: &PreparedQuery<'_>, title: &str) -> Option<i64> {
    let q_len = query.len();
    if q_len == 0 {
        return Some(0);
    }

    // 1D DP where dp[i] stores the best way to match query[0..=i]
    // up to the current title position.
    let mut dp = [None::<MatchState>; MAX_DP_QUERY_LEN];
    let mut prev_char: Option<char> = None;

    for (ti, ch) in title.chars().enumerate() {
        let is_word_boundary = ti == 0 || matches!(prev_char, Some(' ' | '-' | '_' | '/' | '.'));
        let leading_bonus = if ti < LEADING_BONUS_MAX_INDEX {
            (LEADING_BONUS_MAX_INDEX - ti) as i64 * LEADING_BONUS_STEP
        } else {
            0
        };
        let position_bonus = if is_word_boundary {
            WORD_BOUNDARY_BONUS
        } else {
            0
        } + leading_bonus;

        // Iterate backwards so current character updates do not affect states
        // needed by larger qi in the same title position.
        for qi in (0..q_len).rev() {
            if ch != query.chars[qi] {
                continue;
            }

            let (new_score, new_consecutive) = if qi == 0 {
                (MATCH_BASE + position_bonus, 0)
            } else if let Some(prev) = dp[qi - 1] {
                let gap = (ti - prev.last_match_idx - 1) as i64;
                if gap == 0 {
                    let consecutive = prev.consecutive + 1;
                    (
                        prev.score
                            + MATCH_BASE
                            + position_bonus
                            + (consecutive * CONSECUTIVE_BONUS_STEP),
                        consecutive,
                    )
                } else {
                    (
                        prev.score + MATCH_BASE + position_bonus - gap.min(MAX_GAP_PENALTY),
                        0,
                    )
                }
            } else {
                continue;
            };

            if dp[qi].is_none_or(|existing| new_score > existing.score) {
                dp[qi] = Some(MatchState {
                    score: new_score,
                    last_match_idx: ti,
                    consecutive: new_consecutive,
                });
            }
        }

        prev_char = Some(ch);
    }

    dp[q_len - 1].map(|state| state.score.max(0))
}

#[inline]
pub fn fuzzy_quality_bonus_prepared(query: &PreparedQuery<'_>, title: &str) -> i64 {
    if query.is_empty() || title == query.raw() || title.starts_with(query.raw()) {
        return 0;
    }

    let mut qi = 0usize;
    let mut bonus = 0i64;
    let mut prev_match_idx: Option<usize> = None;
    let mut consecutive: i64 = 0;
    let mut prev_char: Option<char> = None;

    for (ti, ch) in title.chars().enumerate() {
        if qi >= query.len() {
            break;
        }

        if ch != query.chars[qi] {
            prev_char = Some(ch);
            continue;
        }

        qi += 1;

        if let Some(prev) = prev_match_idx {
            if prev + 1 == ti {
                consecutive += 1;
                bonus += consecutive * 8;
            } else {
                consecutive = 0;
                let gap = (ti - prev - 1) as i64;
                bonus -= gap.min(20);
            }
        }

        if ti == 0 || matches!(prev_char, Some(' ' | '-' | '_' | '/' | '.')) {
            bonus += 20;
        }

        if ti < 5 {
            bonus += (5 - ti as i64) * 3;
        }

        prev_match_idx = Some(ti);
        prev_char = Some(ch);
    }

    if qi == query.len() { bonus.max(0) } else { 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_match_returns_max_score() {
        assert_eq!(fuzzy_score("safari", "safari"), Some(2_000));
    }

    #[test]
    fn prefix_match_returns_high_score() {
        let score = fuzzy_score("saf", "safari").unwrap();
        assert!(score > 1_000);
        assert!(score < 2_000);
    }

    #[test]
    fn prefix_score_penalizes_longer_titles() {
        let short = fuzzy_score("saf", "safari").unwrap();
        let long = fuzzy_score("saf", "safari browser extended").unwrap();
        assert!(short > long);
    }

    #[test]
    fn subsequence_match_returns_positive_score() {
        let score = fuzzy_score("vsc", "visual studio code").unwrap();
        assert!(score > 0);
    }

    #[test]
    fn no_match_returns_none() {
        assert_eq!(fuzzy_score("xyz", "safari"), None);
    }

    #[test]
    fn empty_query_matches_everything() {
        let score = fuzzy_score("", "safari").unwrap();
        assert!(score > 0);
    }

    #[test]
    fn empty_title_only_matches_empty_query() {
        assert_eq!(fuzzy_score("", ""), Some(2_000));
        assert_eq!(fuzzy_score("a", ""), None);
    }

    #[test]
    fn single_char_query() {
        assert!(fuzzy_score("s", "safari").is_some());
        assert!(fuzzy_score("z", "safari").is_none());
    }

    #[test]
    fn score_hierarchy_is_consistent() {
        let exact = fuzzy_score("safari", "safari").unwrap();
        let prefix = fuzzy_score("saf", "safari").unwrap();
        let subseq = fuzzy_score("sri", "safari").unwrap();

        assert!(exact > prefix, "exact ({exact}) > prefix ({prefix})");
        assert!(
            prefix > subseq,
            "prefix ({prefix}) > subsequence ({subseq})"
        );
    }

    #[test]
    fn prepared_query_matches_raw_behavior() {
        let prepared = prepare_query("vsc");
        assert_eq!(
            fuzzy_score_prepared(&prepared, "visual studio code"),
            fuzzy_score("vsc", "visual studio code")
        );
    }

    #[test]
    fn quality_bonus_prefers_compact_word_boundaries() {
        let prepared = prepare_query("vsc");
        let tight = fuzzy_quality_bonus_prepared(&prepared, "vs code");
        let loose = fuzzy_quality_bonus_prepared(&prepared, "visual studio code");
        assert!(tight > loose);
    }

    #[test]
    fn dp_base_score_prefers_dense_clusters_over_sparse_greedy_paths() {
        let prepared = prepare_query("app");
        let sparse = fuzzy_score_prepared(&prepared, "a_partial_app.rs").unwrap();
        let dense = fuzzy_score_prepared(&prepared, "my_app.rs").unwrap();
        assert!(
            dense > sparse,
            "dense ({dense}) should be > sparse ({sparse})"
        );
    }
}
