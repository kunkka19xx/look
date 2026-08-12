//! Timeframe extraction for schedule questions. Not a phrase list: a small
//! grammar of modifiers (this/next/coming, "in N") composed with calendar units
//! (day/week/month/year/weekend), weekday names, and month names. Weeks are ISO
//! (Monday-start) as the canonical cross-shell rule. Returns None when no frame
//! is named (caller picks a default window).

use chrono::{DateTime, Datelike, Days, Duration, Local, Months, NaiveDate, TimeZone, Weekday};

use crate::lexicon::{self, RelativeDay};

#[derive(Debug, PartialEq, serde::Serialize)]
pub struct Window {
    /// Unix epoch seconds, local-time boundaries.
    pub start: i64,
    pub end: i64,
    pub label: String,
}

#[derive(Clone, Copy, PartialEq)]
enum Unit {
    Day,
    Week,
    Month,
    Year,
}

pub fn query_window(query: &str, now_epoch: i64) -> Option<Window> {
    let now = Local.timestamp_opt(now_epoch, 0).single()?;
    let today = now.date_naive();
    let lower = query.to_lowercase();
    let words: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();

    let month_guards = [
        "in", "this", "next", "coming", "for", "during", "of", "early", "late",
    ];

    for (i, word) in words.iter().enumerate() {
        let previous = if i > 0 { words[i - 1] } else { "" };

        if let Some(day) = lexicon::relative_day(word) {
            return match day {
                RelativeDay::Today => single_day(today, "today"),
                RelativeDay::Yesterday => single_day(today.pred_opt()?, "yesterday"),
                RelativeDay::Tomorrow => single_day(today.succ_opt()?, "tomorrow"),
            };
        }
        if *word == "weekend" {
            return weekend(today, now_epoch, previous);
        }

        if let Some(unit) = unit_of(word) {
            let (offset, label) = if previous == "next" || previous == "coming" {
                (1, format!("next {word}"))
            } else if previous == "last" || previous == "previous" || previous == "past" {
                (-1, format!("last {word}"))
            } else if let Ok(n) = previous.parse::<i64>() {
                let before = if i > 1 { words[i - 2] } else { "" };
                if n > 0 && matches!(before, "last" | "previous" | "past") {
                    // Rolling backward window: "past 2 weeks" = from 2 weeks
                    // ago through the end of today.
                    return Some(Window {
                        start: midnight(shift(today, unit, -n)?)?.timestamp(),
                        end: midnight(today.succ_opt()?)?.timestamp(),
                        label: format!("last {n} {word}"),
                    });
                }
                if n > 0 {
                    (n, format!("in {n} {word}"))
                } else {
                    (0, format!("this {word}"))
                }
            } else {
                (0, format!("this {word}"))
            };
            let base = shift(today, unit, offset)?;
            let (start_date, end_date) = interval(base, unit)?;
            let start = midnight(start_date)?.timestamp();
            let start = if offset == 0 {
                start.max(now_epoch)
            } else {
                start
            };
            return Some(Window {
                start,
                end: midnight(end_date)?.timestamp(),
                label,
            });
        }

        if let Some(weekday) = lexicon::weekday_of(word) {
            if matches!(previous, "last" | "previous" | "past") {
                return single_day(
                    prev_weekday_before(today, weekday),
                    &format!("last {}", capitalize(word)),
                );
            }
            return single_day(next_weekday_after(today, weekday), &capitalize(word));
        }

        if let Some(month) = lexicon::month_of(word) {
            if *word == "may" && !month_guards.contains(&previous) {
                continue; // "may" is usually a verb
            }
            if today.month() == month && previous != "next" {
                // Inside that month: list the rest of it.
                let (start_date, end_date) = interval(today, Unit::Month)?;
                let start = midnight(start_date)?.timestamp().max(now_epoch);
                return Some(Window {
                    start,
                    end: midnight(end_date)?.timestamp(),
                    label: capitalize(word),
                });
            }
            let year = if month > today.month() {
                today.year()
            } else {
                today.year() + 1
            };
            let base = NaiveDate::from_ymd_opt(year, month, 1)?;
            let (start_date, end_date) = interval(base, Unit::Month)?;
            return Some(Window {
                start: midnight(start_date)?.timestamp(),
                end: midnight(end_date)?.timestamp(),
                label: capitalize(word),
            });
        }
    }
    None
}

