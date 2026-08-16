//! Placing a call from a typed line: "call mom", "facetime sarah",
//! "message alex on iphone".
//!
//! Like the meeting join tier, this needs no API and no network - macOS reaches
//! FaceTime, the phone, and Messages through URL schemes. Only two things are
//! genuinely shared across shells and so live here: reading the intent out of
//! the words, and turning a handle into the URL that dials it. Finding the
//! contact belongs to the platform (Contacts on macOS), and dialling is one
//! `open`.

/// How to reach someone. Not interchangeable: FaceTime works from the Mac
/// alone, while `tel:` routes through a nearby iPhone over Continuity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Modality {
    FaceTimeAudio,
    FaceTimeVideo,
    /// Dials through the user's iPhone.
    Phone,
    Message,
}

impl Modality {
    /// What a bare "call mom" means. FaceTime audio because it is the one that
    /// works with nothing but this Mac; `tel:` silently needs an iPhone in
    /// range, which is a bad default for a launcher that promises to just work.
    pub const DEFAULT: Modality = Modality::FaceTimeAudio;

    /// Name for the UI ("Call mom · FaceTime audio").
    pub fn label(self) -> &'static str {
        match self {
            Modality::FaceTimeAudio => "FaceTime audio",
            Modality::FaceTimeVideo => "FaceTime video",
            Modality::Phone => "Call via iPhone",
            Modality::Message => "Message",
        }
    }

    /// Stable id across the FFI, matching the serde representation. Hand-written
    /// on both sides so a shell can name a modality without pulling in serde.
    pub fn id(self) -> &'static str {
        match self {
            Modality::FaceTimeAudio => "face_time_audio",
            Modality::FaceTimeVideo => "face_time_video",
            Modality::Phone => "phone",
            Modality::Message => "message",
        }
    }

    pub fn from_id(id: &str) -> Option<Modality> {
        [
            Modality::FaceTimeAudio,
            Modality::FaceTimeVideo,
            Modality::Phone,
            Modality::Message,
        ]
        .into_iter()
        .find(|modality| modality.id() == id)
    }

    /// The URL scheme that starts it.
    fn scheme(self) -> &'static str {
        match self {
            Modality::FaceTimeAudio => "facetime-audio://",
            Modality::FaceTimeVideo => "facetime://",
            Modality::Phone => "tel:",
            Modality::Message => "sms:",
        }
    }
}

/// A parsed "call ..." line: who, and how if the words said so.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallRequest {
    /// The words naming the person, for the shell to match against Contacts.
    pub name: String,
    /// None when the line did not say, so the shell applies `Modality::DEFAULT`
    /// or asks. Kept as "unsaid" rather than defaulted here: a picker wants to
    /// know the difference between "they chose audio" and "they said nothing".
    pub modality: Option<Modality>,
}

/// Verbs that open a call request, and the modality each one implies.
const VERBS: &[(&str, Option<Modality>)] = &[
    ("call", None),
    ("facetime", Some(Modality::FaceTimeVideo)),
    ("ring", Some(Modality::Phone)),
    ("phone", Some(Modality::Phone)),
    ("message", Some(Modality::Message)),
    ("text", Some(Modality::Message)),
    ("imessage", Some(Modality::Message)),
];

/// Words after the verb that name a SERVICE. These always win: "facetime
/// sarah with audio" asked for audio.
const SERVICE_WORDS: &[(&str, Modality)] = &[
    ("facetime", Modality::FaceTimeVideo),
    ("video", Modality::FaceTimeVideo),
    ("audio", Modality::FaceTimeAudio),
    ("voice", Modality::FaceTimeAudio),
    ("message", Modality::Message),
    ("text", Modality::Message),
    ("imessage", Modality::Message),
    ("sms", Modality::Message),
];

/// Words that name a DEVICE rather than a service. They only decide when the
/// verb did not: "message alex on iphone" is still a message, sent to his
/// iPhone - reading it as a phone call contradicts the word the user typed.
const DEVICE_WORDS: &[(&str, Modality)] = &[
    ("iphone", Modality::Phone),
    ("phone", Modality::Phone),
    ("mobile", Modality::Phone),
    ("cell", Modality::Phone),
];

/// Words that carry neither a name nor a modality.
const FILLER: &[&str] = &[
    "my", "the", "a", "up", "on", "by", "via", "with", "to", "using", "please", "over",
];

/// The call request in the typed text, or None when this is ordinary search.
///
/// Only the leading verb is fixed; everything after it is either a modality
/// word or part of the name. A name is REQUIRED - a bare "call" asks for
/// nobody - and a name that matches no contact shows nothing, which is what
/// keeps "call stack" a file search.
pub fn call_query(input: &str) -> Option<CallRequest> {
    let lower = input.trim().to_lowercase();
    let mut words = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty());

    let verb = words.next()?;
    let (_, implied) = VERBS.iter().find(|(candidate, _)| *candidate == verb)?;
    let mut modality = *implied;

    let mut name_words: Vec<&str> = Vec::new();
    for word in words {
        if let Some((_, service)) = SERVICE_WORDS.iter().find(|(w, _)| *w == word) {
            modality = Some(*service);
            continue;
        }
        if let Some((_, device)) = DEVICE_WORDS.iter().find(|(w, _)| *w == word) {
            // Only when nothing has been said yet, so a device never overrides
            // the verb.
            if modality.is_none() {
                modality = Some(*device);
            }
            continue;
        }
        if FILLER.contains(&word) {
            continue;
        }
        name_words.push(word);
    }

    if name_words.is_empty() {
        return None;
    }
    Some(CallRequest {
        name: name_words.join(" "),
        modality,
    })
}

