//! Natural-language file recall: turn "files I downloaded yesterday", "pdfs from
//! last week", "screenshots today" into a structured query the shell runs
//! against Spotlight. Deterministic - a small lexicon of file types, locations,
//! and date fields, plus the shared `window` date grammar for the time range.
//! Returns None for anything that is not clearly a file-recall query, so it
//! never hijacks a normal app/file launch.

use serde::Serialize;

use crate::window;

#[derive(Debug, Default, PartialEq, Serialize)]
pub struct FileQuery {
    /// Free-text left over for Spotlight name/content search ("resume", "taxes").
    pub terms: String,
    /// Category names the shell maps to UTIs: pdf, image, screenshot, movie,
    /// audio, document, spreadsheet, presentation, folder, archive.
    pub types: Vec<String>,
    /// The time range filters on modification time - the only timestamp the
    /// engine indexes ("created"/"downloaded" phrasings map here too).
    pub start: Option<i64>,
    pub end: Option<i64>,
    /// Folder hints the shell scopes to: downloads, desktop, documents.
    pub locations: Vec<String>,
}

/// Canonical file category for a kind word ("pdfs" -> "pdf"). Public so the
/// model's structured `recall` params normalize through the same lexicon as
/// the deterministic parser.
pub fn type_of(word: &str) -> Option<&'static str> {
    Some(match word {
        "pdf" | "pdfs" => "pdf",
        "image" | "images" | "photo" | "photos" | "picture" | "pictures" | "png" | "jpg"
        | "jpeg" | "heic" => "image",
        "screenshot" | "screenshots" | "screengrab" => "screenshot",
        "video" | "videos" | "movie" | "movies" | "mp4" | "mov" => "movie",
        "audio" | "song" | "songs" | "mp3" | "podcast" => "audio",
        "doc" | "docs" | "document" | "word" => "document",
        "sheet" | "sheets" | "spreadsheet" | "spreadsheets" | "excel" | "csv" | "numbers" => {
            "spreadsheet"
        }
        "presentation" | "presentations" | "slide" | "slides" | "keynote" | "powerpoint"
        | "ppt" => "presentation",
        "folder" | "folders" | "directory" | "directories" => "folder",
        "zip" | "archive" | "archives" => "archive",
        _ => return None,
    })
}

/// Canonical location hint for a place word ("downloaded" -> "downloads").
pub fn location_of(word: &str) -> Option<&'static str> {
    Some(match word {
        "download" | "downloads" | "downloaded" => "downloads",
        "desktop" => "desktop",
        "documents" => "documents", // the folder; "document"/"doc" is the type
        _ => return None,
    })
}

/// Words that are file-recall glue, not search terms.
const NOISE: &[&str] = &[
    "file",
    "files",
    "my",
    "the",
    "a",
    "an",
    "from",
    "show",
    "find",
    "me",
    "all",
    "get",
    "list",
    "give",
    "recent",
    "that",
    "i",
    "in",
    "on",
    "of",
    "with",
    "any",
    "some",
    "look",
    "for",
    "about",
    "containing",
    "named",
    "called",
    "and",
    "downloaded",
    "download",
    "downloads",
    "edited",
    "modified",
    "changed",
    "updated",
    "created",
    "made",
    "saved",
    "opened",
    "added",
    "put",
    "placed",
    "dropped",
    "search",
    "searches",
    "searching",
    "open",
    "reveal",
    "where",
    "are",
    "were",
    "is",
    "was",
    "did",
    "you",
    "your",
    "have",
    "had",
    "to",
    "please",
];

/// True when the words read as a calendar/reminder request: a schedule noun
/// anywhere, a schedule-only opening verb, or a reschedule verb with a named
/// day ("move the doc review to friday"). File-capable verbs alone never
/// qualify, so "delete the pdfs i downloaded" stays recall.
fn is_scheduling(words: &[&str]) -> bool {
    let leads = |set: &[&str]| words.first().is_some_and(|w| set.contains(w));
    // A schedule noun does not win when the request also names a file type or
    // place: "find the meeting notes pdf from friday" is a file search.
    let names_a_file = words.iter().any(|w| {
        type_of(w).is_some() || location_of(w).is_some() || matches!(*w, "file" | "files")
    });
    let schedule_noun = words.iter().any(|w| crate::lexicon::is_schedule_noun(w));
    (schedule_noun && !names_a_file)
        || words
            .first()
            .is_some_and(|w| crate::lexicon::is_schedule_verb(w))
        || (leads(&["move", "push", "shift"])
            && words.iter().any(|w| crate::lexicon::is_day_word(w)))
}

