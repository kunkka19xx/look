//! Ranks stored items against the user's phrase for the mutate tools, and
//! applies the safety gate: never mutate on an uncertain match. Deterministic
//! tiers (exact > contains > word-prefix) with the shared `core/matching`
//! fuzzy scorer as the weak tier. Auto-plan requires a STRONG score AND a clear
//! lead; a lone weak hit becomes a one-item choice instead of silently mutating
//! the wrong thing.
//!
//! Built to stay flat as the candidate set grows (apps, files, works). Per
//! resolve the query is prepared once, and each candidate is rejected in O(1)
//! by an a-z presence mask before any expensive scoring runs; only survivors
//! are scored, and only a bounded top-K is kept.

/// Bits set for the ascii letters present in an already-lowercased string. A
/// subsequence/contains match needs every query letter to appear in the title,
/// so `qmask & !tmask != 0` rejects a candidate with no string scan.
fn char_mask(lower: &str) -> u32 {
    let mut mask = 0u32;
    for ch in lower.bytes() {
        if ch.is_ascii_lowercase() {
            mask |= 1 << (ch - b'a');
        }
    }
    mask
}

/// Tier score for one already-lowercased title against a prepared query. Strong
/// tiers (>=300) can auto-plan; the graded fuzzy tier stays in the weak band
/// (<300) so it ranks weak hits without ever crossing the auto-plan threshold.
fn score_prepared(query_lower: &str, query_words: &[&str], title_lower: &str) -> Option<i64> {
    if title_lower == query_lower {
        return Some(1000);
    }
    if title_lower.contains(query_lower) {
        return Some(500);
    }
    if !query_words.is_empty()
        && query_words
            .iter()
            .all(|qw| title_lower.split_whitespace().any(|tw| tw.starts_with(qw)))
    {
        return Some(300);
    }
    // Graded, not flat: the real score orders weak hits (better Choice lists),
    // compressed into 100..=299 so the gate semantics are unchanged.
    look_matching::fuzzy_score(query_lower, title_lower)
        .map(|raw| 100 + (raw / 10).clamp(0, 199))
}

/// Single-pair scorer. `resolve` uses the prepared path; this stays for callers
/// and tests that score one pair.
pub fn score(query: &str, title: &str) -> Option<i64> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return None;
    }
    let words: Vec<&str> = q.split_whitespace().collect();
    score_prepared(&q, &words, &title.to_lowercase())
}

pub enum Outcome<T> {
    One(T),
    Several(Vec<T>),
    None,
}

/// Top-K needs only the top two for the lead check plus five for a Choice list.
const KEEP: usize = 6;

/// Inserts into a score-descending vec capped at `KEEP`, so we never sort the
/// whole survivor set.
fn insert_top(top: &mut Vec<(i64, usize)>, item: (i64, usize)) {
    let pos = top.partition_point(|&(s, _)| s >= item.0);
    top.insert(pos, item);
    if top.len() > KEEP {
        top.pop();
    }
}

/// A candidate with its match key (letter mask + lowercased title) precomputed.
/// Built once when the target list loads so per-query resolves skip the rebuild.
pub struct Indexed<T> {
    mask: u32,
    title_lower: String,
    item: T,
}

impl<T> Indexed<T> {
    pub fn item(&self) -> &T {
        &self.item
    }
}

/// Precompute the match key for one candidate.
pub fn index<T>(item: T, title: &str) -> Indexed<T> {
    let title_lower = title.to_lowercase();
    Indexed { mask: char_mask(&title_lower), title_lower, item }
}

/// Shared ranking core: prefilter by mask, score survivors, keep a bounded
/// top-K. `items` yields `(letter_mask, lowercased_title)` in candidate order.
fn rank<'a>(
    query_lower: &str,
    query_mask: u32,
    query_words: &[&str],
    items: impl Iterator<Item = (u32, &'a str)>,
) -> (Vec<(i64, usize)>, usize) {
    let mut top: Vec<(i64, usize)> = Vec::with_capacity(KEEP + 1);
    let mut match_count = 0usize;
    for (idx, (title_mask, title_lower)) in items.enumerate() {
        // O(1) reject: query needs a letter this title lacks -> no match possible.
        if query_mask & !title_mask != 0 {
            continue;
        }
        if let Some(s) = score_prepared(query_lower, query_words, title_lower) {
            match_count += 1;
            insert_top(&mut top, (s, idx));
        }
    }
    (top, match_count)
}