pub fn query_window_json(query: &str, now_epoch: i64) -> Option<String> {
    query_window(query, now_epoch).and_then(|w| serde_json::to_string(&w).ok())
}

/// Resolve a phrase to the specific DAY it names (weekday incl. abbreviations,
/// or a relative-day word), as local-midnight epoch seconds. The shell's
/// natural-date parser (NSDataDetector on macOS) runs first; this is the
/// shared-lexicon fallback so "wed" means Wednesday on every shell. Unlike
/// `query_window`, range words ("this week") are skipped, not answered - the
/// question here is "which day", not "which span".
pub fn day_phrase(phrase: &str, now_epoch: i64) -> Option<i64> {
    let now = Local.timestamp_opt(now_epoch, 0).single()?;
    let today = now.date_naive();
    let lower = phrase.to_lowercase();
    let words: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();

    for (i, word) in words.iter().enumerate() {
        let previous = if i > 0 { words[i - 1] } else { "" };
        if let Some(day) = lexicon::relative_day(word) {
            let date = match day {
                RelativeDay::Yesterday => today.pred_opt()?,
                RelativeDay::Today => today,
                RelativeDay::Tomorrow => today.succ_opt()?,
            };
            return Some(midnight(date)?.timestamp());
        }
        if let Some(weekday) = lexicon::weekday_of(word) {
            let date = if matches!(previous, "last" | "previous" | "past") {
                prev_weekday_before(today, weekday)
            } else {
                next_weekday_after(today, weekday)
            };
            return Some(midnight(date)?.timestamp());
        }
    }
    None
}

/// Nudges a shell-resolved time toward the future when the user gave only a
/// clock time ("lunch at 1pm") that already passed today - they mean the next
/// one. A phrase that names a day or month is respected as-is (even if past).
pub fn future_leaning(phrase: &str, resolved_epoch: i64, now_epoch: i64) -> i64 {
    if resolved_epoch >= now_epoch || mentions_date(phrase) {
        return resolved_epoch;
    }
    // Time-only and in the past: same wall-clock, next day (DST-safe via chrono).
    Local
        .timestamp_opt(resolved_epoch, 0)
        .single()
        .and_then(|dt| dt.checked_add_days(Days::new(1)))
        .map(|dt| dt.timestamp())
        .unwrap_or(resolved_epoch)
}

/// Whether a phrase names a day/month/explicit date (so a past resolution is
/// intentional), vs a bare clock time.
fn mentions_date(phrase: &str) -> bool {
    let lower = phrase.to_lowercase();
    let words: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    // Day words (incl. abbreviations), month names, and forward calendar
    // units all count as an explicit date reference. Deliberately narrower
    // than `lexicon::is_date_word`: times of day ("morning") are NOT a date -
    // "8am in the morning" that already passed still means the next one.
    let day_month = words.iter().any(|w| {
        lexicon::is_day_word(w)
            || lexicon::month_of(w).is_some()
            || matches!(*w, "next" | "week" | "month" | "year")
    });
    if day_month {
        return true;
    }
    // Numeric dates: "5/3", "5-3", "the 5th".
    use std::sync::LazyLock;
    static SLASH: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"\d{1,2}[/-]\d{1,2}").expect("valid"));
    static ORDINAL: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"\b\d{1,2}(st|nd|rd|th)\b").expect("valid"));
    SLASH.is_match(&lower) || ORDINAL.is_match(&lower)
}

fn midnight(date: NaiveDate) -> Option<DateTime<Local>> {
    Local
        .from_local_datetime(&date.and_hms_opt(0, 0, 0)?)
        .earliest()
}

fn single_day(date: NaiveDate, label: &str) -> Option<Window> {
    let start = midnight(date)?;
    let end = midnight(date.checked_add_days(Days::new(1))?)?;
    Some(Window {
        start: start.timestamp(),
        end: end.timestamp(),
        label: label.into(),
    })
}

fn unit_of(word: &str) -> Option<Unit> {
    match word {
        "day" | "days" => Some(Unit::Day),
        "week" | "weeks" => Some(Unit::Week),
        "month" | "months" => Some(Unit::Month),
        "year" | "years" => Some(Unit::Year),
        _ => None,
    }
}

