use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use std::sync::mpsc;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum CandidateKind {
    App,
    File,
    Folder,
    /// A row with no filesystem target: a bundle of steps, or a row a user's
    /// list or command produced. It is performed, never opened.
    Action,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateIdKind {
    App,
    File,
    Folder,
    Setting,
    /// A row from a user-declared source. Its own namespace so a source refresh
    /// prunes only its own rows, and so a row can be told apart from an
    /// identically-shaped one the file walker produced.
    Source,
}

impl CandidateIdKind {
    pub const PREFIX_APP: &'static str = "app:";
    pub const PREFIX_FILE: &'static str = "file:";
    pub const PREFIX_FOLDER: &'static str = "folder:";
    pub const PREFIX_SETTING: &'static str = "setting:";
    pub const PREFIX_SOURCE: &'static str = "src:";

    pub fn as_prefix(&self) -> &'static str {
        match self {
            CandidateIdKind::App => Self::PREFIX_APP,
            CandidateIdKind::File => Self::PREFIX_FILE,
            CandidateIdKind::Folder => Self::PREFIX_FOLDER,
            CandidateIdKind::Setting => Self::PREFIX_SETTING,
            CandidateIdKind::Source => Self::PREFIX_SOURCE,
        }
    }

    pub fn from_candidate_id(id: &str) -> Option<Self> {
        if id.starts_with(Self::PREFIX_APP) {
            Some(Self::App)
        } else if id.starts_with(Self::PREFIX_FILE) {
            Some(Self::File)
        } else if id.starts_with(Self::PREFIX_FOLDER) {
            Some(Self::Folder)
        } else if id.starts_with(Self::PREFIX_SETTING) {
            Some(Self::Setting)
        } else if id.starts_with(Self::PREFIX_SOURCE) {
            Some(Self::Source)
        } else {
            None
        }
    }

    /// The source id inside a source row id (`src:<source>:<row>`).
    pub fn source_id_of(candidate_id: &str) -> Option<&str> {
        candidate_id
            .strip_prefix(Self::PREFIX_SOURCE)?
            .split_once(':')
            .map(|(source, _)| source)
    }

    /// The namespace one block's rows share. Private: an id assembled by hand
    /// is how an encoder and its readers drift.
    fn source_row_prefix(source_id: &str) -> String {
        format!("{}{source_id}:", Self::PREFIX_SOURCE)
    }

    /// The ROW's own id inside a source row id (`src:<source>:<row>`), which is
    /// what a user's command expects `{id}` to be. Anything not namespaced is
    /// returned as-is, so a caller can pass either shape.
    ///
    /// A drilled row carries its ancestors between two marks first; the row id
    /// is everything after them, untouched.
    pub fn source_row_id_of(candidate_id: &str) -> &str {
        let Some(rest) = candidate_id
            .strip_prefix(Self::PREFIX_SOURCE)
            .and_then(|rest| rest.split_once(':'))
            .map(|(_, row)| row)
        else {
            return candidate_id;
        };

        let Some(after_mark) = rest.strip_prefix(Self::CHAIN_MARK) else {
            return rest;
        };
        // Doubled: a top-level row id, minus the mark the encoder added.
        if after_mark.starts_with(Self::CHAIN_MARK) {
            return after_mark;
        }
        match after_mark.split_once(Self::CHAIN_MARK) {
            Some((_, row)) => row,
            None => rest,
        }
    }

    /// Separators for the ancestor chain a drilled row id carries. A chain
    /// segment escapes them; the row id, which is free-form script output, is
    /// left exactly as the script wrote it.
    const CHAIN_MARK: char = '|';
    const CHAIN_SEPARATOR: char = ';';
    const CHAIN_PAIR: char = '/';

    /// The candidate id for a row of `block_id`, reached through `ancestors`
    /// (outermost first, each a block id and the row picked in it).
    ///
    /// The one encoder; `source_id_of`, `source_row_id_of` and
    /// `source_ancestors_of` are the only readers. Handing a script
    /// `src:branches:main` where it expected `main` is the bug this prevents.
    pub fn source_row_candidate_id(
        block_id: &str,
        ancestors: &[(String, String)],
        row_id: &str,
    ) -> String {
        let prefix = Self::source_row_prefix(block_id);
        if ancestors.is_empty() {
            // Doubled, not escaped: a row id opening with the mark would read
            // as a chain, and the decoder returns a slice it cannot unescape.
            return match row_id.starts_with(Self::CHAIN_MARK) {
                true => format!("{prefix}{}{row_id}", Self::CHAIN_MARK),
                false => format!("{prefix}{row_id}"),
            };
        }

        let chain = ancestors
            .iter()
            .map(|(block, row)| {
                format!(
                    "{}{}{}",
                    escape_chain(block),
                    Self::CHAIN_PAIR,
                    escape_chain(row)
                )
            })
            .collect::<Vec<_>>()
            .join(&Self::CHAIN_SEPARATOR.to_string());

        format!(
            "{prefix}{}{chain}{}{row_id}",
            Self::CHAIN_MARK,
            Self::CHAIN_MARK
        )
    }

    /// The levels a drilled row was reached through, outermost first. Empty for
    /// a top-level row.
    pub fn source_ancestors_of(candidate_id: &str) -> Vec<(String, String)> {
        let Some(chain) = Self::chain_of(candidate_id) else {
            return Vec::new();
        };
        chain
            .split(Self::CHAIN_SEPARATOR)
            .filter_map(|pair| pair.split_once(Self::CHAIN_PAIR))
            .map(|(block, row)| (unescape_chain(block), unescape_chain(row)))
            .collect()
    }

    /// The chain between the two marks, when the id carries one.
    fn chain_of(candidate_id: &str) -> Option<&str> {
        let after_mark = candidate_id
            .strip_prefix(Self::PREFIX_SOURCE)?
            .split_once(':')?
            .1
            .strip_prefix(Self::CHAIN_MARK)?;
        // Doubled means a row id beginning with the mark, not a chain.
        if after_mark.starts_with(Self::CHAIN_MARK) {
            return None;
        }
        after_mark
            .split_once(Self::CHAIN_MARK)
            .map(|(chain, _)| chain)
    }
}

