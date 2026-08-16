//! Choosing which meeting to open, out of the events a shell fetched.

use super::link::{JoinLink, Provider, find_join_link};

/// One calendar event as the shell hands it over. Only the fields a join link
/// can hide in, plus what it takes to pick between events.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventInput {
    pub title: String,
    pub start_unix_s: i64,
    pub end_unix_s: i64,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    /// All-day entries are excluded outright. "Conference week" spanning the
    /// whole day would otherwise outrank the standup starting in five minutes.
    #[serde(default)]
    pub all_day: bool,
}

/// The meeting to join, with everything a surface needs to render it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinableMeeting {
    pub title: String,
    pub start_unix_s: i64,
    pub end_unix_s: i64,
    pub url: String,
    pub provider: Provider,
    /// Pre-rendered so a shell never re-implements the naming.
    pub provider_label: String,
    /// Seconds until it starts; negative once it has.
    pub starts_in_s: i64,
    pub in_progress: bool,
}

/// How close to the start a meeting is worth surfacing unprompted. A proactive
/// tile earlier than this is noise, not help.
pub const IMMINENT_WINDOW_S: i64 = 15 * 60;

impl JoinableMeeting {
    /// Whether a proactive surface (a tile, a banner) should show this now.
    pub fn is_imminent(&self) -> bool {
        self.starts_in_s <= IMMINENT_WINDOW_S
    }
}

/// The meeting to join right now, out of the events the shell fetched.
///
/// A meeting already under way wins over one starting sooner-but-later, which
/// is what "join my next meeting" means when you are five minutes late. Ended
/// events, all-day entries, and anything without a join link are not
/// candidates at all. With a `name`, only meetings whose title contains all of
/// its words qualify, so "join standup" skips past the thing starting sooner.
pub fn next_joinable(
    events: &[EventInput],
    now_unix_s: i64,
    name: Option<&str>,
) -> Option<JoinableMeeting> {
    joinable_meetings(events, now_unix_s, name)
        .into_iter()
        .next()
}

/// What a `join` request found: the meetings it can open, plus the ones it
/// matched by name and could NOT open.
///
/// The second list is why this is not just a `Vec`. "No meeting matching
/// Testing" is a lie when a meeting called Testing is sitting right there
/// without a link - the honest answer names it and says what is missing.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinOutcome {
    pub meetings: Vec<JoinableMeeting>,
    /// Titles that answered to the name but carry no join link, earliest
    /// first, each named once.
    pub without_link: Vec<String>,
}

/// Every meeting that could be joined, best first. A surface that asks the
/// user to pick needs the whole list; `next_joinable` is the head of it, so
/// the order the picker shows and the one a bare "join" takes cannot diverge.
pub fn joinable_meetings(
    events: &[EventInput],
    now_unix_s: i64,
    name: Option<&str>,
) -> Vec<JoinableMeeting> {
    join_outcome(events, now_unix_s, name).meetings
}

/// The joinable meetings and the near-misses, in one pass over the events.
pub fn join_outcome(events: &[EventInput], now_unix_s: i64, name: Option<&str>) -> JoinOutcome {
    let mut candidates: Vec<&EventInput> = events
        .iter()
        .filter(|event| !event.all_day && event.end_unix_s > now_unix_s)
        .filter(|event| title_matches(&event.title, name))
        .collect();
    candidates.sort_by_key(|event| (event.start_unix_s.max(now_unix_s), event.start_unix_s));

    let mut without_link: Vec<String> = Vec::new();
    let mut found: Vec<(&EventInput, JoinLink)> = Vec::new();
    for event in candidates {
        match find_join_link(
            event.url.as_deref(),
            event.location.as_deref(),
            event.notes.as_deref(),
        ) {
            Some(link) => found.push((event, link)),
            None => {
                if !without_link.contains(&event.title) {
                    without_link.push(event.title.clone());
                }
            }
        }
    }

    // The candidates were sorted before the link lookup, so both lists come out
    // in the same order: anything in progress first, then by start time.
    let meetings = found
        .into_iter()
        .map(|(event, link)| JoinableMeeting {
            title: event.title.clone(),
            start_unix_s: event.start_unix_s,
            end_unix_s: event.end_unix_s,
            url: link.url,
            provider: link.provider,
            provider_label: link.provider.label().to_string(),
            starts_in_s: event.start_unix_s - now_unix_s,
            in_progress: event.start_unix_s <= now_unix_s,
        })
        .collect();
    JoinOutcome {
        meetings,
        without_link,
    }
}

