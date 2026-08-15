//! Finding the "join" link inside a calendar event.
//!
//! A Teams, Zoom, or Meet invite already carries everything needed to join, so
//! no API and no network are involved: the link is sitting in the event's own
//! fields. Pure text logic, kept here rather than in a shell, so every platform
//! that grows a calendar source inherits the same answer.
//!
//! The hard part is not finding *a* URL. An invite body is full of them - help
//! pages, meeting options, dial-in pages, the doc someone attached - so each
//! provider is matched by its JOIN shape specifically, and everything else is
//! ignored rather than ranked.

use std::sync::LazyLock;

use regex::Regex;

/// A conferencing service we can recognise a join link for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Teams,
    Zoom,
    Meet,
    Webex,
    Jitsi,
    GoToMeeting,
    Whereby,
}

impl Provider {
    /// Name for the UI ("Join Zoom meeting").
    pub fn label(self) -> &'static str {
        match self {
            Provider::Teams => "Teams",
            Provider::Zoom => "Zoom",
            Provider::Meet => "Google Meet",
            Provider::Webex => "Webex",
            Provider::Jitsi => "Jitsi",
            Provider::GoToMeeting => "GoToMeeting",
            Provider::Whereby => "Whereby",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct JoinLink {
    /// Always absolute and https, ready to hand to the OS opener.
    pub url: String,
    pub provider: Provider,
}

/// The join link for an event, or None when it is not an online meeting.
///
/// Fields are searched in the order an organiser's intent is clearest: `url` is
/// where the provider or the organiser put the canonical link, `location` is
/// where Google and hand-made invites put it, and `notes` is last because it is
/// the field most polluted with other links.
pub fn find_join_link(
    url: Option<&str>,
    location: Option<&str>,
    notes: Option<&str>,
) -> Option<JoinLink> {
    [url, location, notes]
        .into_iter()
        .flatten()
        .find_map(first_join_link)
}

/// The earliest join link in one field. Earliest rather than "best": each
/// pattern already matches only join shapes, so position is the only sensible
/// tie-break between two providers named in the same text (a Teams invite that
/// pastes a Zoom backup link, say).
fn first_join_link(text: &str) -> Option<JoinLink> {
    patterns()
        .iter()
        .filter_map(|(provider, re)| re.find(text).map(|m| (m.start(), *provider, m.as_str())))
        .min_by_key(|(start, _, _)| *start)
        .map(|(_, provider, raw)| JoinLink {
            url: normalize(raw),
            provider,
        })
}

/// Trailing characters a URL never ends with, but the text around it often
/// does: sentence punctuation, the closing half of `<...>` or `(...)`, and the
/// quote from an HTML `href`.
const TRAILING_NOISE: &[char] = &['.', ',', ';', ':', ')', ']', '>', '"', '\'', '!', '?'];

const HTTPS_PREFIX: &str = "https://";
const HTTP_PREFIX: &str = "http://";
/// Exchange writes invite bodies as HTML, so a query string arrives entity
/// encoded. Left as-is the link still opens, but on the wrong meeting.
const ENCODED_AMPERSAND: &str = "&amp;";

fn normalize(raw: &str) -> String {
    let trimmed = raw.trim_end_matches(TRAILING_NOISE);
    let decoded = trimmed.replace(ENCODED_AMPERSAND, "&");
    let lower = decoded.to_lowercase();
    if lower.starts_with(HTTPS_PREFIX) || lower.starts_with(HTTP_PREFIX) {
        decoded
    } else {
        // Google in particular drops a bare host into `location`.
        format!("{HTTPS_PREFIX}{decoded}")
    }
}

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
/// candidates at all.
pub fn next_joinable(events: &[EventInput], now_unix_s: i64) -> Option<JoinableMeeting> {
    events
        .iter()
        .filter(|event| !event.all_day && event.end_unix_s > now_unix_s)
        .filter_map(|event| {
            find_join_link(
                event.url.as_deref(),
                event.location.as_deref(),
                event.notes.as_deref(),
            )
            .map(|link| (event, link))
        })
        // Everything in progress collapses to the same key, so the sort puts
        // all of them ahead of anything upcoming; start time breaks the tie.
        .min_by_key(|(event, _)| (event.start_unix_s.max(now_unix_s), event.start_unix_s))
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
}

/// The verb that opens a join request. Deliberately the only one: "open" and
/// "start" are already spoken for by apps and files.
const JOIN_VERB: &str = "join";

/// Words allowed to follow `join` and still mean "my next meeting". Anything
/// else makes it a search again, so "join two pdfs" stays a file query.
const JOIN_FILLER: &[&str] = &[
    "meeting", "meetings", "call", "my", "the", "a", "next", "now", "up", "in", "current",
    "standup", "zoom", "teams", "meet", "webex", "please",
];

/// Whether the typed text is asking to join a meeting.
///
/// Tier 1: a fixed grammar, no model, cheap enough for every keystroke. It has
/// to be strict because it competes with real search - the launcher must not
/// swallow "join" as a verb when the user is looking for a file called Join.
pub fn is_join_query(input: &str) -> bool {
    let lower = input.trim().to_lowercase();
    let mut words = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty());

    if words.next() != Some(JOIN_VERB) {
        return false;
    }
    words.all(|word| JOIN_FILLER.contains(&word))
}

/// What a URL may still contain after its host. NOT `\S+`: an invite body is
/// often HTML, and `href="…">Join</a>` has no whitespace to stop at, so the
/// quote and the angle brackets have to end the match themselves.
const URL_TAIL: &str = r#"[^\s"'<>]+"#;
/// The same, but allowed to be empty - for the middle of a path.
const URL_TAIL_OPTIONAL: &str = r#"[^\s"'<>]*"#;

/// One join shape per provider. `(?i)` throughout: hosts are case insensitive
/// and some clients upper-case the whole line. The scheme is optional so a bare
/// host in `location` is still found; `normalize` puts one back.
fn patterns() -> &'static [(Provider, Regex)] {
    static PATTERNS: LazyLock<Vec<(Provider, Regex)>> = LazyLock::new(|| {
        let compile = |source: &str| Regex::new(source).expect("valid");
        vec![
            // `/l/meetup-join/` is the join link; `/meet/` is the newer short
            // form. Deliberately NOT `teams.microsoft.com/...` in general, which
            // would match the "Meeting options" and "Learn more" links sitting
            // right beside it in every invite.
            (
                Provider::Teams,
                compile(&format!(
                    r"(?i)(https?://)?\b(teams\.microsoft\.com/(l/meetup-join/|meet/)|teams\.live\.com/meet/){URL_TAIL}"
                )),
            ),
            // `/j/` is a meeting, `/w/` a webinar, `/my/` a personal room.
            // `/u/` (a user profile) and `/rec/` (a recording) are not.
            (
                Provider::Zoom,
                compile(&format!(
                    r"(?i)(https?://)?\b[a-z0-9.-]*zoom\.us/(j|w|my)/{URL_TAIL}"
                )),
            ),
            (
                Provider::Meet,
                compile(&format!(
                    r"(?i)(https?://)?\bmeet\.google\.com/[a-z0-9]{{3,}}-[a-z0-9-]{{3,}}{URL_TAIL_OPTIONAL}"
                )),
            ),
            (
                Provider::Webex,
                compile(&format!(
                    r"(?i)(https?://)?\b[a-z0-9.-]*webex\.com/({URL_TAIL_OPTIONAL}/j\.php\?{URL_TAIL}|(meet|join)/{URL_TAIL})"
                )),
            ),
            (
                Provider::Jitsi,
                compile(&format!(r"(?i)(https?://)?\bmeet\.jit\.si/{URL_TAIL}")),
            ),
            (
                Provider::GoToMeeting,
                compile(&format!(
                    r"(?i)(https?://)?\b((global\.)?gotomeeting\.com/join/{URL_TAIL}|gotomeet\.me/{URL_TAIL})"
                )),
            ),
            (
                Provider::Whereby,
                compile(&format!(r"(?i)(https?://)?\bwhereby\.com/{URL_TAIL}")),
            ),
        ]
    });
    &PATTERNS
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Trimmed from a real Outlook invite: the join link is surrounded by three
    /// other microsoft.com URLs that must all lose.
    const TEAMS_NOTES: &str = r#"________________________________________________________________________________
Microsoft Teams Need help? <https://aka.ms/JoinTeamsMeeting>
Join the meeting now <https://teams.microsoft.com/l/meetup-join/19%3ameeting_NGI3@thread.v2/0?context=%7b%22Tid%22%3a%22abc%22%7d>
Meeting ID: 123 456 789
Or call in (audio only) +1 555-0100,,123456789#
Meeting options <https://teams.microsoft.com/meetingOptions/?organizerId=xyz>
"#;

