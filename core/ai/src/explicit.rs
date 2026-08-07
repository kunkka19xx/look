//! The instant, no-model action path. Handles ONLY the explicit delimited form
//! where the user separates title and time with `@`:
//!   >add <title> @ <when>       ->  calendar.add_event
//!   >remind <title> @ <when>    ->  reminder.add
//! Anything without `@` is natural language and returns None so the model
//! normalizes it. A day word in the title half defers to the model too (the
//! date intent is split); without a model (lenient mode), the whole phrase
//! becomes the `when` so the day still resolves right.

use serde_json::{Value, json};

const DAY_WORDS: [&str; 24] = [
    "today",
    "tomorrow",
    "tonight",
    "tmr",
    "tmrw",
    "tmw",
    "2moro",
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
    "sunday",
    "mon",
    "tue",
    "tues",
    "wed",
    "thu",
    "thur",
    "thurs",
    "fri",
    "sat",
    "sun",
];

pub fn contains_day_word(text: &str) -> bool {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .any(|w| DAY_WORDS.contains(&w))
}

pub fn parse(input: &str, model_available: bool) -> Option<Value> {
    let trimmed = input.trim();
    let body = trimmed.strip_prefix('>')?.trim();
    let (verb, rest) = body.split_once(char::is_whitespace)?;
    let rest = rest.trim();
    if rest.is_empty() {
        return None;
    }

    // ` @` (whitespace before the at-sign) splits title from time; emails in
    // titles stay safe. Accepts both "title @ 3pm" and "title @3pm".
    let at = rest.find(" @")?;
    let title = rest[..at].trim();
    let when_text = rest[at + 2..].trim();
    if title.is_empty() {
        return None;
    }
    let mut when = if when_text.is_empty() {
        None
    } else {
        Some(when_text.to_string())
    };

    if contains_day_word(title) {
        if model_available {
            return None; // model gives a clean title AND the right day
        }
        // Lenient: verbatim title, but resolve the date from the whole phrase.
        when = Some(rest.to_string());
    }

    let verb = verb.to_lowercase();
    match verb.as_str() {
        "add" | "event" | "cal" => {
            // "1pm for 2 hours" -> when "1pm", duration "2 hours". Only splits
            // when the tail is a real duration, so titles/times are untouched.
            let when = when.unwrap_or_default();
            let (when, duration) = match when.split_once(" for ") {
                Some((head, tail))
                    if crate::resolve::parse_duration_minutes(tail.trim()).is_some() =>
                {
                    (head.trim().to_string(), Some(tail.trim().to_string()))
                }
                _ => (when, None),
            };
            let mut params = json!({ "title": title, "when": when });
            if let Some(duration) = duration {
                params["duration"] = Value::String(duration);
            }
            Some(json!({ "tool": "calendar.add_event", "params": params }))
        }
        "remind" | "reminder" => {
            let mut params = json!({ "title": title });
            if let Some(when) = when {
                params["when"] = Value::String(when);
            }
            Some(json!({ "tool": "reminder.add", "params": params }))
        }
        _ => None,
    }
}

pub fn parse_json(input: &str, model_available: bool) -> Option<String> {
    parse(input, model_available).map(|v| v.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_add_with_separator() {
        let call = parse(">add lunch @ tomorrow 12pm", true).unwrap();
        assert_eq!(call["tool"], "calendar.add_event");
        assert_eq!(call["params"]["title"], "lunch");
        assert_eq!(call["params"]["when"], "tomorrow 12pm");
    }

    #[test]
    fn parses_separator_without_trailing_space() {
        let call = parse(">add lunch @3pm", true).unwrap();
        assert_eq!(call["params"]["when"], "3pm");
    }

    #[test]
    fn parses_trailing_duration() {
        let call = parse(">add lunch @ 1pm for 2 hours", true).unwrap();
        assert_eq!(call["params"]["when"], "1pm");
        assert_eq!(call["params"]["duration"], "2 hours");
    }

    #[test]
    fn non_duration_for_is_left_in_when() {
        // "for review" is not a duration, so the phrase stays intact.
        let call = parse(">add sync @ 3pm for review", true).unwrap();
        assert_eq!(call["params"]["when"], "3pm for review");
        assert!(call["params"].get("duration").is_none());
    }

    #[test]
    fn parses_remind() {
        let call = parse(">remind call mom @ 5pm", true).unwrap();
        assert_eq!(call["tool"], "reminder.add");
        assert_eq!(call["params"]["title"], "call mom");
        assert_eq!(call["params"]["when"], "5pm");
    }

    #[test]
    fn day_word_in_title_defers_to_model() {
        assert!(parse(">remind me to call mom on sunday @ 5pm", true).is_none());
        assert!(parse(">add standup tomorrow @ 9am", true).is_none());
        assert!(parse(">remind call mom @ sunday 5pm", true).is_some());
    }

    #[test]
    fn lenient_mode_resolves_day_from_whole_phrase() {
        let call = parse(">add call GF today @2pm", false).unwrap();
        assert_eq!(call["params"]["title"], "call GF today");
        assert_eq!(call["params"]["when"], "call GF today @2pm");
        let clean = parse(">add lunch @ 1pm", false).unwrap();
        assert_eq!(clean["params"]["when"], "1pm");
    }

    #[test]
    fn non_action_input_is_none() {
        assert!(parse("add lunch @ 1pm", true).is_none()); // no `>`
        assert!(parse(">bogus something @ 5pm", true).is_none()); // unknown verb
        assert!(parse(">add", true).is_none()); // no spec
        assert!(parse(">remind me to walk my dog at 7pm", true).is_none()); // no `@`
    }
}
