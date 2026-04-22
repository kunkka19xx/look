use crate::normalize::normalize_for_search;
use look_indexing::CandidateKind;

const PREFIX_APPS: u8 = b'a';
const PREFIX_FILES: u8 = b'f';
const PREFIX_FOLDERS: u8 = b'd';
const PREFIX_REGEX: u8 = b'r';
const PREFIX_MARKER: u8 = b'"';
const PREFIX_LENGTH: usize = 2;

#[derive(Clone, Debug)]
pub(crate) struct ParsedQuery {
    pub(crate) normalized_query: String,
    pub(crate) raw_query: Option<String>,
    pub(crate) kind_filter: Option<CandidateKind>,
    pub(crate) is_regex: bool,
}

impl ParsedQuery {
    pub(crate) fn from_input(input: &str) -> Self {
        let trimmed = input.trim();

        if let Some(rest) = strip_prefixed_query(trimmed, PREFIX_FOLDERS) {
            return Self {
                normalized_query: normalize_for_search(rest),
                raw_query: None,
                kind_filter: Some(CandidateKind::Folder),
                is_regex: false,
            };
        }

        if let Some(rest) = strip_prefixed_query(trimmed, PREFIX_FILES) {
            return Self {
                normalized_query: normalize_for_search(rest),
                raw_query: None,
                kind_filter: Some(CandidateKind::File),
                is_regex: false,
            };
        }

        if let Some(rest) = strip_prefixed_query(trimmed, PREFIX_APPS) {
            return Self {
                normalized_query: normalize_for_search(rest),
                raw_query: None,
                kind_filter: Some(CandidateKind::App),
                is_regex: false,
            };
        }

        if let Some(rest) = strip_prefixed_query(trimmed, PREFIX_REGEX) {
            return Self {
                normalized_query: String::new(),
                raw_query: Some(rest.to_string()),
                kind_filter: None,
                is_regex: true,
            };
        }

        Self {
            normalized_query: normalize_for_search(trimmed),
            raw_query: None,
            kind_filter: None,
            is_regex: false,
        }
    }
}

fn strip_prefixed_query(input: &str, prefix: u8) -> Option<&str> {
    let bytes = input.as_bytes();
    if bytes.len() < PREFIX_LENGTH {
        return None;
    }

    if !bytes[0].eq_ignore_ascii_case(&prefix) || bytes[1] != PREFIX_MARKER {
        return None;
    }

    Some(input[PREFIX_LENGTH..].trim())
}
