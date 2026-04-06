use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum CandidateKind {
    App,
    File,
    Folder,
}

impl fmt::Display for CandidateKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CandidateKind::App => write!(f, "app"),
            CandidateKind::File => write!(f, "file"),
            CandidateKind::Folder => write!(f, "folder"),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Candidate {
    pub id: String,
    pub kind: CandidateKind,
    pub title: String,
    pub subtitle: Option<String>,
    pub path: String,
    pub use_count: u64,
    pub last_used_at_unix_s: Option<i64>,
}

impl Candidate {
    pub fn new(id: &str, kind: CandidateKind, title: &str, path: &str) -> Self {
        Self {
            id: id.to_string(),
            kind,
            title: title.to_string(),
            subtitle: Some(path.to_string()),
            path: path.to_string(),
            use_count: 0,
            last_used_at_unix_s: None,
        }
    }
}

pub trait Source {
    fn collect(&self) -> Vec<Candidate>;
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ClipboardContentType {
    Text,
    Image,
    FileList,
}

impl fmt::Display for ClipboardContentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClipboardContentType::Text => write!(f, "text"),
            ClipboardContentType::Image => write!(f, "image"),
            ClipboardContentType::FileList => write!(f, "file_list"),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClipboardItem {
    pub id: String,
    pub content_type: ClipboardContentType,
    pub content: String,
    pub preview: Option<String>,
    pub source_app: Option<String>,
    pub created_at_unix_s: i64,
    pub last_used_at_unix_s: Option<i64>,
    pub use_count: u64,
    pub pinned: bool,
}
