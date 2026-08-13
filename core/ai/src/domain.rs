//! The deterministic domain prefilter: which slice of the tool vocabulary a
//! request could possibly want. Narrowing the planner prompt to one domain is
//! what lets the vocabulary grow past ten tools on a 7-8B local model. Fewer
//! candidates is fewer ways to be wrong, and each domain can carry its own
//! disambiguation rules without paying for them on every other request (a
//! single flat prompt cannot: rules added for one tool destabilize the rest).
//!
//! Conservative BY DESIGN. Every rule fires on an unambiguous signal only, and
//! everything else is None, meaning "offer the model every tool" - exactly the
//! pre-shard behaviour. A wrong narrow is unrecoverable; a missing narrow only
//! forgoes accuracy we did not have.

use crate::lexicon;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Domain {
    Calendar,
    Reminder,
    Files,
    Clipboard,
}

/// Words that name the clipboard payload itself, wherever they appear.
fn is_clipboard_noun(word: &str) -> bool {
    matches!(word, "clipboard" | "copied" | "pasted" | "selection")
}

/// Verbs that only ever open a rewrite of text the user already has. Excludes
/// "turn"/"convert" (they open system and file requests too) - those reach
/// Clipboard through the noun rule instead.
fn is_rewrite_verb(word: &str) -> bool {
    matches!(
        word,
        "make"
            | "translate"
            | "rewrite"
            | "reword"
            | "summarize"
            | "summarise"
            | "shorten"
            | "expand"
            | "proofread"
            | "paraphrase"
            | "fix"
    )
}

/// The one domain a request unambiguously belongs to, or None when the signal
/// is too weak and the planner must see the whole vocabulary.
///
/// Nouns are tested before verbs, so "postpone the standup" is Calendar rather
/// than a reminder snooze. `Files` is never returned: the strong file shapes
/// are already claimed by `files::parse` before the planner runs, and what is
/// left ("what was that resume i was working on") has no reliable signal.
pub fn of(input: &str) -> Option<Domain> {
    let lower = input.trim().to_lowercase();
    let words: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    let first = *words.first()?;

    if words.iter().any(|w| lexicon::is_reminder_noun(w)) {
        return Some(Domain::Reminder);
    }
    if words.iter().any(|w| lexicon::is_event_noun(w)) {
        return Some(Domain::Calendar);
    }
    if words.iter().any(|w| is_clipboard_noun(w)) {
        return Some(Domain::Clipboard);
    }
    match first {
        "remind" | "snooze" => Some(Domain::Reminder),
        "cancel" | "reschedule" | "block" | "schedule" | "book" => Some(Domain::Calendar),
        // "make this shorter": a rewrite verb pointed at something the user
        // has, rather than at a new thing to create.
        _ if is_rewrite_verb(first)
            && words.iter().any(|w| matches!(*w, "this" | "it" | "that")) =>
        {
            Some(Domain::Clipboard)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nouns_pick_their_store() {
        assert_eq!(of("delete the buy milk reminder"), Some(Domain::Reminder));
        assert_eq!(of("cancel my dentist appointment"), Some(Domain::Calendar));
        assert_eq!(of("whats on my calendar tomorrow"), Some(Domain::Calendar));
        // Reminder wins the tie: "the meeting reminder" is a reminder.
        assert_eq!(of("snooze the meeting reminder"), Some(Domain::Reminder));
    }

    #[test]
    fn nouns_beat_verbs() {
        // Without the noun this leads with a reminder verb; with it, the
        // request is plainly a reschedule.
        assert_eq!(of("postpone the standup"), Some(Domain::Calendar));
        assert_eq!(of("snooze buy milk until tomorrow"), Some(Domain::Reminder));
    }

    #[test]
    fn opening_verbs_narrow_when_unambiguous() {
        assert_eq!(of("cancel it"), Some(Domain::Calendar));
        assert_eq!(of("block 2 hours friday"), Some(Domain::Calendar));
        assert_eq!(of("remind me to buy milk"), Some(Domain::Reminder));
    }

    #[test]
    fn clipboard_needs_its_own_object() {
        assert_eq!(of("make this shorter"), Some(Domain::Clipboard));
        assert_eq!(
            of("translate my copied text to german"),
            Some(Domain::Clipboard)
        );
        assert_eq!(
            of("turn what i copied into bullet points"),
            Some(Domain::Clipboard)
        );
        // A rewrite verb building something new is not a clipboard op.
        assert_eq!(
            of("make a dentist appointment friday"),
            Some(Domain::Calendar)
        );
        assert_eq!(of("make a note to buy milk"), None);
    }

    #[test]
    fn weak_signals_offer_the_whole_vocabulary() {
        for input in [
            "lunch with sarah tomorrow at noon",
            "mark it done",
            "i finished the expense report",
            "what was that resume i was working on",
            "what is the capital of france",
            "",
        ] {
            assert_eq!(of(input), None, "{input}");
        }
    }

    #[test]
    fn move_stays_ambiguous_between_stores() {
        // "move" and "push" govern both stores, so they must not narrow.
        assert_eq!(of("move it to 5pm"), None);
        assert_eq!(of("push the sync to tomorrow"), None);
    }
}
