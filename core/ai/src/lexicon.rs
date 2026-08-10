//! The ONE date/time lexicon: weekday and month names with their common
//! abbreviations, relative-day words, calendar units, and modifiers. Every
//! parser (explicit, window, files) reads from here so the lists can never
//! drift apart - "screenshots fri" must mean the same Friday everywhere.

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

#[cfg(test)]
mod tests {
    use super::*;

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