/// Percent-escapes what the chain uses as structure. The escape character is
/// escaped first, or decoding a row named `100%` would be ambiguous.
fn escape_chain(segment: &str) -> String {
    segment
        .replace(CHAIN_ESCAPE, "%25")
        .replace(CandidateIdKind::CHAIN_MARK, "%7C")
        .replace(CandidateIdKind::CHAIN_PAIR, "%2F")
        .replace(CandidateIdKind::CHAIN_SEPARATOR, "%3B")
}

fn unescape_chain(segment: &str) -> String {
    segment
        .replace("%7C", "|")
        .replace("%2F", "/")
        .replace("%3B", ";")
        .replace("%25", "%")
}

/// Escaped by `escape_chain` before anything else, and unescaped last.
const CHAIN_ESCAPE: char = '%';

#[cfg(test)]
mod id_tests {
    use super::CandidateIdKind;

    #[test]
    fn a_row_id_is_what_a_users_command_sees() {
        // Handing git the whole candidate id makes it read `src:branches:main`
        // as rev:path and fail with "invalid object name 'src'".
        let id = "src:branches:326-bug-battery-on-macos";
        assert_eq!(CandidateIdKind::source_id_of(id), Some("branches"));
        assert_eq!(
            CandidateIdKind::source_row_id_of(id),
            "326-bug-battery-on-macos"
        );
    }