/// Whether an event title answers to `name`: every word of the name appears in
/// it. Word containment, not fuzzy scoring - a meeting is opened, not searched,
/// so a near-miss that opens the WRONG call is worse than no row at all.
///
/// Folded through the same normalization the file search uses, or `join hop`
/// would miss a meeting called `Họp` that `hop` finds everywhere else in the
/// app.
fn title_matches(title: &str, name: Option<&str>) -> bool {
    let Some(name) = name else { return true };
    let title = look_matching::normalize_for_search(title);
    look_matching::normalize_for_search(name)
        .split_whitespace()
        .all(|word| title.contains(word))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ZOOM_NOTES: &str = r#"Alex Kim is inviting you to a scheduled Zoom meeting.

Join Zoom Meeting
https://us02web.zoom.us/j/89123456789?pwd=Q2hhbmdlTWU

Meeting ID: 891 2345 6789
One tap mobile
+16465588656,,89123456789# US (New York)
Find your local number: https://us02web.zoom.us/u/kbXyZ1
"#;

    const NOW: i64 = 1_760_000_000;
    const MINUTE: i64 = 60;

    fn event(title: &str, starts_in_min: i64, minutes: i64, link: Option<&str>) -> EventInput {
        EventInput {
            title: title.to_string(),
            start_unix_s: NOW + starts_in_min * MINUTE,
            end_unix_s: NOW + (starts_in_min + minutes) * MINUTE,
            url: link.map(str::to_string),
            location: None,
            notes: None,
            all_day: false,
        }
    }

    #[test]
    fn a_meeting_already_running_beats_one_starting_sooner() {
        let events = [
            event(
                "Standup",
                2,
                15,
                Some("https://meet.google.com/abc-defg-hij"),
            ),
            event("Design review", -10, 45, Some("https://meet.jit.si/design")),
        ];
        let next = next_joinable(&events, NOW, None).expect("a meeting");
        assert_eq!(next.title, "Design review");
        assert!(next.in_progress);
        assert_eq!(next.starts_in_s, -10 * MINUTE);
    }
    #[test]
    fn the_earliest_upcoming_meeting_wins() {
        let events = [
            event("Later", 90, 30, Some("https://meet.jit.si/later")),
            event("Sooner", 20, 30, Some("https://meet.jit.si/sooner")),
        ];
        let next = next_joinable(&events, NOW, None).expect("a meeting");
        assert_eq!(next.title, "Sooner");
        assert!(!next.in_progress);
        assert_eq!(next.starts_in_s, 20 * MINUTE);
    }
    #[test]
    fn finished_events_are_not_candidates() {
        let events = [event("Over", -60, 30, Some("https://meet.jit.si/over"))];
        assert_eq!(next_joinable(&events, NOW, None), None);
    }
    #[test]
    fn events_without_a_link_are_not_candidates() {
        let events = [
            event("Desk work", 5, 60, None),
            event("Sync", 30, 30, Some("https://meet.jit.si/sync")),
        ];
        let next = next_joinable(&events, NOW, None).expect("a meeting");
        assert_eq!(next.title, "Sync");
    }
    #[test]
    fn an_all_day_entry_never_wins() {
        let mut all_day = event("Offsite", -120, 600, Some("https://meet.jit.si/offsite"));
        all_day.all_day = true;
        let events = [
            all_day,
            event("Standup", 10, 15, Some("https://meet.jit.si/standup")),
        ];
        let next = next_joinable(&events, NOW, None).expect("a meeting");
        assert_eq!(next.title, "Standup");
    }
    #[test]
    fn imminent_covers_in_progress_and_the_quarter_hour_before() {
        let soon = next_joinable(
            &[event("Soon", 10, 30, Some("https://meet.jit.si/soon"))],
            NOW,
            None,
        )
        .expect("a meeting");
        assert!(soon.is_imminent());

        let later = next_joinable(
            &[event("Later", 40, 30, Some("https://meet.jit.si/later"))],
            NOW,
            None,
        )
        .expect("a meeting");
        assert!(!later.is_imminent());

        let running = next_joinable(
            &[event(
                "Running",
                -5,
                30,
                Some("https://meet.jit.si/running"),
            )],
            NOW,
            None,
        )
        .expect("a meeting");
        assert!(running.is_imminent());
    }
    #[test]
    fn an_empty_calendar_has_nothing_to_join() {
        assert_eq!(next_joinable(&[], NOW, None), None);
    }
    #[test]
    fn a_named_join_skips_the_sooner_meeting() {
        let events = [
            event("Sooner", 5, 30, Some("https://meet.jit.si/sooner")),
            event("Design review", 60, 30, Some("https://meet.jit.si/design")),
        ];
        let named = next_joinable(&events, NOW, Some("design review")).expect("a meeting");
        assert_eq!(named.title, "Design review");

        // Matching is case- and order-insensitive over words, but every word
        // has to appear.
        assert!(next_joinable(&events, NOW, Some("REVIEW")).is_some());
        assert_eq!(next_joinable(&events, NOW, Some("design retro")), None);
    }
    #[test]
    fn the_list_holds_every_candidate_best_first() {
        let events = [
            event("Later", 90, 30, Some("https://meet.jit.si/later")),
            event("No link", 5, 30, None),
            event("Running", -5, 30, Some("https://meet.jit.si/running")),
            event("Soon", 20, 30, Some("https://meet.jit.si/soon")),
        ];
        let listed = joinable_meetings(&events, NOW, None);
        let titles: Vec<&str> = listed.iter().map(|m| m.title.as_str()).collect();
        assert_eq!(titles, ["Running", "Soon", "Later"]);
        // The head of the list is exactly what a bare "join" would take.
        assert_eq!(
            next_joinable(&events, NOW, None).map(|m| m.title),
            Some("Running".to_string())
        );
    }
    #[test]
    fn a_matching_meeting_without_a_link_is_reported_by_name() {
        // Two meetings called Testing, one with a link and one without: the
        // list holds the joinable one and the outcome still names the other.
        let events = [
            event(
                "Testing",
                30,
                60,
                Some("https://meet.google.com/abc-defg-hij"),
            ),
            event("Testing", 300, 60, None),
            event("Retro", 20, 30, None),
        ];
        let outcome = join_outcome(&events, NOW, Some("testing"));
        assert_eq!(outcome.meetings.len(), 1);
        assert_eq!(outcome.without_link, ["Testing"]);
        // "Retro" did not answer to the name, so it is not a near-miss.
        assert!(!outcome.without_link.contains(&"Retro".to_string()));
    }
    #[test]
    fn near_misses_are_named_once_each() {
        let events = [
            event("Standup", 10, 30, None),
            event("Standup", 60, 30, None),
        ];
        let outcome = join_outcome(&events, NOW, Some("standup"));
        assert!(outcome.meetings.is_empty());
        assert_eq!(outcome.without_link, ["Standup"]);
    }
    #[test]
    fn a_name_matches_across_diacritics() {
        // What a Vietnamese user actually types. The file search has folded
        // this way for a long time; the join tier now agrees with it.
        let events = [
            event("Họp nhóm", 30, 60, Some("https://meet.jit.si/hop")),
            event("Điện thoại", 90, 30, Some("https://meet.jit.si/dt")),
        ];
        assert_eq!(
            next_joinable(&events, NOW, Some("hop")).map(|m| m.title),
            Some("Họp nhóm".to_string())
        );
        assert_eq!(
            next_joinable(&events, NOW, Some("dien thoai")).map(|m| m.title),
            Some("Điện thoại".to_string())
        );
        // And the other direction: typing the diacritics still works.
        assert!(next_joinable(&events, NOW, Some("Họp")).is_some());
    }
    #[test]
    fn a_name_that_matches_nothing_produces_no_row() {
        // This is what keeps "join two pdfs" a file search: the words are read
        // as a name, no meeting answers to it, and the launcher shows nothing.
        let events = [event("Standup", 5, 30, Some("https://meet.jit.si/standup"))];
        assert_eq!(next_joinable(&events, NOW, Some("two pdfs")), None);
    }
    #[test]
    fn the_link_is_found_in_notes_as_well_as_the_url_field() {
        let mut in_notes = event("Standup", 5, 15, None);
        in_notes.notes = Some(ZOOM_NOTES.to_string());
        let next = next_joinable(&[in_notes], NOW, None).expect("a meeting");
        assert_eq!(next.provider, Provider::Zoom);
        assert_eq!(next.provider_label, "Zoom");
    }
}
