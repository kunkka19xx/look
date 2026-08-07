//! Ranks stored items against the user's phrase for the mutate tools, and
//! applies the safety gate: never mutate on an uncertain match. Deterministic
//! tiers (exact > contains > word-prefix) with the shared `core/matching`
//! fuzzy scorer as the weak tier (an upgrade over the old subsequence check).
//! Auto-plan requires a STRONG score AND a clear lead; a lone weak hit becomes
//! a one-item choice instead of silently mutating the wrong thing.

pub fn score(query: &str, title: &str) -> Option<i64> {
    let q = query.trim().to_lowercase();
    let t = title.to_lowercase();
    if q.is_empty() {
        return None;
    }
    if t == q {
        return Some(1000);
    }
    if t.contains(&q) {
        return Some(500);
    }
    let query_words: Vec<&str> = q.split_whitespace().collect();
    let title_words: Vec<&str> = t.split_whitespace().collect();
    if !query_words.is_empty()
        && query_words
            .iter()
            .all(|qw| title_words.iter().any(|tw| tw.starts_with(qw)))
    {
        return Some(300);
    }
    look_matching::fuzzy_score(&q, &t).map(|_| 100)
}

pub enum Outcome<T> {
    One(T),
    Several(Vec<T>),
    None,
}

pub fn resolve<T: Clone>(candidates: &[T], query: &str, title: impl Fn(&T) -> String) -> Outcome<T> {
    let mut scored: Vec<(T, i64)> = candidates
        .iter()
        .filter_map(|c| score(query, &title(c)).map(|s| (c.clone(), s)))
        .collect();
    scored.sort_by(|a, b| b.1.cmp(&a.1));
    let Some(top) = scored.first() else {
        return Outcome::None;
    };
    if top.1 >= 300 && (scored.len() == 1 || top.1 > scored[1].1) {
        return Outcome::One(top.0.clone());
    }
    Outcome::Several(scored.into_iter().take(5).map(|(c, _)| c).collect())
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
}