    #[test]
    fn a_drilled_row_keeps_its_block_and_its_own_id() {
        let id = CandidateIdKind::source_row_candidate_id(
            "scripts",
            &[("projects".into(), "animate".into())],
            "check:watch",
        );
        assert_eq!(id, "src:scripts:|projects/animate|check:watch");
        assert_eq!(CandidateIdKind::source_id_of(&id), Some("scripts"));
        assert_eq!(CandidateIdKind::source_row_id_of(&id), "check:watch");
        assert_eq!(
            CandidateIdKind::source_ancestors_of(&id),
            [("projects".to_string(), "animate".to_string())]
        );
    }

    #[test]
    fn the_same_row_under_two_parents_is_two_ids() {
        let here = CandidateIdKind::source_row_candidate_id(
            "scripts",
            &[("projects".into(), "animate".into())],
            "build",
        );
        let there = CandidateIdKind::source_row_candidate_id(
            "scripts",
            &[("projects".into(), "look".into())],
            "build",
        );
        assert_ne!(here, there, "ranking is keyed on the ancestor path");
    }

    #[test]
    fn a_deeper_chain_reads_outermost_first() {
        let id = CandidateIdKind::source_row_candidate_id(
            "pods",
            &[
                ("contexts".into(), "prod".into()),
                ("namespaces".into(), "web".into()),
            ],
            "api-7d9",
        );
        assert_eq!(id, "src:pods:|contexts/prod;namespaces/web|api-7d9");
        assert_eq!(CandidateIdKind::source_row_id_of(&id), "api-7d9");
        assert_eq!(
            CandidateIdKind::source_ancestors_of(&id),
            [
                ("contexts".to_string(), "prod".to_string()),
                ("namespaces".to_string(), "web".to_string())
            ]
        );
    }

    #[test]
    fn a_row_id_carrying_the_chain_characters_survives_the_round_trip() {
        // Script output is free-form: a branch named `feat/a|b;c` and a host
        // named `user@host:22` must come back exactly as they went in.
        for row in ["feat/a|b;c", "user@host:22", "100%", "|leading-mark"] {
            let drilled = CandidateIdKind::source_row_candidate_id(
                "child",
                &[("parent".into(), "a/b|c;d%e".into())],
                row,
            );
            assert_eq!(CandidateIdKind::source_row_id_of(&drilled), row);
            assert_eq!(
                CandidateIdKind::source_ancestors_of(&drilled),
                [("parent".to_string(), "a/b|c;d%e".to_string())]
            );

            let top = CandidateIdKind::source_row_candidate_id("child", &[], row);
            assert!(CandidateIdKind::source_ancestors_of(&top).is_empty());
            assert_eq!(CandidateIdKind::source_row_id_of(&top), row);
        }
    }

    #[test]
    fn a_top_level_id_is_unchanged_and_carries_no_ancestors() {
        let id = CandidateIdKind::source_row_candidate_id("branches", &[], "main");
        assert_eq!(id, "src:branches:main");
        assert!(CandidateIdKind::source_ancestors_of(&id).is_empty());
        assert!(CandidateIdKind::source_ancestors_of("app:safari").is_empty());
    }

    #[test]
    fn a_row_id_containing_colons_keeps_them() {
        assert_eq!(
            CandidateIdKind::source_row_id_of("src:hosts:user@host:22"),
            "user@host:22"
        );
    }

