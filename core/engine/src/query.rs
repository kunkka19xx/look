use crate::normalize::normalize_for_search;
use look_indexing::CandidateKind;

// Query prefixes: leading letter(s) matched case-insensitively, trailing `"`
// exactly (see `strip_query_prefix`). `rc"` must be checked before `r"` - see
// `from_input`. `rc"` is engine-side (unlike the Swift-handled `t"`/`tw"`/`c"`)
// because recency ordering needs the per-candidate timestamps only the engine has.
const PREFIX_APPS: &[u8] = b"a\"";
const PREFIX_FILES: &[u8] = b"f\"";
const PREFIX_FOLDERS: &[u8] = b"d\"";
const PREFIX_REGEX: &[u8] = b"r\"";
const RECENT_PREFIX: &[u8] = b"rc\"";

#[derive(Clone, Debug)]
pub(crate) struct ParsedQuery {
    pub(crate) normalized_query: String,
    pub(crate) raw_query: Option<String>,
    pub(crate) kind_filter: Option<CandidateKind>,
    pub(crate) is_regex: bool,
    pub(crate) is_recent: bool,
}

impl ParsedQuery {
    pub(crate) fn from_input(input: &str) -> Self {
        let trimmed = input.trim();

        // `rc"` is checked before `r"` (single-char) - they can't collide since
        // `r"` requires the 2nd byte to be `"`, which is `c` here.
        if let Some(rest) = strip_query_prefix(trimmed, RECENT_PREFIX) {
            return Self {
                normalized_query: normalize_for_search(rest),
                raw_query: None,
                kind_filter: None,
                is_regex: false,
                is_recent: true,
            };
        }

        if let Some(rest) = strip_query_prefix(trimmed, PREFIX_FOLDERS) {
            return Self {
                normalized_query: normalize_for_search(rest),
                raw_query: None,
                kind_filter: Some(CandidateKind::Folder),
                is_regex: false,
                is_recent: false,
            };
        }

        if let Some(rest) = strip_query_prefix(trimmed, PREFIX_FILES) {
            return Self {
                normalized_query: normalize_for_search(rest),
                raw_query: None,
                kind_filter: Some(CandidateKind::File),
                is_regex: false,
                is_recent: false,
            };
        }

        if let Some(rest) = strip_query_prefix(trimmed, PREFIX_APPS) {
            return Self {
                normalized_query: normalize_for_search(rest),
                raw_query: None,
                kind_filter: Some(CandidateKind::App),
                is_regex: false,
                is_recent: false,
            };
        }

        if let Some(rest) = strip_query_prefix(trimmed, PREFIX_REGEX) {
            return Self {
                normalized_query: String::new(),
                raw_query: Some(rest.to_string()),
                kind_filter: None,
                is_regex: true,
                is_recent: false,
            };
        }

        Self {
            normalized_query: normalize_for_search(trimmed),
            raw_query: None,
            kind_filter: None,
            is_regex: false,
            is_recent: false,
        }
    }
}

/// Strips a `…"` query prefix, returning the trimmed text after it (or `None`
/// when `input` doesn't start with `prefix`). The prefix is matched ASCII
/// case-insensitively - the trailing `"` has no case, so it matches exactly.
/// `prefix` is all-ASCII, so its length is always a UTF-8 char boundary in a
/// matched input.
fn strip_query_prefix<'a>(input: &'a str, prefix: &[u8]) -> Option<&'a str> {
    let bytes = input.as_bytes();
    if bytes.len() < prefix.len() || !bytes[..prefix.len()].eq_ignore_ascii_case(prefix) {
        return None;
    }
    Some(input[prefix.len()..].trim())
}

#[cfg(test)]
mod tests {
    use super::ParsedQuery;

    #[test]
    fn recent_prefix_sets_flag_and_filter() {
        let parsed = ParsedQuery::from_input("rc\"report");
        assert!(parsed.is_recent);
        assert!(!parsed.is_regex);
        assert_eq!(parsed.normalized_query, "report");
    }

    #[test]
    fn recent_prefix_is_case_insensitive_and_allows_empty_filter() {
        let parsed = ParsedQuery::from_input("RC\"");
        assert!(parsed.is_recent);
        assert!(parsed.normalized_query.is_empty());
    }

    #[test]
    fn regex_prefix_is_not_treated_as_recent() {
        let parsed = ParsedQuery::from_input("r\"foo");
        assert!(parsed.is_regex);
        assert!(!parsed.is_recent);
    }
}