/// The URL that places the call. Handles are what Contacts hands over: a phone
/// number as the user typed it into their address book, or an email/Apple ID.
pub fn call_url(modality: Modality, handle: &str) -> String {
    format!("{}{}", modality.scheme(), sanitize_handle(handle))
}

/// Strips the punctuation people put in phone numbers, which the schemes do not
/// want, while leaving an email (or any handle with a letter in it) untouched.
fn sanitize_handle(handle: &str) -> String {
    let trimmed = handle.trim();
    let looks_numeric = trimmed
        .chars()
        .all(|c| c.is_ascii_digit() || " ()-.+".contains(c));
    if !looks_numeric {
        return trimmed.to_string();
    }
    trimmed
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '+')
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(input: &str) -> CallRequest {
        call_query(input).unwrap_or_else(|| panic!("expected a call request: {input}"))
    }

    #[test]
    fn a_bare_call_names_the_person_and_leaves_how_unsaid() {
        let parsed = request("call mom");
        assert_eq!(parsed.name, "mom");
        assert_eq!(parsed.modality, None);
    }

    #[test]
    fn the_verb_can_carry_the_modality() {
        assert_eq!(
            request("facetime sarah").modality,
            Some(Modality::FaceTimeVideo)
        );
        assert_eq!(request("message alex").modality, Some(Modality::Message));
        assert_eq!(request("text alex").modality, Some(Modality::Message));
        assert_eq!(request("ring dad").modality, Some(Modality::Phone));
    }

    #[test]
    fn trailing_words_refine_the_modality_and_never_the_name() {
        for (input, expected) in [
            ("call sarah on facetime", Modality::FaceTimeVideo),
            ("call sarah by video", Modality::FaceTimeVideo),
            ("call mom on iphone", Modality::Phone),
            ("call mom on her mobile", Modality::Phone),
            ("facetime sarah with audio", Modality::FaceTimeAudio),
            ("call alex via sms", Modality::Message),
            // The verb wins over a device qualifier: this is a message to a
            // phone, not a phone call.
            ("message alex on iphone", Modality::Message),
            ("text mom on her mobile", Modality::Message),
        ] {
            let parsed = request(input);
            assert_eq!(parsed.modality, Some(expected), "for {input}");
            assert!(
                !parsed.name.contains("facetime") && !parsed.name.contains("iphone"),
                "modality words leaked into the name: {}",
                parsed.name
            );
        }
    }

    #[test]
    fn filler_is_dropped_and_the_rest_is_the_name() {
        assert_eq!(request("call up my mom please").name, "mom");
        assert_eq!(request("call sarah lee").name, "sarah lee");
        // "her" is not filler: it is rare in a name but harmless there, and a
        // list of pronouns would be a lexicon this tier does not need.
        assert_eq!(request("call mom on her mobile").name, "mom her");
    }

    #[test]
    fn a_line_naming_nobody_is_not_a_call() {
        for input in ["call", "facetime", "call up", "call on iphone", ""] {
            assert_eq!(call_query(input), None, "for {input}");
        }
    }

    #[test]
    fn a_line_that_does_not_open_with_a_call_verb_is_ordinary_search() {
        for input in ["recall mom", "calls", "calling mom", "the call", "callback"] {
            assert_eq!(call_query(input), None, "for {input}");
        }
    }

    #[test]
    fn a_name_that_matches_no_contact_is_the_shell_s_problem() {
        // "call stack" parses; it produces nothing because no contact answers
        // to "stack", exactly as "join two pdfs" produces no meeting.
        assert_eq!(request("call stack").name, "stack");
    }

    #[test]
    fn urls_use_the_scheme_each_modality_needs() {
        assert_eq!(
            call_url(Modality::FaceTimeVideo, "+15551234567"),
            "facetime://+15551234567"
        );
        assert_eq!(
            call_url(Modality::FaceTimeAudio, "+15551234567"),
            "facetime-audio://+15551234567"
        );
        assert_eq!(
            call_url(Modality::Phone, "+15551234567"),
            "tel:+15551234567"
        );
        assert_eq!(
            call_url(Modality::Message, "+15551234567"),
            "sms:+15551234567"
        );
    }

    #[test]
    fn phone_punctuation_is_stripped_but_an_email_is_not() {
        assert_eq!(
            call_url(Modality::Phone, "+1 (555) 123-4567"),
            "tel:+15551234567"
        );
        assert_eq!(
            call_url(Modality::FaceTimeVideo, "sarah@example.com"),
            "facetime://sarah@example.com"
        );
    }

    #[test]
    fn every_modality_survives_the_string_round_trip() {
        for modality in [
            Modality::FaceTimeAudio,
            Modality::FaceTimeVideo,
            Modality::Phone,
            Modality::Message,
        ] {
            assert_eq!(Modality::from_id(modality.id()), Some(modality));
            // The id must match what serde writes, or the two sides of the FFI
            // would disagree about the same value.
            let json = serde_json::to_string(&modality).expect("serializable");
            assert_eq!(json, format!("\"{}\"", modality.id()));
        }
        assert_eq!(Modality::from_id("carrier pigeon"), None);
    }

    #[test]
    fn the_default_is_the_one_that_needs_no_iphone() {
        assert_eq!(Modality::DEFAULT, Modality::FaceTimeAudio);
        assert_eq!(Modality::DEFAULT.label(), "FaceTime audio");
    }
}