    const ZOOM_NOTES: &str = r#"Alex Kim is inviting you to a scheduled Zoom meeting.

Join Zoom Meeting
https://us02web.zoom.us/j/89123456789?pwd=Q2hhbmdlTWU

Meeting ID: 891 2345 6789
One tap mobile
+16465588656,,89123456789# US (New York)
Find your local number: https://us02web.zoom.us/u/kbXyZ1
"#;

    #[test]
    fn teams_picks_the_join_link_not_help_or_options() {
        let link = find_join_link(None, None, Some(TEAMS_NOTES)).expect("a join link");
        assert_eq!(link.provider, Provider::Teams);
        assert!(
            link.url.contains("/l/meetup-join/"),
            "expected the meetup-join link, got {}",
            link.url
        );
        assert!(!link.url.contains("aka.ms"));
        assert!(!link.url.contains("meetingOptions"));
    }

    #[test]
    fn teams_join_link_keeps_its_percent_encoded_context() {
        let link = find_join_link(None, None, Some(TEAMS_NOTES)).expect("a join link");
        assert!(link.url.contains("19%3ameeting_NGI3@thread.v2/0"));
        assert!(link.url.contains("context=%7b%22Tid%22%3a%22abc%22%7d"));
        // The `<...>` wrapper is the mail client's, not part of the URL.
        assert!(!link.url.ends_with('>'));
    }

