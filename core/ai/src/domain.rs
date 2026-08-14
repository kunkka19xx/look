//! Which slice of the tool vocabulary a request could want, so the planner
//! prompt can be narrowed to one domain. Conservative: None means "offer every
//! tool", because a wrong narrow is unrecoverable and a missing one is not.

use crate::lexicon;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Domain {
    Calendar,
    Reminder,
    Files,
    Clipboard,
}

/// The one domain a request unambiguously belongs to, or None to offer every
/// tool. Nouns beat verbs ("postpone the standup" is Calendar). `Files` is
/// never returned: `files::parse` claims those shapes before the planner runs.
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
    if words.iter().any(|w| lexicon::is_clipboard_noun(w)) {
        return Some(Domain::Clipboard);
    }
    // Verb tables live in the lexicon so this and `files::is_scheduling` cannot
    // disagree - they did, over "postpone".
    if lexicon::is_reminder_verb(first) {
        return Some(Domain::Reminder);
    }
    if lexicon::is_calendar_verb(first) {
        return Some(Domain::Calendar);
    }
    // "make this shorter": a rewrite verb pointed at something the user has,
    // rather than at a new thing to create.
    if lexicon::is_rewrite_verb(first) && words.iter().any(|w| matches!(*w, "this" | "it" | "that"))
    {
        return Some(Domain::Clipboard);
    }
    None
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

    /// Every schedule verb the lexicon knows must narrow to a domain. The
    /// inline copy this used to keep had already dropped "postpone".
    #[test]
    fn every_schedule_verb_narrows() {
        for word in [
            "remind",
            "snooze",
            "postpone",
            "cancel",
            "reschedule",
            "block",
            "schedule",
        ] {
            assert!(of(&format!("{word} it")).is_some(), "{word}");
        }
        assert_eq!(of("postpone it"), Some(Domain::Reminder));
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
