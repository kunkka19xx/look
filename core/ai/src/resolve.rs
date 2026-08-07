//! Tool RESOLUTION (the P4 contract): the shell passes platform data in
//! (candidates, the seam-resolved date); this validates, matches through the
//! ambiguity gate, computes dates/previews, and returns a data-only outcome the
//! shell can execute and undo. No closures cross the boundary.

use chrono::{DateTime, Local, TimeZone, Timelike};
use serde::{Deserialize, Serialize};

use crate::matcher::{self, Outcome as Match};

pub const DEFAULT_EVENT_MINUTES: i64 = 60;
const SEARCH_DAYS: i64 = 30;

#[derive(Debug, Clone, Deserialize)]
pub struct EventCandidate {
    pub id: String,
    pub title: String,
    pub start: i64,
    pub end: i64,
    #[serde(default)]
    pub all_day: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReminderCandidate {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub due: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct ResolveRequest {
    pub tool: String,
    #[serde(default)]
    pub params: std::collections::HashMap<String, String>,
    pub now: i64,
    #[serde(default)]
    pub events: Vec<EventCandidate>,
    #[serde(default)]
    pub reminders: Vec<ReminderCandidate>,
    /// The seam: shell-resolved epoch of `params.when` (NSDataDetector on
    /// macOS). None when absent or unresolvable.
    #[serde(default)]
    pub resolved_when: Option<i64>,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Execute {
    AddEvent { title: String, start: i64, end: i64, all_day: bool },
    AddReminder { title: String, due: Option<i64> },
    RemoveEvent { id: String },
    MoveEvent { id: String, start: i64, end: i64 },
    CompleteReminder { id: String },
    RemoveReminder { id: String },
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Undo {
    RemoveEventByNewId,
    RemoveReminderByNewId,
    RecreateEvent { title: String, start: i64, end: i64, all_day: bool },
    MoveEvent { id: String, start: i64, end: i64 },
    UncompleteReminder { id: String },
    RecreateReminder { title: String, due: Option<i64> },
}

#[derive(Debug, PartialEq, Serialize)]
pub struct Candidate {
    pub id: String,
    pub label: String,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ResolveOutcome {
    Planned {
        preview_title: String,
        preview_detail: String,
        summary: String,
        /// Subject id for referent follow-ups; "new" means the id the executor
        /// gets back from an add.
        subject: Option<String>,
        execute: Execute,
        undo: Undo,
    },
    Choice { candidates: Vec<Candidate> },
    Invalid { message: String },
}

pub fn resolve(request: &ResolveRequest) -> ResolveOutcome {
    match request.tool.as_str() {
        "calendar.add_event" => add_event(request),
        "reminder.add" => add_reminder(request),
        "calendar.cancel_event" => cancel_event(request),
        "calendar.move_event" => move_event(request),
        "reminder.complete" => complete_reminder(request),
        "reminder.remove" => remove_reminder(request),
        other => ResolveOutcome::Invalid { message: format!("Unknown action: {other}") },
    }
}

pub fn resolve_json(request_json: &str) -> String {
    let outcome = match serde_json::from_str::<ResolveRequest>(request_json) {
        Ok(request) => resolve(&request),
        Err(_) => ResolveOutcome::Invalid { message: "Bad resolve request.".into() },
    };
    serde_json::to_string(&outcome)
        .unwrap_or_else(|_| r#"{"outcome":"invalid","message":"encode failed"}"#.into())
}

// ── add tools ───────────────────────────────────────────────────────────

fn add_event(request: &ResolveRequest) -> ResolveOutcome {
    let Some(title) = param(request, "title") else {
        return invalid("Need a title for the event.");
    };
    let when_phrase = param(request, "when").unwrap_or_default();

    // No date at all -> all-day today. A day with no clock time -> all-day on
    // that day. A day with a clock time -> timed. Never invents a time.
    if when_phrase.is_empty() {
        return all_day_event(&title, request.now);
    }
    let Some(start) = request.resolved_when else {
        return invalid(&format!("Could not understand the time \"{when_phrase}\"."));
    };
    if !has_clock_time(&when_phrase) {
        return all_day_event(&title, start);
    }
    let minutes = param(request, "duration_minutes")
        .and_then(|m| m.parse::<i64>().ok())
        .unwrap_or(DEFAULT_EVENT_MINUTES);
    if minutes <= 0 {
        return invalid("Duration must be positive.");
    }
    let end = start + minutes * 60;
    ResolveOutcome::Planned {
        preview_title: "Add event".into(),
        preview_detail: format!("\"{title}\"  {}", fmt_range(start, end)),
        summary: format!("Added \"{title}\""),
        subject: Some("new".into()),
        execute: Execute::AddEvent { title: title.clone(), start, end, all_day: false },
        undo: Undo::RemoveEventByNewId,
    }
}

fn all_day_event(title: &str, day_epoch: i64) -> ResolveOutcome {
    let Some((start, end)) = day_bounds(day_epoch) else {
        return invalid("Could not compute that day.");
    };
    ResolveOutcome::Planned {
        preview_title: "Add event".into(),
        preview_detail: format!("\"{title}\"  {} (all day)", fmt_day(start)),
        summary: format!("Added \"{title}\""),
        subject: Some("new".into()),
        execute: Execute::AddEvent { title: title.into(), start, end, all_day: true },
        undo: Undo::RemoveEventByNewId,
    }
}

fn add_reminder(request: &ResolveRequest) -> ResolveOutcome {
    let Some(title) = param(request, "title") else {
        return invalid("Need something to be reminded of.");
    };
    let when_phrase = param(request, "when").unwrap_or_default();
    let due = if when_phrase.is_empty() {
        None
    } else {
        match request.resolved_when {
            Some(due) => Some(due),
            None => {
                return invalid(&format!("Could not understand the time \"{when_phrase}\"."));
            }
        }
    };
    let detail = match due {
        Some(due) => format!("\"{title}\"  due {}", fmt_instant(due)),
        None => format!("\"{title}\""),
    };
    ResolveOutcome::Planned {
        preview_title: "Add reminder".into(),
        preview_detail: detail,
        summary: format!("Added reminder \"{title}\""),
        subject: Some("new".into()),
        execute: Execute::AddReminder { title: title.clone(), due },
        undo: Undo::RemoveReminderByNewId,
    }
}

// ── mutate tools ────────────────────────────────────────────────────────

fn cancel_event(request: &ResolveRequest) -> ResolveOutcome {
    match pick_event(request) {
        Err(outcome) => outcome,
        Ok(event) => ResolveOutcome::Planned {
            preview_title: "Cancel event".into(),
            preview_detail: event_detail(&event),
            summary: format!("Cancelled \"{}\"", event.title),
            subject: None,
            execute: Execute::RemoveEvent { id: event.id.clone() },
            undo: Undo::RecreateEvent {
                title: event.title,
                start: event.start,
                end: event.end,
                all_day: event.all_day,
            },
        },
    }
}

fn move_event(request: &ResolveRequest) -> ResolveOutcome {
    let Some(when_phrase) = param(request, "when") else {
        return invalid("When should it move to?");
    };
    let Some(resolved) = request.resolved_when else {
        return invalid(&format!("Could not understand the time \"{when_phrase}\"."));
    };
    match pick_event(request) {
        Err(outcome) => outcome,
        Ok(event) => {
            // A day-only phrase ("friday") keeps the event's clock time.
            let new_start = if has_clock_time(&when_phrase) {
                resolved
            } else {
                preserving_clock(event.start, resolved).unwrap_or(resolved)
            };
            let new_end = new_start + (event.end - event.start);
            ResolveOutcome::Planned {
                preview_title: "Move event".into(),
                preview_detail: format!(
                    "\"{}\"  {} -> {}",
                    event.title,
                    fmt_instant(event.start),
                    fmt_range(new_start, new_end)
                ),
                summary: format!("Moved \"{}\" to {}", event.title, fmt_instant(new_start)),
                subject: Some(event.id.clone()),
                execute: Execute::MoveEvent { id: event.id.clone(), start: new_start, end: new_end },
                undo: Undo::MoveEvent { id: event.id, start: event.start, end: event.end },
            }
        }
    }
}

fn complete_reminder(request: &ResolveRequest) -> ResolveOutcome {
    match pick_reminder(request) {
        Err(outcome) => outcome,
        Ok(reminder) => ResolveOutcome::Planned {
            preview_title: "Complete reminder".into(),
            preview_detail: format!("\"{}\"", reminder.title),
            summary: format!("Completed \"{}\"", reminder.title),
            subject: Some(reminder.id.clone()),
            execute: Execute::CompleteReminder { id: reminder.id.clone() },
            undo: Undo::UncompleteReminder { id: reminder.id },
        },
    }
}

fn remove_reminder(request: &ResolveRequest) -> ResolveOutcome {
    match pick_reminder(request) {
        Err(outcome) => outcome,
        Ok(reminder) => ResolveOutcome::Planned {
            preview_title: "Remove reminder".into(),
            preview_detail: format!("\"{}\"", reminder.title),
            summary: format!("Removed reminder \"{}\"", reminder.title),
            subject: None,
            execute: Execute::RemoveReminder { id: reminder.id.clone() },
            undo: Undo::RecreateReminder { title: reminder.title, due: reminder.due },
        },
    }
}

// ── matching (chosen_id bypass, then the gate) ──────────────────────────

fn pick_event(request: &ResolveRequest) -> Result<EventCandidate, ResolveOutcome> {
    if let Some(chosen) = param(request, "chosen_id") {
        return request
            .events
            .iter()
            .find(|e| e.id == chosen)
            .cloned()
            .ok_or_else(|| invalid("That event is gone."));
    }
    let Some(query) = param(request, "match") else {
        return Err(invalid("Which event?"));
    };
    match matcher::resolve(&request.events, &query, |e| e.title.clone()) {
        Match::None => Err(invalid(&format!(
            "No event matching \"{query}\" in the next {SEARCH_DAYS} days."
        ))),
        Match::Several(options) => Err(ResolveOutcome::Choice {
            candidates: options
                .into_iter()
                .map(|e| Candidate {
                    label: format!("{}  ·  {}", e.title, fmt_instant(e.start)),
                    id: e.id,
                })
                .collect(),
        }),
        Match::One(event) => Ok(event),
    }
}

fn pick_reminder(request: &ResolveRequest) -> Result<ReminderCandidate, ResolveOutcome> {
    if let Some(chosen) = param(request, "chosen_id") {
        return request
            .reminders
            .iter()
            .find(|r| r.id == chosen)
            .cloned()
            .ok_or_else(|| invalid("That reminder is gone."));
    }
    let Some(query) = param(request, "match") else {
        return Err(invalid("Which reminder?"));
    };
    match matcher::resolve(&request.reminders, &query, |r| r.title.clone()) {
        Match::None => Err(invalid(&format!("No open reminder matching \"{query}\"."))),
        Match::Several(options) => Err(ResolveOutcome::Choice {
            candidates: options
                .into_iter()
                .map(|r| Candidate { label: r.title.clone(), id: r.id })
                .collect(),
        }),
        Match::One(reminder) => Ok(reminder),
    }
}

// ── helpers ─────────────────────────────────────────────────────────────

fn param(request: &ResolveRequest, key: &str) -> Option<String> {
    request
        .params
        .get(key)
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn invalid(message: &str) -> ResolveOutcome {
    ResolveOutcome::Invalid { message: message.into() }
}

/// Whether a phrase names a clock time (vs just a day): "3pm", "15:00", "noon".
/// Lexical, not a semantic guess.
pub fn has_clock_time(phrase: &str) -> bool {
    let p = phrase.to_lowercase();
    if p.contains("noon") || p.contains("midnight") || p.contains("o'clock") {
        return true;
    }
    let am_pm = regex::Regex::new(r"\d{1,2}\s?(am|pm)").expect("valid");
    let hh_mm = regex::Regex::new(r"\d{1,2}:\d{2}").expect("valid");
    am_pm.is_match(&p) || hh_mm.is_match(&p)
}

fn local(epoch: i64) -> Option<DateTime<Local>> {
    Local.timestamp_opt(epoch, 0).single()
}

fn day_bounds(epoch: i64) -> Option<(i64, i64)> {
    let date = local(epoch)?.date_naive();
    let start = Local.from_local_datetime(&date.and_hms_opt(0, 0, 0)?).earliest()?;
    let end = Local
        .from_local_datetime(&date.succ_opt()?.and_hms_opt(0, 0, 0)?)
        .earliest()?;
    Some((start.timestamp(), end.timestamp()))
}

/// New day from `day_of`, keeping the clock time of `old`.
fn preserving_clock(old: i64, day_of: i64) -> Option<i64> {
    let old = local(old)?;
    let day = local(day_of)?.date_naive();
    let combined = day.and_hms_opt(old.hour(), old.minute(), 0)?;
    Some(Local.from_local_datetime(&combined).earliest()?.timestamp())
}

fn fmt_range(start: i64, end: i64) -> String {
    match (local(start), local(end)) {
        (Some(s), Some(e)) => format!(
            "{}, {}-{}",
            s.format("%a %b %-d"),
            s.format("%H:%M"),
            e.format("%H:%M")
        ),
        _ => String::new(),
    }
}

fn fmt_instant(epoch: i64) -> String {
    local(epoch).map(|d| d.format("%a %b %-d, %H:%M").to_string()).unwrap_or_default()
}

fn fmt_day(epoch: i64) -> String {
    local(epoch).map(|d| d.format("%a %b %-d").to_string()).unwrap_or_default()
}

fn event_detail(event: &EventCandidate) -> String {
    if event.all_day {
        format!("\"{}\"  {} (all day)", event.title, fmt_day(event.start))
    } else {
        format!("\"{}\"  {}", event.title, fmt_range(event.start, event.end))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_754_000_000;

    fn request(tool: &str, params: &[(&str, &str)]) -> ResolveRequest {
        ResolveRequest {
            tool: tool.into(),
            params: params.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            now: NOW,
            events: vec![
                EventCandidate { id: "e1".into(), title: "Dentist".into(), start: NOW + 3600, end: NOW + 7200, all_day: false },
                EventCandidate { id: "e2".into(), title: "Team Sync".into(), start: NOW + 86_400, end: NOW + 90_000, all_day: false },
                EventCandidate { id: "e3".into(), title: "Team Standup".into(), start: NOW + 172_800, end: NOW + 176_400, all_day: false },
            ],
            reminders: vec![ReminderCandidate { id: "r1".into(), title: "Walk Dog".into(), due: None }],
            resolved_when: None,
        }
    }

    #[test]
    fn add_event_timed_defaults_sixty_minutes() {
        let mut req = request("calendar.add_event", &[("title", "Lunch"), ("when", "tomorrow 1pm")]);
        req.resolved_when = Some(NOW + 90_000);
        match resolve(&req) {
            ResolveOutcome::Planned { execute: Execute::AddEvent { start, end, all_day, .. }, subject, .. } => {
                assert_eq!(end - start, 3600);
                assert!(!all_day);
                assert_eq!(subject.as_deref(), Some("new"));
            }
            other => panic!("expected planned, got {other:?}"),
        }
    }

    #[test]
    fn add_event_day_without_clock_is_all_day() {
        let mut req = request("calendar.add_event", &[("title", "Birthday"), ("when", "march 5")]);
        req.resolved_when = Some(NOW + 86_400);
        match resolve(&req) {
            ResolveOutcome::Planned { execute: Execute::AddEvent { all_day, .. }, preview_detail, .. } => {
                assert!(all_day);
                assert!(preview_detail.contains("all day"));
            }
            other => panic!("expected planned, got {other:?}"),
        }
    }

    #[test]
    fn add_event_no_date_is_all_day_today() {
        let req = request("calendar.add_event", &[("title", "Lunch with Sarah")]);
        match resolve(&req) {
            ResolveOutcome::Planned { execute: Execute::AddEvent { all_day, start, .. }, .. } => {
                assert!(all_day);
                let (day_start, _) = day_bounds(NOW).unwrap();
                assert_eq!(start, day_start);
            }
            other => panic!("expected planned, got {other:?}"),
        }
    }

    #[test]
    fn add_reminder_without_due_is_undated() {
        let req = request("reminder.add", &[("title", "Buy milk")]);
        match resolve(&req) {
            ResolveOutcome::Planned { execute: Execute::AddReminder { due, .. }, .. } => {
                assert!(due.is_none());
            }
            other => panic!("expected planned, got {other:?}"),
        }
    }

    #[test]
    fn cancel_confident_match_with_recreate_undo() {
        let req = request("calendar.cancel_event", &[("match", "dentist")]);
        match resolve(&req) {
            ResolveOutcome::Planned { execute: Execute::RemoveEvent { id }, undo, subject, .. } => {
                assert_eq!(id, "e1");
                assert!(subject.is_none());
                assert!(matches!(undo, Undo::RecreateEvent { .. }));
            }
            other => panic!("expected planned, got {other:?}"),
        }
    }

    #[test]
    fn cancel_ambiguous_is_choice_and_never_mutates() {
        let req = request("calendar.cancel_event", &[("match", "team")]);
        match resolve(&req) {
            ResolveOutcome::Choice { candidates } => {
                let ids: Vec<&str> = candidates.iter().map(|c| c.id.as_str()).collect();
                assert!(ids.contains(&"e2") && ids.contains(&"e3"));
            }
            other => panic!("expected choice, got {other:?}"),
        }
    }

    #[test]
    fn cancel_lone_weak_match_is_choice() {
        // The wrong-target regression: "it" must never silently cancel Dentist.
        let mut req = request("calendar.cancel_event", &[("match", "it")]);
        req.events.truncate(1);
        assert!(matches!(resolve(&req), ResolveOutcome::Choice { .. }));
    }

    #[test]
    fn chosen_id_bypasses_matching() {
        let req = request("calendar.cancel_event", &[("chosen_id", "e2")]);
        match resolve(&req) {
            ResolveOutcome::Planned { execute: Execute::RemoveEvent { id }, .. } => assert_eq!(id, "e2"),
            other => panic!("expected planned, got {other:?}"),
        }
    }

    #[test]
    fn move_preserves_duration_and_restores_on_undo() {
        let mut req = request("calendar.move_event", &[("match", "dentist"), ("when", "3pm")]);
        req.resolved_when = Some(NOW + 200_000);
        match resolve(&req) {
            ResolveOutcome::Planned {
                execute: Execute::MoveEvent { id, start, end },
                undo: Undo::MoveEvent { start: old_start, end: old_end, .. },
                subject,
                ..
            } => {
                assert_eq!(id, "e1");
                assert_eq!(end - start, 3600);
                assert_eq!(old_start, NOW + 3600);
                assert_eq!(old_end, NOW + 7200);
                assert_eq!(subject.as_deref(), Some("e1"));
            }
            other => panic!("expected planned, got {other:?}"),
        }
    }

    #[test]
    fn move_day_only_phrase_keeps_clock_time() {
        let mut req = request("calendar.move_event", &[("match", "dentist"), ("when", "friday")]);
        req.resolved_when = Some(NOW + 400_000);
        match resolve(&req) {
            ResolveOutcome::Planned { execute: Execute::MoveEvent { start, .. }, .. } => {
                let old_clock = local(NOW + 3600).unwrap();
                let new_clock = local(start).unwrap();
                assert_eq!(new_clock.hour(), old_clock.hour());
                assert_eq!(new_clock.minute(), old_clock.minute());
            }
            other => panic!("expected planned, got {other:?}"),
        }
    }

    #[test]
    fn reminder_complete_and_remove() {
        match resolve(&request("reminder.complete", &[("match", "walk dog")])) {
            ResolveOutcome::Planned { execute: Execute::CompleteReminder { id }, undo, .. } => {
                assert_eq!(id, "r1");
                assert!(matches!(undo, Undo::UncompleteReminder { .. }));
            }
            other => panic!("expected planned, got {other:?}"),
        }
        match resolve(&request("reminder.remove", &[("match", "walk dog")])) {
            ResolveOutcome::Planned { undo: Undo::RecreateReminder { title, .. }, .. } => {
                assert_eq!(title, "Walk Dog");
            }
            other => panic!("expected planned, got {other:?}"),
        }
    }

    #[test]
    fn no_match_and_unknown_tool_are_invalid() {
        assert!(matches!(
            resolve(&request("calendar.cancel_event", &[("match", "zebra")])),
            ResolveOutcome::Invalid { .. }
        ));
        assert!(matches!(
            resolve(&request("nope", &[])),
            ResolveOutcome::Invalid { .. }
        ));
    }

    #[test]
    fn clock_time_detection() {
        assert!(has_clock_time("3pm"));
        assert!(has_clock_time("friday 15:00"));
        assert!(has_clock_time("friday noon"));
        assert!(!has_clock_time("march 5"));
        assert!(!has_clock_time("next friday"));
    }

    #[test]
    fn resolve_json_roundtrip() {
        let json = r#"{"tool":"reminder.add","params":{"title":"Test"},"now":1754000000}"#;
        let out = resolve_json(json);
        assert!(out.contains("\"outcome\":\"planned\""));
        assert!(out.contains("add_reminder"));
    }
}