    #[test]
    fn zoom_picks_the_meeting_not_the_local_numbers_page() {
        let link = find_join_link(None, None, Some(ZOOM_NOTES)).expect("a join link");
        assert_eq!(link.provider, Provider::Zoom);
        assert_eq!(
            link.url,
            "https://us02web.zoom.us/j/89123456789?pwd=Q2hhbmdlTWU"
        );
    }

    #[test]
    fn meet_in_location_without_a_scheme_becomes_absolute() {
        let link =
            find_join_link(None, Some("meet.google.com/abc-defg-hij"), None).expect("a join link");
        assert_eq!(link.provider, Provider::Meet);
        assert_eq!(link.url, "https://meet.google.com/abc-defg-hij");
    }

    #[test]
    fn url_field_wins_over_notes() {
        let link = find_join_link(
            Some("https://meet.google.com/abc-defg-hij"),
            None,
            Some(ZOOM_NOTES),
        )
        .expect("a join link");
        assert_eq!(link.provider, Provider::Meet);
    }

    #[test]
    fn location_wins_over_notes() {
        let link = find_join_link(
            None,
            Some("Join at https://meet.google.com/abc-defg-hij"),
            Some(ZOOM_NOTES),
        )
        .expect("a join link");
        assert_eq!(link.provider, Provider::Meet);
    }

    #[test]
    fn sentence_punctuation_is_not_part_of_the_url() {
        let link = find_join_link(
            None,
            None,
            Some("Dial in at https://meet.jit.si/look-standup."),
        )
        .expect("a join link");
        assert_eq!(link.provider, Provider::Jitsi);
        assert_eq!(link.url, "https://meet.jit.si/look-standup");
    }

    #[test]
    fn html_encoded_ampersands_are_decoded() {
        let notes = r#"<a href="https://us02web.zoom.us/j/8912?pwd=abc&amp;from=addon">Join</a>"#;
        let link = find_join_link(None, None, Some(notes)).expect("a join link");
        assert_eq!(
            link.url,
            "https://us02web.zoom.us/j/8912?pwd=abc&from=addon"
        );
    }

    #[test]
    fn an_attached_document_is_not_a_meeting() {
        let notes = "Agenda: https://docs.google.com/document/d/1a2b3c/edit\nRoom 4B";
        assert_eq!(find_join_link(None, Some("Room 4B"), Some(notes)), None);
    }

    #[test]
    fn a_zoom_recording_is_not_a_join_link() {
        let notes = "Last week's recording: https://us02web.zoom.us/rec/share/abc123";
        assert_eq!(find_join_link(None, None, Some(notes)), None);
    }

    #[test]
    fn webex_matches_both_join_shapes() {
        let hosted = find_join_link(
            Some("https://acme.webex.com/acme/j.php?MTID=m123abc"),
            None,
            None,
        )
        .expect("a join link");
        assert_eq!(hosted.provider, Provider::Webex);

        let personal =
            find_join_link(Some("https://acme.webex.com/meet/alex"), None, None).expect("a link");
        assert_eq!(personal.provider, Provider::Webex);
    }

    #[test]
    fn uppercased_invites_still_match() {
        let link = find_join_link(None, Some("HTTPS://US02WEB.ZOOM.US/J/8912"), None)
            .expect("a join link");
        assert_eq!(link.provider, Provider::Zoom);
        // The original casing is preserved: the path may be case sensitive.
        assert_eq!(link.url, "HTTPS://US02WEB.ZOOM.US/J/8912");
    }

