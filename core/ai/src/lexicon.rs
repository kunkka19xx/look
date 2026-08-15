//! The ONE scheduling lexicon: weekday and month names with their common
//! abbreviations, relative-day words, calendar units, modifiers, and the
//! calendar/reminder domain words. Every parser (explicit, window, files)
//! reads from here so the lists can never drift apart - "screenshots fri" must
//! mean the same Friday everywhere.

use chrono::Weekday;

/// Days a relative word resolves to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RelativeDay {
    Yesterday,
    Today,
    Tomorrow,
}

/// Weekday for a name or common abbreviation ("friday", "fri").
pub fn weekday_of(word: &str) -> Option<Weekday> {
    Some(match word {
        "monday" | "mon" => Weekday::Mon,
        "tuesday" | "tue" | "tues" => Weekday::Tue,
        "wednesday" | "wed" => Weekday::Wed,
        "thursday" | "thu" | "thur" | "thurs" => Weekday::Thu,
        "friday" | "fri" => Weekday::Fri,
        "saturday" | "sat" => Weekday::Sat,
        "sunday" | "sun" => Weekday::Sun,
        _ => return None,
    })
}

/// Month number for a name or abbreviation ("august", "aug"). Callers guard
/// the "may" ambiguity themselves where it matters (see window's may-guard).
pub fn month_of(word: &str) -> Option<u32> {
    Some(match word {
        "january" | "jan" => 1,
        "february" | "feb" => 2,
        "march" | "mar" => 3,
        "april" | "apr" => 4,
        "may" => 5,
        "june" | "jun" => 6,
        "july" | "jul" => 7,
        "august" | "aug" => 8,
        "september" | "sep" | "sept" => 9,
        "october" | "oct" => 10,
        "november" | "nov" => 11,
        "december" | "dec" => 12,
        _ => return None,
    })
}

/// The day a relative word names ("tomorrow", "tmr", "yday").
pub fn relative_day(word: &str) -> Option<RelativeDay> {
    Some(match word {
        "yesterday" | "yday" => RelativeDay::Yesterday,
        "today" | "tonight" => RelativeDay::Today,
        "tomorrow" | "tmr" | "tmrw" | "tmw" | "2moro" => RelativeDay::Tomorrow,
        _ => return None,
    })
}

/// A word that names a specific day (weekday or relative), so a scheduling
/// phrase containing it carries date intent.
pub fn is_day_word(word: &str) -> bool {
    weekday_of(word).is_some() || relative_day(word).is_some()
}

/// Any date/time concept word: days, months, calendar units, range modifiers,
/// and times of day. Used to strip time words from search terms and to detect
/// explicit date references.
pub fn is_date_word(word: &str) -> bool {
    is_day_word(word)
        || month_of(word).is_some()
        || matches!(
            word,
            "day"
                | "days"
                | "week"
                | "weeks"
                | "month"
                | "months"
                | "year"
                | "years"
                | "weekend"
                | "weekends"
                | "last"
                | "this"
                | "next"
                | "past"
                | "previous"
                | "coming"
                | "recent"
                | "recently"
                | "ago"
                | "morning"
                | "afternoon"
                | "evening"
                | "night"
        )
}

/// A noun that names something on the calendar.
pub fn is_event_noun(word: &str) -> bool {
    matches!(
        word,
        "meeting"
            | "meetings"
            | "appointment"
            | "appointments"
            | "event"
            | "events"
            | "invite"
            | "invites"
            | "calendar"
            | "standup"
            | "standups"
    )
}

/// A noun that names something on the reminder list.
pub fn is_reminder_noun(word: &str) -> bool {
    matches!(word, "reminder" | "reminders" | "todo" | "todos")
}

/// A noun that names a calendar or reminder object, so the request is about
/// the user's schedule even when it also names a file type ("cancel the pdf
/// review meeting").
pub fn is_schedule_noun(word: &str) -> bool {
    is_event_noun(word) || is_reminder_noun(word)
}

/// A verb that only ever opens a scheduling request. Deliberately excludes the
/// verbs that could also govern a file ("delete", "remove", "move", "open"),
/// so "delete the pdf i downloaded" stays file recall.
pub fn is_schedule_verb(word: &str) -> bool {
    is_reminder_verb(word) || is_calendar_verb(word)
}

/// Opens a request about the reminder list and nothing else.
pub fn is_reminder_verb(word: &str) -> bool {
    matches!(word, "remind" | "snooze" | "postpone")
}

/// Opens a request about the calendar and nothing else.
pub fn is_calendar_verb(word: &str) -> bool {
    matches!(
        word,
        "cancel" | "reschedule" | "block" | "book" | "schedule"
    )
}

/// Names the clipboard payload itself, wherever it appears in the request.
pub fn is_clipboard_noun(word: &str) -> bool {
    matches!(word, "clipboard" | "copied" | "pasted" | "selection")
}

/// Only ever opens a rewrite of text the user already has. Excludes
/// "turn"/"convert", which open system and file requests too.
pub fn is_rewrite_verb(word: &str) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The domain prefilter and the file-recall veto read the SAME verb
    /// tables. When `domain.rs` kept its own inline copies they drifted inside
    /// one commit: "postpone" was a schedule verb but not a reminder verb, so
    /// `of("postpone it")` narrowed to nothing while `of("snooze it")` did.
    #[test]
    fn schedule_verbs_are_exactly_the_two_domain_sets() {
        for word in [
            "remind",
            "snooze",
            "postpone",
            "cancel",
            "reschedule",
            "block",
        ] {
            assert!(is_schedule_verb(word), "{word}");
            assert!(
                is_reminder_verb(word) || is_calendar_verb(word),
                "{word} belongs to no domain"
            );
        }
        // A verb cannot claim both domains, or the prefilter has no answer.
        for word in ["remind", "cancel", "postpone", "book"] {
            assert!(
                !(is_reminder_verb(word) && is_calendar_verb(word)),
                "{word}"
            );
        }
    }

    #[test]
    fn schedule_words_exclude_file_capable_verbs() {
        for word in ["meeting", "appointment", "reminder", "calendar"] {
            assert!(is_schedule_noun(word), "{word}");
        }
        for word in ["pdf", "screenshot", "invoice", "desktop"] {
            assert!(!is_schedule_noun(word), "{word}");
        }
        for word in ["cancel", "reschedule", "snooze", "remind"] {
            assert!(is_schedule_verb(word), "{word}");
        }
        // These govern files too, so they must never veto file recall.
        for word in ["delete", "remove", "move", "open", "find"] {
            assert!(!is_schedule_verb(word), "{word}");
        }
    }

    #[test]
    fn abbreviations_match_their_full_names() {
        for (abbrev, full) in [
            ("mon", "monday"),
            ("tues", "tuesday"),
            ("thurs", "thursday"),
            ("fri", "friday"),
            ("sun", "sunday"),
        ] {
            assert_eq!(weekday_of(abbrev), weekday_of(full), "{abbrev}");
        }
        for (abbrev, full) in [
            ("jan", "january"),
            ("sept", "september"),
            ("dec", "december"),
        ] {
            assert_eq!(month_of(abbrev), month_of(full), "{abbrev}");
        }
    }

    #[test]
    fn date_words_cover_days_units_and_modifiers() {
        for word in ["fri", "tmr", "yday", "aug", "weeks", "past", "evening"] {
            assert!(is_date_word(word), "{word}");
        }
        for word in ["taxes", "resume", "screenshot", "mai"] {
            assert!(!is_date_word(word), "{word}");
        }
    }
}