/// Applies the safety gate to a ranked top-K. `pick` maps a winning index to an
/// owned candidate.
fn finish<T>(top: Vec<(i64, usize)>, match_count: usize, pick: impl Fn(usize) -> T) -> Outcome<T> {
    let Some(&(best, best_idx)) = top.first() else {
        return Outcome::None;
    };
    let clear_lead = match_count == 1 || best > top[1].0;
    if best >= 300 && clear_lead {
        return Outcome::One(pick(best_idx));
    }
    Outcome::Several(top.iter().take(5).map(|&(_, i)| pick(i)).collect())
}

pub fn resolve<T: Clone>(candidates: &[T], query: &str, title: impl Fn(&T) -> String) -> Outcome<T> {
    let query_lower = query.trim().to_lowercase();
    if query_lower.is_empty() {
        return Outcome::None;
    }
    let query_mask = char_mask(&query_lower);
    let query_words: Vec<&str> = query_lower.split_whitespace().collect();
    // Own the lowercased titles so the ranking iterator can borrow them.
    let lowered: Vec<String> = candidates.iter().map(|c| title(c).to_lowercase()).collect();
    let (top, match_count) = rank(
        &query_lower,
        query_mask,
        &query_words,
        lowered.iter().map(|s| (char_mask(s), s.as_str())),
    );
    finish(top, match_count, |i| candidates[i].clone())
}

/// Like `resolve`, but over candidates whose match keys were precomputed with
/// `index`, so nothing is lowercased or masked per query.
pub fn resolve_indexed<T: Clone>(candidates: &[Indexed<T>], query: &str) -> Outcome<T> {
    let query_lower = query.trim().to_lowercase();
    if query_lower.is_empty() {
        return Outcome::None;
    }
    let query_mask = char_mask(&query_lower);
    let query_words: Vec<&str> = query_lower.split_whitespace().collect();
    let (top, match_count) = rank(
        &query_lower,
        query_mask,
        &query_words,
        candidates.iter().map(|c| (c.mask, c.title_lower.as_str())),
    );
    finish(top, match_count, |i| candidates[i].item.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiers() {
        assert_eq!(score("dentist", "Dentist"), Some(1000));
        assert_eq!(score("sync", "Team Sync"), Some(500));
        assert_eq!(score("te sy", "Team Sync"), Some(300));
        assert_eq!(score("zebra", "Team Sync"), None);
    }

    #[test]
    fn fuzzy_tier_is_graded_but_stays_weak() {
        // Subsequence hit: below the auto-plan threshold, but graded (not flat).
        let s = score("tmsy", "Team Sync").unwrap();
        assert!(s >= 100 && s < 300, "fuzzy score {s} must stay in the weak band");
    }

    #[test]
    fn mask_prefilter_agrees_with_scoring() {
        // A candidate missing a query letter must be a non-match either way.
        assert_eq!(score("qq", "Team Sync"), None);
        let candidates = vec!["Team Sync".to_string()];
        assert!(matches!(resolve(&candidates, "qq", |c| c.clone()), Outcome::None));
    }

    #[test]
    fn lone_weak_match_is_a_choice() {
        // "it" weakly matches "DentIsT" via fuzzy; the gate must not auto-plan.
        let candidates = vec!["Dentist".to_string()];
        match resolve(&candidates, "it", |c| c.clone()) {
            Outcome::Several(list) => assert_eq!(list.len(), 1),
            _ => panic!("expected Several"),
        }
    }

    #[test]
    fn strong_lead_auto_plans() {
        let candidates = vec!["Dentist".to_string(), "Team Sync".to_string()];
        match resolve(&candidates, "dentist", |c| c.clone()) {
            Outcome::One(winner) => assert_eq!(winner, "Dentist"),
            _ => panic!("expected One"),
        }
    }

    #[test]
    fn near_tie_is_a_choice() {
        let candidates = vec!["Team Sync".to_string(), "Team Standup".to_string()];
        match resolve(&candidates, "team", |c| c.clone()) {
            Outcome::Several(list) => assert_eq!(list.len(), 2),
            _ => panic!("expected Several"),
        }
    }

    #[test]
    fn indexed_matches_inline() {
        let titles = ["Dentist", "Team Sync", "Team Standup"];
        let indexed: Vec<_> = titles.iter().map(|t| index(t.to_string(), t)).collect();
        for query in ["dentist", "team", "it", "zzz"] {
            let inline = resolve(&titles.map(String::from), query, |c| c.clone());
            let pre = resolve_indexed(&indexed, query);
            let tag = |o: &Outcome<String>| match o {
                Outcome::One(w) => format!("one:{w}"),
                Outcome::Several(l) => format!("several:{}", l.join(",")),
                Outcome::None => "none".into(),
            };
            assert_eq!(tag(&inline), tag(&pre), "mismatch for {query:?}");
        }
    }
}