    #[test]
    fn a_lookalike_host_does_not_match() {
        assert_eq!(
            find_join_link(None, None, Some("https://notmeet.google.com/abc-defg-hij")),
            None
        );
        assert_eq!(
            find_join_link(None, None, Some("https://fakezoom.us.evil.com/j/1")),
            None
        );
    }

    #[test]
    fn earliest_link_wins_when_an_invite_names_two_services() {
        let notes = "Primary: https://meet.jit.si/look\nBackup: https://us02web.zoom.us/j/8912";
        let link = find_join_link(None, None, Some(notes)).expect("a join link");
        assert_eq!(link.provider, Provider::Jitsi);
    }

    #[test]
    fn empty_and_absent_fields_are_not_meetings() {
        assert_eq!(find_join_link(None, None, None), None);
        assert_eq!(find_join_link(Some(""), Some(""), Some("")), None);
        assert_eq!(find_join_link(None, Some("Meeting Room 3"), None), None);
    }

    #[test]
    fn remaining_providers_are_recognised() {
        for (text, expected) in [
            ("https://whereby.com/look-team", Provider::Whereby),
            ("https://gotomeet.me/alexkim", Provider::GoToMeeting),
            (
                "https://global.gotomeeting.com/join/123456789",
                Provider::GoToMeeting,
            ),
            ("https://teams.live.com/meet/9312345", Provider::Teams),
            (
                "https://teams.microsoft.com/meet/1234567890?p=xy",
                Provider::Teams,
            ),
        ] {
            let link = find_join_link(Some(text), None, None)
                .unwrap_or_else(|| panic!("no link found in {text}"));
            assert_eq!(link.provider, expected, "for {text}");
        }
    }

    #[test]
    fn labels_are_display_ready() {
        assert_eq!(Provider::Meet.label(), "Google Meet");
        assert_eq!(Provider::Teams.label(), "Teams");
    }

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
        let next = next_joinable(&events, NOW).expect("a meeting");
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
        let next = next_joinable(&events, NOW).expect("a meeting");
        assert_eq!(next.title, "Sooner");
        assert!(!next.in_progress);
        assert_eq!(next.starts_in_s, 20 * MINUTE);
    }

    #[test]
    fn finished_events_are_not_candidates() {
        let events = [event("Over", -60, 30, Some("https://meet.jit.si/over"))];
        assert_eq!(next_joinable(&events, NOW), None);
    }

    #[test]
    fn events_without_a_link_are_not_candidates() {
        let events = [
            event("Desk work", 5, 60, None),
            event("Sync", 30, 30, Some("https://meet.jit.si/sync")),
        ];
        let next = next_joinable(&events, NOW).expect("a meeting");
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
        let next = next_joinable(&events, NOW).expect("a meeting");
        assert_eq!(next.title, "Standup");
    }

    #[test]
    fn imminent_covers_in_progress_and_the_quarter_hour_before() {
        let soon = next_joinable(
            &[event("Soon", 10, 30, Some("https://meet.jit.si/soon"))],
            NOW,
        )
        .expect("a meeting");
        assert!(soon.is_imminent());

        let later = next_joinable(
            &[event("Later", 40, 30, Some("https://meet.jit.si/later"))],
            NOW,
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
        )
        .expect("a meeting");
        assert!(running.is_imminent());
    }

    #[test]
    fn an_empty_calendar_has_nothing_to_join() {
        assert_eq!(next_joinable(&[], NOW), None);
    }

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
            "join the standup",
        ] {
            assert!(is_join_query(phrasing), "expected a join query: {phrasing}");
        }
    }

    #[test]
    fn a_search_that_merely_starts_with_join_is_not_a_join_query() {
        for phrasing in [
            "join two pdfs",
            "join the tables in sql",
            "joins",
            "joint account",
            "adjoin",
            "join.pdf",
            "rejoin meeting",
            "",
        ] {
            assert!(
                !is_join_query(phrasing),
                "expected a plain search: {phrasing}"
            );
        }
    }

    #[test]
    fn the_link_is_found_in_notes_as_well_as_the_url_field() {
        let mut in_notes = event("Standup", 5, 15, None);
        in_notes.notes = Some(ZOOM_NOTES.to_string());
        let next = next_joinable(&[in_notes], NOW).expect("a meeting");
        assert_eq!(next.provider, Provider::Zoom);
        assert_eq!(next.provider_label, "Zoom");
    }
}