pub fn parse(query: &str, now_epoch: i64) -> Option<FileQuery> {
    let lower = query.trim().to_lowercase();
    if lower.is_empty() {
        return None;
    }
    let words: Vec<&str> = lower.split_whitespace().collect();

    // A scheduling request that merely NAMES a file type ("cancel the pdf
    // review meeting", "remind me to send the slides") is an action on a
    // target, not file recall. Veto it here or the type word below claims it
    // and the planner never sees the request.
    if is_scheduling(&words) {
        return None;
    }

    let mut types: Vec<String> = Vec::new();
    let mut locations: Vec<String> = Vec::new();
    let mut has_file_noun = false;
    let mut has_download = false;

    for &w in &words {
        if let Some(t) = type_of(w) {
            let t = t.to_string();
            if !types.contains(&t) {
                types.push(t);
            }
        }
        if let Some(l) = location_of(w) {
            let l = l.to_string();
            if !locations.contains(&l) {
                locations.push(l);
            }
        }
        match w {
            "file" | "files" => has_file_noun = true,
            "download" | "downloads" | "downloaded" => has_download = true,
            "documents" if !locations.contains(&"documents".to_string()) => {
                locations.push("documents".to_string());
            }
            "desktop" => {}
            _ => {}
        }
    }

    let window = window::query_window(&lower, now_epoch);
    let (start, end) = window
        .map(|w| (Some(w.start), Some(w.end)))
        .unwrap_or((None, None));

    // Trigger only on a clear file-recall shape, so normal launches are untouched:
    // an explicit type/file-noun/download, or (a location or time) with either.
    let triggered = !types.is_empty()
        || has_file_noun
        || has_download
        || (!locations.is_empty() && start.is_some());
    if !triggered {
        return None;
    }

    let terms: Vec<&str> = words
        .iter()
        .copied()
        .filter(|w| {
            type_of(w).is_none()
                && location_of(w).is_none()
                && !NOISE.contains(w)
                // Time words the `window` grammar consumes (shared lexicon).
                && !crate::lexicon::is_date_word(w)
                && w.len() > 1
        })
        .collect();

    Some(FileQuery {
        terms: terms.join(" "),
        types,
        start,
        end,
        locations,
    })
}

pub fn parse_json(query: &str, now_epoch: i64) -> Option<String> {
    parse(query, now_epoch).map(|q| serde_json::to_string(&q).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2026-01-15 12:00 UTC-ish anchor; window grammar only needs a plausible now.
    const NOW: i64 = 1_768_000_000;

    #[test]
    fn downloaded_yesterday_sets_location_and_time() {
        let q = parse("files i downloaded yesterday", NOW).unwrap();
        assert!(q.locations.contains(&"downloads".to_string()));
        assert!(q.start.is_some() && q.end.is_some());
        assert!(q.terms.is_empty());
    }

    #[test]
    fn added_to_desktop_is_glue_not_a_term() {
        let q = parse("files added to desktop", NOW).unwrap();
        assert!(q.locations.contains(&"desktop".to_string()));
        assert!(q.terms.is_empty());
    }

    #[test]
    fn pdfs_from_last_week() {
        let q = parse("pdfs from last week", NOW).unwrap();
        assert_eq!(q.types, vec!["pdf"]);
        assert!(q.start.is_some());
    }

    #[test]
    fn plural_units_and_abbreviations_stay_out_of_terms() {
        // Shared-lexicon fix: "weeks" and "tmr" weren't in the old TIME_NOISE
        // list, so they leaked into terms and filtered everything out.
        let weeks = parse("pdfs from the last 2 weeks", NOW).unwrap();
        assert!(weeks.terms.is_empty(), "terms: {:?}", weeks.terms);
        let tmr = parse("screenshots tmr", NOW).unwrap();
        assert!(tmr.terms.is_empty(), "terms: {:?}", tmr.terms);
    }

    #[test]
    fn screenshots_today() {
        let q = parse("screenshots today", NOW).unwrap();
        assert_eq!(q.types, vec!["screenshot"]);
    }

    #[test]
    fn free_terms_survive_for_content_search() {
        let q = parse("pdf about taxes", NOW).unwrap();
        assert_eq!(q.types, vec!["pdf"]);
        assert_eq!(q.terms, "taxes");
    }

    #[test]
    fn scheduling_requests_keep_their_type_words() {
        // A file type inside an event or reminder name must not divert the
        // request to file recall - the planner owns these.
        for query in [
            "cancel the pdf review meeting",
            "move the design doc review to friday",
            "remind me to send the slides tomorrow",
            "block 2 hours to review the slides",
            "delete the gym reminder",
            "reschedule the screenshot walkthrough",
        ] {
            assert!(parse(query, NOW).is_none(), "{query}");
        }
    }

    /// A file query that mentions a meeting is still a file query. The veto
    /// exists to protect scheduling requests, not to claim every sentence with
    /// the word "meeting" in it.
    #[test]
    fn a_file_type_beats_a_passing_schedule_noun() {
        for query in [
            "find the meeting notes pdf from friday",
            "the standup screenshots",
            "files from the meeting",
        ] {
            assert!(parse(query, NOW).is_some(), "{query}");
        }
        // A LEADING schedule verb still wins: this is an action, not a search.
        assert!(parse("cancel the meeting notes pdf review", NOW).is_none());
    }

    #[test]
    fn file_capable_verbs_still_recall() {
        // "delete"/"move"/"open" govern files too, so only a schedule noun
        // takes these away from recall.
        for query in [
            "delete the pdfs i downloaded yesterday",
            "open the screenshots from today",
            "move the invoice pdf to desktop",
        ] {
            assert!(parse(query, NOW).is_some(), "{query}");
        }
    }

    #[test]
    fn normal_launches_do_not_trigger() {
        assert!(parse("safari", NOW).is_none());
        assert!(parse("invoice", NOW).is_none());
        assert!(parse("visual studio code", NOW).is_none());
        // A bare time word alone is too ambiguous to hijack search.
        assert!(parse("yesterday", NOW).is_none());
    }
}