/// The offset comes from user text ("in 999999999 weeks"), so every multiply
/// and narrowing cast is checked: an overflow here would wrap to a nonsense
/// date instead of declining the phrase.
fn shift(date: NaiveDate, unit: Unit, offset: i64) -> Option<NaiveDate> {
    let magnitude = offset.unsigned_abs();
    let (days, months) = match unit {
        Unit::Day => (Some(magnitude), None),
        Unit::Week => (Some(magnitude.checked_mul(7)?), None),
        Unit::Month => (None, Some(u32::try_from(magnitude).ok()?)),
        Unit::Year => (None, Some(u32::try_from(magnitude.checked_mul(12)?).ok()?)),
    };
    match (days, months, offset < 0) {
        (Some(d), _, true) => date.checked_sub_days(Days::new(d)),
        (Some(d), _, false) => date.checked_add_days(Days::new(d)),
        (_, Some(m), true) => date.checked_sub_months(Months::new(m)),
        (_, Some(m), false) => date.checked_add_months(Months::new(m)),
        _ => None,
    }
}

/// Natural bounds of the unit containing `date`. ISO weeks (Monday-start).
fn interval(date: NaiveDate, unit: Unit) -> Option<(NaiveDate, NaiveDate)> {
    match unit {
        Unit::Day => Some((date, date.checked_add_days(Days::new(1))?)),
        Unit::Week => {
            let start = date - Duration::days(date.weekday().num_days_from_monday() as i64);
            Some((start, start.checked_add_days(Days::new(7))?))
        }
        Unit::Month => {
            let start = NaiveDate::from_ymd_opt(date.year(), date.month(), 1)?;
            Some((start, start.checked_add_months(Months::new(1))?))
        }
        Unit::Year => {
            let start = NaiveDate::from_ymd_opt(date.year(), 1, 1)?;
            Some((start, start.checked_add_months(Months::new(12))?))
        }
    }
}

/// The next occurrence strictly after `date` (never `date` itself).
fn next_weekday_after(date: NaiveDate, weekday: Weekday) -> NaiveDate {
    let mut candidate = date;
    loop {
        candidate = match candidate.succ_opt() {
            Some(next) => next,
            None => return date,
        };
        if candidate.weekday() == weekday {
            return candidate;
        }
    }
}

/// The most recent occurrence strictly before `date` (never `date` itself).
fn prev_weekday_before(date: NaiveDate, weekday: Weekday) -> NaiveDate {
    let mut candidate = date;
    loop {
        candidate = match candidate.pred_opt() {
            Some(prev) => prev,
            None => return date,
        };
        if candidate.weekday() == weekday {
            return candidate;
        }
    }
}

