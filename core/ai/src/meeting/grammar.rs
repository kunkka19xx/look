//! Reading "join" out of a typed line. Tier 1: a fixed grammar, no model,
//! cheap enough for every keystroke.

/// The verb that opens a join request. Deliberately the only one: "open" and
/// "start" are already spoken for by apps and files.
const JOIN_VERB: &str = "join";

/// Words that carry no meeting name of their own, so "join my next meeting"
/// means "whatever is next". Everything else after the verb is read as the
/// name of a meeting. Provider names count as filler ("join zoom" is not
/// hunting for an event titled Zoom); ordinary words like "standup" do NOT,
/// because that is exactly how people name their meetings.
const JOIN_FILLER: &[&str] = &[
    "meeting", "meetings", "call", "my", "the", "a", "next", "now", "up", "in", "current", "zoom",
    "teams", "meet", "webex", "please",
];

/// A parsed join request: the words after `join` that were not filler, if any.
/// `None` name means "whatever is next".
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinRequest {
    /// Absent from the JSON when there is no name, so a bare "join" crosses the
    /// boundary as `{}` rather than a null the shell has to special-case.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// The join request in the typed text, or None when this is an ordinary search.
///
/// Tier 1: a fixed grammar, no model, cheap enough for every keystroke. Only
/// the leading verb is fixed; the rest is either filler ("my next meeting") or
/// the name of the meeting to join ("join standup"). Naming one is the shape
/// people reach for first, and it is safe here because a name that matches no
/// meeting produces no row at all, so "join two pdfs" still falls through to
/// file search.
pub fn join_query(input: &str) -> Option<JoinRequest> {
    let lower = input.trim().to_lowercase();
    let mut words = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty());

    if words.next() != Some(JOIN_VERB) {
        return None;
    }
    let name: Vec<&str> = words.filter(|word| !JOIN_FILLER.contains(word)).collect();
    Some(JoinRequest {
        name: if name.is_empty() {
            None
        } else {
            Some(name.join(" "))
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_phrasings_are_recognised() {
        for phrasing in [
            "join",
            "Join",
            "  join  ",
            "join meeting",
            "join my meeting",
            "join the meeting",
            "join my next meeting",
            "join next call",
            "join now",
            "join zoom",
            "join teams meeting",
        ] {
            assert_eq!(
                join_query(phrasing),
                Some(JoinRequest { name: None }),
                "expected a nameless join query: {phrasing}"
            );
        }
    }
    #[test]
    fn the_words_after_join_name_a_meeting() {
        for (phrasing, expected) in [
            ("join testing", "testing"),
            ("Join Testing", "testing"),
            ("join the design review", "design review"),
            ("join the standup", "standup"),
            ("join my standup with sarah", "standup with sarah"),
            // Punctuation splits like any other separator, and the pieces are
            // matched against the title independently, so `1:1` still finds it.
            ("join 1:1", "1 1"),
        ] {
            assert_eq!(
                join_query(phrasing),
                Some(JoinRequest {
                    name: Some(expected.to_string())
                }),
                "for {phrasing}"
            );
        }
    }
    #[test]
    fn a_search_that_does_not_start_with_join_is_never_a_join_query() {
        // A NAME is allowed after the verb now, so the guard is the verb itself
        // plus the fact that a name matching no meeting yields no row at all.
        for phrasing in ["joins", "joint account", "adjoin", "rejoin meeting", ""] {
            assert_eq!(
                join_query(phrasing),
                None,
                "expected a plain search: {phrasing}"
            );
        }
    }
}
