//! Timeframe extraction for schedule questions. Not a phrase list: a small
//! grammar of modifiers (this/next/coming, "in N") composed with calendar units
//! (day/week/month/year/weekend), weekday names, and month names. Weeks are ISO
//! (Monday-start) as the canonical cross-shell rule. Returns None when no frame
//! is named (caller picks a default window).

use chrono::{DateTime, Datelike, Days, Duration, Local, Months, NaiveDate, TimeZone, Weekday};

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

    let month_guards = ["in", "this", "next", "coming", "for", "during", "of", "early", "late"];

    for (i, word) in words.iter().enumerate() {
        let previous = if i > 0 { words[i - 1] } else { "" };

        match *word {
            "today" | "tonight" => return single_day(today, "today"),
            "tomorrow" | "tmr" | "tmrw" | "tmw" => {
                return single_day(today.succ_opt()?, "tomorrow");
            }
            "weekend" => return weekend(today, now_epoch, previous),
            _ => {}
        }

        if let Some(unit) = unit_of(word) {
            let (offset, label) = if previous == "next" || previous == "coming" {
                (1, format!("next {word}"))
            } else if let Ok(n) = previous.parse::<i64>() {
                if n > 0 { (n, format!("in {n} {word}")) } else { (0, format!("this {word}")) }
            } else {
                (0, format!("this {word}"))
            };
            let base = shift(today, unit, offset)?;
            let (start_date, end_date) = interval(base, unit)?;
            let start = midnight(start_date)?.timestamp();
            let start = if offset == 0 { start.max(now_epoch) } else { start };
            return Some(Window { start, end: midnight(end_date)?.timestamp(), label });
        }

        if let Some(weekday) = weekday_of(word) {
            return single_day(next_weekday_after(today, weekday), &capitalize(word));
        }

        if let Some(month) = month_of(word) {
            if *word == "may" && !month_guards.contains(&previous) {
                continue; // "may" is usually a verb
            }
            if today.month() == month {
                // Inside that month: list the rest of it.
                let (start_date, end_date) = interval(today, Unit::Month)?;
                let start = midnight(start_date)?.timestamp().max(now_epoch);
                return Some(Window {
                    start,
                    end: midnight(end_date)?.timestamp(),
                    label: capitalize(word),
                });
            }
            let year = if month > today.month() { today.year() } else { today.year() + 1 };
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

fn midnight(date: NaiveDate) -> Option<DateTime<Local>> {
    Local.from_local_datetime(&date.and_hms_opt(0, 0, 0)?).earliest()
}

fn single_day(date: NaiveDate, label: &str) -> Option<Window> {
    let start = midnight(date)?;
    let end = midnight(date.checked_add_days(Days::new(1))?)?;
    Some(Window { start: start.timestamp(), end: end.timestamp(), label: label.into() })
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

fn shift(date: NaiveDate, unit: Unit, offset: i64) -> Option<NaiveDate> {
    match unit {
        Unit::Day => date.checked_add_days(Days::new(offset as u64)),
        Unit::Week => date.checked_add_days(Days::new((offset * 7) as u64)),
        Unit::Month => date.checked_add_months(Months::new(offset as u32)),
        Unit::Year => date.checked_add_months(Months::new((offset * 12) as u32)),
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

fn weekday_of(word: &str) -> Option<Weekday> {
    match word {
        "monday" => Some(Weekday::Mon),
        "tuesday" => Some(Weekday::Tue),
        "wednesday" => Some(Weekday::Wed),
        "thursday" => Some(Weekday::Thu),
        "friday" => Some(Weekday::Fri),
        "saturday" => Some(Weekday::Sat),
        "sunday" => Some(Weekday::Sun),
        _ => None,
    }
}

fn month_of(word: &str) -> Option<u32> {
    match word {
        "january" => Some(1),
        "february" => Some(2),
        "march" => Some(3),
        "april" => Some(4),
        "may" => Some(5),
        "june" => Some(6),
        "july" => Some(7),
        "august" => Some(8),
        "september" => Some(9),
        "october" => Some(10),
        "november" => Some(11),
        "december" => Some(12),
        _ => None,
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

fn weekend(today: NaiveDate, now_epoch: i64, previous: &str) -> Option<Window> {
    let current = today.weekday();
    let mid_weekend = current == Weekday::Sat || current == Weekday::Sun;
    let saturday = next_weekday_after(today, Weekday::Sat);

    let (start_date, starts_now, label) = if mid_weekend && previous != "next" {
        (today, true, "this weekend")
    } else if previous == "next" && !mid_weekend {
        (saturday.checked_add_days(Days::new(7))?, false, "next weekend")
    } else {
        (saturday, false, if previous == "next" { "next weekend" } else { "this weekend" })
    };
    let monday = next_weekday_after(start_date, Weekday::Mon);
    let start = if starts_now { now_epoch } else { midnight(start_date)?.timestamp() };
    Some(Window { start, end: midnight(monday)?.timestamp(), label: label.into() })
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
        let expected = midnight(this_week_start + Duration::days(7)).unwrap().timestamp();
        assert_eq!(w.start, expected);
        assert_eq!(w.label, "next week");
    }

    #[test]
    fn tomorrow_is_one_day() {
        let w = query_window("am I busy tomorrow", NOW).unwrap();
        assert_eq!(w.start, midnight(today().succ_opt().unwrap()).unwrap().timestamp());
        assert_eq!(w.label, "tomorrow");
        let next = Local.timestamp_opt(w.end, 0).single().unwrap().date_naive();
        assert_eq!(next, today().succ_opt().unwrap().succ_opt().unwrap());
    }

    #[test]
    fn weekday_name_is_next_occurrence() {
        let w = query_window("what's on friday?", NOW).unwrap();
        let start = Local.timestamp_opt(w.start, 0).single().unwrap().date_naive();
        assert_eq!(start.weekday(), Weekday::Fri);
        assert!(start > today());
        assert_eq!(w.label, "Friday");
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
        let start = Local.timestamp_opt(august.start, 0).single().unwrap().date_naive();
        assert_eq!(start.month(), 8);
        assert_eq!(august.label, "August");

        // "may" as a verb must not become the month May.
        let verb = query_window("what may be on tomorrow", NOW).unwrap();
        assert_eq!(verb.label, "tomorrow");
    }

    #[test]
    fn json_shape() {
        let json = query_window_json("tomorrow", NOW).unwrap();
        assert!(json.contains("\"label\":\"tomorrow\""));
        assert!(json.contains("\"start\":"));
    }
}
