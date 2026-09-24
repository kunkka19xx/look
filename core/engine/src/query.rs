use crate::normalize::normalize_for_search;
use look_indexing::CandidateKind;

// Query prefixes: leading letter(s) matched case-insensitively, followed by
// `"`, `:`, or a space delimiter (`a"term`, `a:term`, `a term`).
// `rc` must be checked before `r` - see `from_input`.
// `rc` is engine-side (unlike the Swift-handled `t`/`tw`/`c`) because recency
// ordering needs the per-candidate timestamps only the engine has.

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
        let untrimmed_start = input.trim_start();

        // `rc` is checked before `r` (single-char) so it isn't swallowed by regex mode.
        if let Some(rest) = match_query_prefix(untrimmed_start, "rc") {
            return Self {
                normalized_query: normalize_for_search(rest),
                raw_query: None,
                kind_filter: None,
                is_regex: false,
                is_recent: true,
            };
        }

        if let Some(rest) = match_query_prefix(untrimmed_start, "d") {
            return Self {
                normalized_query: normalize_for_search(rest),
                raw_query: None,
                kind_filter: Some(CandidateKind::Folder),
                is_regex: false,
                is_recent: false,
            };
        }

        if let Some(rest) = match_query_prefix(untrimmed_start, "f") {
            return Self {
                normalized_query: normalize_for_search(rest),
                raw_query: None,
                kind_filter: Some(CandidateKind::File),
                is_regex: false,
                is_recent: false,
            };
        }

        if let Some(rest) = match_query_prefix(untrimmed_start, "a") {
            return Self {
                normalized_query: normalize_for_search(rest),
                raw_query: None,
                kind_filter: Some(CandidateKind::App),
                is_regex: false,
                is_recent: false,
            };
        }

        if let Some(rest) = match_query_prefix(untrimmed_start, "r") {
            return Self {
                normalized_query: String::new(),
                raw_query: Some(rest.to_string()),
                kind_filter: None,
                is_regex: true,
                is_recent: false,
            };
        }

        Self {
            normalized_query: normalize_for_search(input.trim()),
            raw_query: None,
            kind_filter: None,
            is_regex: false,
            is_recent: false,
        }
    }
}

/// Matches a prefix name (`a`, `f`, `d`, `r`, `rc`) followed by `"`, `:`, or a space delimiter.
/// Returns the remaining term (trimmed), or `None` if it does not match.
fn match_query_prefix<'a>(input: &'a str, name: &str) -> Option<&'a str> {
    let bytes = input.as_bytes();
    let name_bytes = name.as_bytes();
    let len = name_bytes.len();
    if bytes.len() < len || !bytes[..len].eq_ignore_ascii_case(name_bytes) {
        return None;
    }
    // Since `name` is ASCII and matched, `len` is guaranteed to be a char boundary.
    let rest = &input[len..];
    if rest.starts_with('"') || rest.starts_with(':') {
        return Some(rest[1..].trim());
    }
    if rest.starts_with(' ') {
        return Some(rest.trim());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::ParsedQuery;

    #[test]
    fn recent_prefix_sets_flag_and_filter_with_quotes() {
        let parsed = ParsedQuery::from_input("rc\"report");
        assert!(parsed.is_recent);
        assert!(!parsed.is_regex);
        assert_eq!(parsed.normalized_query, "report");
    }

    #[test]
    fn recent_prefix_supports_colon_and_space() {
        let parsed_colon = ParsedQuery::from_input("rc:report");
        assert!(parsed_colon.is_recent);
        assert_eq!(parsed_colon.normalized_query, "report");

        let parsed_space = ParsedQuery::from_input("rc report");
        assert!(parsed_space.is_recent);
        assert_eq!(parsed_space.normalized_query, "report");
    }

    #[test]
    fn recent_prefix_is_case_insensitive_and_allows_empty_filter() {
        let parsed = ParsedQuery::from_input("RC\"");
        assert!(parsed.is_recent);
        assert!(parsed.normalized_query.is_empty());

        let parsed_colon = ParsedQuery::from_input("RC:");
        assert!(parsed_colon.is_recent);
        assert!(parsed_colon.normalized_query.is_empty());

        let parsed_space = ParsedQuery::from_input("RC ");
        assert!(parsed_space.is_recent);
        assert!(parsed_space.normalized_query.is_empty());
    }

    #[test]
    fn app_prefix_supports_colon_and_space() {
        let parsed_quote = ParsedQuery::from_input("a\"chrome");
        assert_eq!(
            parsed_quote.kind_filter,
            Some(look_indexing::CandidateKind::App)
        );
        assert_eq!(parsed_quote.normalized_query, "chrome");

        let parsed_colon = ParsedQuery::from_input("a:chrome");
        assert_eq!(
            parsed_colon.kind_filter,
            Some(look_indexing::CandidateKind::App)
        );
        assert_eq!(parsed_colon.normalized_query, "chrome");

        let parsed_space = ParsedQuery::from_input("a chrome");
        assert_eq!(
            parsed_space.kind_filter,
            Some(look_indexing::CandidateKind::App)
        );
        assert_eq!(parsed_space.normalized_query, "chrome");
    }

    #[test]
    fn plain_words_starting_with_prefix_letters_are_not_prefixes() {
        let parsed_apple = ParsedQuery::from_input("apple");
        assert_eq!(parsed_apple.kind_filter, None);
        assert_eq!(parsed_apple.normalized_query, "apple");

        let parsed_radio = ParsedQuery::from_input("radio");
        assert!(!parsed_radio.is_regex);
        assert_eq!(parsed_radio.normalized_query, "radio");
    }

    #[test]
    fn regex_prefix_is_not_treated_as_recent() {
        let parsed = ParsedQuery::from_input("r\"foo");
        assert!(parsed.is_regex);
        assert!(!parsed.is_recent);

        let parsed_colon = ParsedQuery::from_input("r:foo");
        assert!(parsed_colon.is_regex);
        assert!(!parsed_colon.is_recent);

        let parsed_space = ParsedQuery::from_input("r foo");
        assert!(parsed_space.is_regex);
        assert!(!parsed_space.is_recent);
    }
}