fn weekend(today: NaiveDate, now_epoch: i64, previous: &str) -> Option<Window> {
    let current = today.weekday();
    let mid_weekend = current == Weekday::Sat || current == Weekday::Sun;
    if matches!(previous, "last" | "previous" | "past") {
        let anchor = if current == Weekday::Sat {
            today
        } else {
            prev_weekday_before(today, Weekday::Sat)
        };
        // Mid-weekend, "last weekend" means the one before this one.
        let start_date = if mid_weekend {
            anchor.checked_sub_days(Days::new(7))?
        } else {
            anchor
        };
        return Some(Window {
            start: midnight(start_date)?.timestamp(),
            end: midnight(next_weekday_after(start_date, Weekday::Mon))?.timestamp(),
            label: "last weekend".into(),
        });
    }
    let saturday = next_weekday_after(today, Weekday::Sat);

    let (start_date, starts_now, label) = if mid_weekend && previous != "next" {
        (today, true, "this weekend")
    } else if previous == "next" && !mid_weekend {
        (
            saturday.checked_add_days(Days::new(7))?,
            false,
            "next weekend",
        )
    } else {
        (
            saturday,
            false,
            if previous == "next" {
                "next weekend"
            } else {
                "this weekend"
            },
        )
    };
    let monday = next_weekday_after(start_date, Weekday::Mon);
    let start = if starts_now {
        now_epoch
    } else {
        midnight(start_date)?.timestamp()
    };
    Some(Window {
        start,
        end: midnight(monday)?.timestamp(),
        label: label.into(),
    })
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_754_000_000;

    fn today() -> NaiveDate {
        Local.timestamp_opt(NOW, 0).single().unwrap().date_naive()
    }

    #[test]
    fn next_week_starts_at_iso_week_boundary() {
        let w = query_window("what's on my calendar next week?", NOW).unwrap();
        let this_week_start =
            today() - Duration::days(today().weekday().num_days_from_monday() as i64);
        let expected = midnight(this_week_start + Duration::days(7))
            .unwrap()
            .timestamp();
        assert_eq!(w.start, expected);
        assert_eq!(w.label, "next week");
    }

    #[test]
    fn tomorrow_is_one_day() {
        let w = query_window("am I busy tomorrow", NOW).unwrap();
        assert_eq!(
            w.start,
            midnight(today().succ_opt().unwrap()).unwrap().timestamp()
        );
        assert_eq!(w.label, "tomorrow");
        let next = Local.timestamp_opt(w.end, 0).single().unwrap().date_naive();
        assert_eq!(next, today().succ_opt().unwrap().succ_opt().unwrap());
    }

    #[test]
    fn weekday_name_is_next_occurrence() {
        let w = query_window("what's on friday?", NOW).unwrap();
        let start = Local
            .timestamp_opt(w.start, 0)
            .single()
            .unwrap()
            .date_naive();
        assert_eq!(start.weekday(), Weekday::Fri);
        assert!(start > today());
        assert_eq!(w.label, "Friday");
    }

    #[test]
    fn last_weekday_is_previous_occurrence() {
        let w = query_window("screenshots from last friday", NOW).unwrap();
        let start = Local
            .timestamp_opt(w.start, 0)
            .single()
            .unwrap()
            .date_naive();
        assert_eq!(start.weekday(), Weekday::Fri);
        assert!(start < today());
        assert!(today().signed_duration_since(start).num_days() <= 7);
        assert_eq!(w.label, "last Friday");
    }

    #[test]
    fn past_n_units_is_a_backward_window() {
        let w = query_window("pdfs from the last 2 weeks", NOW).unwrap();
        let start = Local
            .timestamp_opt(w.start, 0)
            .single()
            .unwrap()
            .date_naive();
        assert_eq!(start, today() - Duration::days(14));
        let end = Local.timestamp_opt(w.end, 0).single().unwrap().date_naive();
        assert_eq!(end, today().succ_opt().unwrap()); // through end of today
        assert_eq!(w.label, "last 2 weeks");

        let d = query_window("files from the past 3 days", NOW).unwrap();
        let start = Local
            .timestamp_opt(d.start, 0)
            .single()
            .unwrap()
            .date_naive();
        assert_eq!(start, today() - Duration::days(3));
    }

    #[test]
    fn last_week_is_previous_iso_week() {
        let w = query_window("meetings last week", NOW).unwrap();
        let this_week_start =
            today() - Duration::days(today().weekday().num_days_from_monday() as i64);
        assert_eq!(
            w.start,
            midnight(this_week_start - Duration::days(7))
                .unwrap()
                .timestamp()
        );
        assert_eq!(w.end, midnight(this_week_start).unwrap().timestamp());
        assert_eq!(w.label, "last week");
    }

    #[test]
    fn last_weekend_is_behind_us() {
        let w = query_window("photos from last weekend", NOW).unwrap();
        let start = Local
            .timestamp_opt(w.start, 0)
            .single()
            .unwrap()
            .date_naive();
        assert_eq!(start.weekday(), Weekday::Sat);
        assert!(start < today());
        let end = Local.timestamp_opt(w.end, 0).single().unwrap().date_naive();
        assert_eq!(end.weekday(), Weekday::Mon);
        assert_eq!(w.label, "last weekend");
    }

    #[test]
    fn next_month_name_skips_current_month() {
        let w = query_window("plans for next august", NOW).unwrap();
        let start = Local
            .timestamp_opt(w.start, 0)
            .single()
            .unwrap()
            .date_naive();
        assert_eq!(start.month(), 8);
        assert_eq!(start.day(), 1);
        let expected_year = if today().month() >= 8 {
            today().year() + 1
        } else {
            today().year()
        };
        assert_eq!(start.year(), expected_year);
    }

    #[test]
    fn day_phrase_finds_the_named_day_not_the_range() {
        // "this week wed" names Wednesday; the range words must be skipped.
        let epoch = day_phrase("go to the office this week wed", NOW).unwrap();
        let date = Local.timestamp_opt(epoch, 0).single().unwrap().date_naive();
        assert_eq!(date.weekday(), Weekday::Wed);
        assert!(date > today());

        let tmr = day_phrase("tmr", NOW).unwrap();
        let tmr_date = Local.timestamp_opt(tmr, 0).single().unwrap().date_naive();
        assert_eq!(tmr_date, today().succ_opt().unwrap());

        let last_fri = day_phrase("last fri", NOW).unwrap();
        let fri_date = Local
            .timestamp_opt(last_fri, 0)
            .single()
            .unwrap()
            .date_naive();
        assert_eq!(fri_date.weekday(), Weekday::Fri);
        assert!(fri_date < today());

        assert!(day_phrase("go to the office", NOW).is_none());
        assert!(day_phrase("this week", NOW).is_none());
    }

    #[test]
    fn weekday_abbreviations_resolve() {
        // Shared-lexicon fix: "fri" must mean the same Friday as "friday"
        // (previously files stripped "fri" from terms but no window resolved).
        let w = query_window("screenshots fri", NOW).unwrap();
        let start = Local
            .timestamp_opt(w.start, 0)
            .single()
            .unwrap()
            .date_naive();
        assert_eq!(start.weekday(), Weekday::Fri);
        assert_eq!(w.label, "Fri");
    }

    #[test]
    fn absurd_offsets_decline_instead_of_wrapping() {
        // The number comes from user text; an overflow must yield no window
        // rather than a wrapped, nonsense date.
        for query in [
            "in 999999999999 weeks",
            "in 99999999999999999 months",
            "in 9999999999 years",
        ] {
            let _ = query_window(query, NOW); // must not panic
        }
        assert!(shift(today(), Unit::Week, i64::MAX).is_none());
        assert!(shift(today(), Unit::Year, i64::MIN).is_none());
    }

    #[test]
    fn none_without_timeframe() {
        assert!(query_window("what's on my calendar?", NOW).is_none());
    }

    #[test]
    fn composes_units() {
        let next_month = query_window("what's on next month", NOW).unwrap();
        let base = today().checked_add_months(Months::new(1)).unwrap();
        let expected = NaiveDate::from_ymd_opt(base.year(), base.month(), 1).unwrap();
        assert_eq!(next_month.start, midnight(expected).unwrap().timestamp());
        assert_eq!(next_month.label, "next month");

        let this_year = query_window("events this year", NOW).unwrap();
        assert_eq!(this_year.start, NOW); // clamped: no point listing the past
        let jan1 = NaiveDate::from_ymd_opt(today().year() + 1, 1, 1).unwrap();
        assert_eq!(this_year.end, midnight(jan1).unwrap().timestamp());

        let in_two = query_window("am I busy in 2 weeks", NOW).unwrap();
        let target = today() + Duration::days(14);
        let week_start = target - Duration::days(target.weekday().num_days_from_monday() as i64);
        assert_eq!(in_two.start, midnight(week_start).unwrap().timestamp());
        assert_eq!(in_two.label, "in 2 weeks");
    }

    #[test]
    fn month_names_and_may_guard() {
        let august = query_window("what's happening in august", NOW).unwrap();
        let start = Local
            .timestamp_opt(august.start, 0)
            .single()
            .unwrap()
            .date_naive();
        assert_eq!(start.month(), 8);
        assert_eq!(august.label, "August");

        // "may" as a verb must not become the month May.
        let verb = query_window("what may be on tomorrow", NOW).unwrap();
        assert_eq!(verb.label, "tomorrow");
    }

    #[test]
    fn future_leaning_rolls_past_time_only_forward() {
        // now = midday-ish; "1pm" already passed today -> next day, same time.
        let now = NOW;
        let past = now - 3600; // an hour ago, no date words
        let adjusted = future_leaning("1pm", past, now);
        assert_eq!(adjusted, past + 86_400);
    }

    #[test]
    fn future_leaning_respects_explicit_dates_and_future() {
        let now = NOW;
        // A future time is untouched.
        assert_eq!(future_leaning("3pm", now + 3600, now), now + 3600);
        // A past time WITH a day word is intentional (e.g. "yesterday").
        assert_eq!(future_leaning("yesterday 1pm", now - 3600, now), now - 3600);
        assert_eq!(future_leaning("monday 9am", now - 3600, now), now - 3600);
        assert_eq!(future_leaning("aug 5 noon", now - 3600, now), now - 3600);
    }

    #[test]
    fn json_shape() {
        let json = query_window_json("tomorrow", NOW).unwrap();
        assert!(json.contains("\"label\":\"tomorrow\""));
        assert!(json.contains("\"start\":"));
    }
}