    #[test]
    fn an_unnamespaced_id_passes_through() {
        assert_eq!(CandidateIdKind::source_row_id_of("main"), "main");
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UsageAction {
    Open,
    OpenApp,
    OpenFile,
    OpenFolder,
    OpenUrl,
    Execute,
    WebSearch,
}

impl UsageAction {
    pub const OPEN: &'static str = "open";
    pub const OPEN_APP: &'static str = "open_app";
    pub const OPEN_FILE: &'static str = "open_file";
    pub const OPEN_FOLDER: &'static str = "open_folder";
    pub const OPEN_URL: &'static str = "open_url";
    pub const EXECUTE: &'static str = "execute";
    pub const WEB_SEARCH: &'static str = "web_search";

    pub fn as_str(&self) -> &'static str {
        match self {
            UsageAction::Open => Self::OPEN,
            UsageAction::OpenApp => Self::OPEN_APP,
            UsageAction::OpenFile => Self::OPEN_FILE,
            UsageAction::OpenFolder => Self::OPEN_FOLDER,
            UsageAction::OpenUrl => Self::OPEN_URL,
            UsageAction::Execute => Self::EXECUTE,
            UsageAction::WebSearch => Self::WEB_SEARCH,
        }
    }
}

impl FromStr for UsageAction {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            Self::OPEN => Ok(Self::Open),
            Self::OPEN_APP => Ok(Self::OpenApp),
            Self::OPEN_FILE => Ok(Self::OpenFile),
            Self::OPEN_FOLDER => Ok(Self::OpenFolder),
            Self::OPEN_URL => Ok(Self::OpenUrl),
            Self::EXECUTE => Ok(Self::Execute),
            Self::WEB_SEARCH => Ok(Self::WebSearch),
            _ => Err(()),
        }
    }
}

impl CandidateKind {
    pub const APP_KEY: &'static str = "app";
    pub const FILE_KEY: &'static str = "file";
    pub const FOLDER_KEY: &'static str = "folder";
    pub const ACTION_KEY: &'static str = "action";

    pub fn as_str(&self) -> &'static str {
        match self {
            CandidateKind::App => Self::APP_KEY,
            CandidateKind::File => Self::FILE_KEY,
            CandidateKind::Folder => Self::FOLDER_KEY,
            CandidateKind::Action => Self::ACTION_KEY,
        }
    }

    pub fn from_key(value: &str) -> Option<Self> {
        match value {
            Self::APP_KEY => Some(CandidateKind::App),
            Self::FILE_KEY => Some(CandidateKind::File),
            Self::FOLDER_KEY => Some(CandidateKind::Folder),
            Self::ACTION_KEY => Some(CandidateKind::Action),
            _ => None,
        }
    }
}

impl fmt::Display for CandidateKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Candidate {
    pub id: Box<str>,
    pub kind: CandidateKind,
    pub title: Box<str>,
    pub subtitle: Option<Box<str>>,
    pub path: Box<str>,
    pub use_count: u64,
    pub last_used_at_unix_s: Option<i64>,
    /// Filesystem modification time (Unix seconds), captured at index time.
    /// Lets the "recent" view surface freshly downloaded/created files the user
    /// hasn't opened through Look yet. `None` for app/settings candidates.
    pub fs_modified_at_unix_s: Option<i64>,
    /// What to draw this row as, when it asked for something. Only a declared
    /// source sets it; everything else takes its icon from the kind or the path.
    pub icon: Option<Box<str>>,
}

impl Candidate {
    pub fn new(id: &str, kind: CandidateKind, title: &str, path: &str) -> Self {
        Self {
            id: id.into(),
            kind,
            title: title.into(),
            subtitle: Some(path.into()),
            path: path.into(),
            ..Self::default()
        }
    }
}

impl Default for Candidate {
    /// Empty candidate used as a base for struct-update construction
    /// (`Candidate { id, kind, .., ..Default::default() }`). Keeps the
    /// "all the always-default fields" (`use_count`, the timestamp options) in
    /// one place so adding another such field doesn't touch every call site.
    /// `CandidateKind` deliberately has no `Default`, so it's pinned here.
    fn default() -> Self {
        Self {
            id: "".into(),
            kind: CandidateKind::File,
            title: "".into(),
            subtitle: None,
            path: "".into(),
            use_count: 0,
            last_used_at_unix_s: None,
            fs_modified_at_unix_s: None,
            icon: None,
        }
    }
}

pub trait Source {
    fn collect(&self, tx: mpsc::SyncSender<Candidate>);

    fn collect_vec(&self) -> Vec<Candidate> {
        let (tx, rx) = mpsc::sync_channel(1024);
        self.collect(tx);
        rx.into_iter().collect()
    }
}
