use serde::{Deserialize, Serialize};

use look_indexing::{Candidate, CandidateKind};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LaunchResult {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub path: String,
    pub score: i64,
    pub action: LaunchResultAction,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LaunchResultAction {
    Open { path: String },
    OpenFolder { path: String },
    Reveal { path: String },
    WebSearch { query: String },
    Translate { text: String, target_lang: String },
}

impl From<(&Candidate, i64)> for LaunchResult {
    fn from((candidate, score): (&Candidate, i64)) -> Self {
        let action = match candidate.kind {
            CandidateKind::App => LaunchResultAction::Open {
                path: candidate.path.to_string(),
            },
            CandidateKind::File => LaunchResultAction::Open {
                path: candidate.path.to_string(),
            },
            CandidateKind::Folder => LaunchResultAction::OpenFolder {
                path: candidate.path.to_string(),
            },
        };

        Self {
            id: candidate.id.to_string(),
            kind: candidate.kind.to_string(),
            title: candidate.title.to_string(),
            subtitle: candidate.subtitle.as_deref().map(str::to_string),
            path: candidate.path.to_string(),
            score,
            action,
        }
    }
}
