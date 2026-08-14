//! "it", "that", "this event", "the last one": phrases that refer to something
//! from the conversation rather than naming it. Resolved by the shell against
//! the session's recent targets (last action + last listing) before any store
//! matching.

const PRONOUNS: [&str; 3] = ["it", "that", "this"];
const NOUNS: [&str; 6] = ["event", "meeting", "appointment", "reminder", "task", "one"];

pub fn is_referent(phrase: &str) -> bool {
    let lower = phrase.to_lowercase();
    let mut words: Vec<&str> = lower
        .split(|c: char| !c.is_alphabetic())
        .filter(|w| !w.is_empty())
        .collect();
    if words.first() == Some(&"the") {
        words.remove(0);
    }
    if words == ["last", "one"] {
        return true;
    }
    match words.as_slice() {
        [only] => PRONOUNS.contains(only),
        [first, second] => PRONOUNS.contains(first) && NOUNS.contains(second),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::is_referent;

    #[test]
    fn referent_phrases() {
        assert!(is_referent("it"));
        assert!(is_referent("this event"));
        assert!(is_referent("that meeting"));
        assert!(is_referent("the last one"));
        assert!(!is_referent("dentist"));
        assert!(!is_referent("this dentist visit"));
        assert!(!is_referent("the meeting")); // names, not refers
        assert!(!is_referent(""));
    }
}
