use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use std::sync::mpsc;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum CandidateKind {
    App,
    File,
    Folder,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateIdKind {
    App,
    File,
    Folder,
    Setting,
}

impl CandidateIdKind {
    pub const PREFIX_APP: &'static str = "app:";
    pub const PREFIX_FILE: &'static str = "file:";
    pub const PREFIX_FOLDER: &'static str = "folder:";
    pub const PREFIX_SETTING: &'static str = "setting:";

    pub fn as_prefix(&self) -> &'static str {
        match self {
            CandidateIdKind::App => Self::PREFIX_APP,
            CandidateIdKind::File => Self::PREFIX_FILE,
            CandidateIdKind::Folder => Self::PREFIX_FOLDER,
            CandidateIdKind::Setting => Self::PREFIX_SETTING,
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
        } else {
            None
        }
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

    pub fn as_str(&self) -> &'static str {
        match self {
            CandidateKind::App => Self::APP_KEY,
            CandidateKind::File => Self::FILE_KEY,
            CandidateKind::Folder => Self::FOLDER_KEY,
        }
    }

    pub fn from_key(value: &str) -> Option<Self> {
        match value {
            Self::APP_KEY => Some(CandidateKind::App),
            Self::FILE_KEY => Some(CandidateKind::File),
            Self::FOLDER_KEY => Some(CandidateKind::Folder),
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
}

impl Candidate {
    pub fn new(id: &str, kind: CandidateKind, title: &str, path: &str) -> Self {
        Self {
            id: id.into(),
            kind,
            title: title.into(),
            subtitle: Some(path.into()),
            path: path.into(),
            use_count: 0,
            last_used_at_unix_s: None,
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
