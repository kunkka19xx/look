use look_indexing::UsageAction;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

const ACTION_OPEN_APP: &str = UsageAction::OPEN_APP;
const ACTION_OPEN_FILE: &str = UsageAction::OPEN_FILE;
const ACTION_OPEN_FOLDER: &str = UsageAction::OPEN_FOLDER;
const ACTION_OPEN_URL: &str = UsageAction::OPEN_URL;
const ACTION_REVEAL: &str = "reveal";
const ACTION_EXECUTE: &str = UsageAction::EXECUTE;
const ACTION_WEB_SEARCH: &str = UsageAction::WEB_SEARCH;
const ACTION_TRANSLATE: &str = "translate";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ActionKind {
    OpenApp,
    OpenFile,
    OpenFolder,
    OpenUrl,
    RevealInFinder,
    ExecuteCommand,
    WebSearch,
    Translate,
}

impl ActionKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ActionKind::OpenApp => ACTION_OPEN_APP,
            ActionKind::OpenFile => ACTION_OPEN_FILE,
            ActionKind::OpenFolder => ACTION_OPEN_FOLDER,
            ActionKind::OpenUrl => ACTION_OPEN_URL,
            ActionKind::RevealInFinder => ACTION_REVEAL,
            ActionKind::ExecuteCommand => ACTION_EXECUTE,
            ActionKind::WebSearch => ACTION_WEB_SEARCH,
            ActionKind::Translate => ACTION_TRANSLATE,
        }
    }
}

impl FromStr for ActionKind {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            ACTION_OPEN_APP => Ok(ActionKind::OpenApp),
            ACTION_OPEN_FILE => Ok(ActionKind::OpenFile),
            ACTION_OPEN_FOLDER => Ok(ActionKind::OpenFolder),
            ACTION_OPEN_URL => Ok(ActionKind::OpenUrl),
            ACTION_REVEAL => Ok(ActionKind::RevealInFinder),
            ACTION_EXECUTE => Ok(ActionKind::ExecuteCommand),
            ACTION_WEB_SEARCH => Ok(ActionKind::WebSearch),
            ACTION_TRANSLATE => Ok(ActionKind::Translate),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LaunchAction {
    pub kind: ActionKind,
    pub target: String,
    pub metadata: Option<serde_json::Value>,
}

impl LaunchAction {
    pub fn open_app(path: &str) -> Self {
        Self {
            kind: ActionKind::OpenApp,
            target: path.to_string(),
            metadata: None,
        }
    }

    pub fn open_file(path: &str) -> Self {
        Self {
            kind: ActionKind::OpenFile,
            target: path.to_string(),
            metadata: None,
        }
    }

    pub fn open_folder(path: &str) -> Self {
        Self {
            kind: ActionKind::OpenFolder,
            target: path.to_string(),
            metadata: None,
        }
    }

    pub fn open_url(url: &str) -> Self {
        Self {
            kind: ActionKind::OpenUrl,
            target: url.to_string(),
            metadata: None,
        }
    }

    pub fn web_search(query: &str) -> Self {
        Self {
            kind: ActionKind::WebSearch,
            target: query.to_string(),
            metadata: None,
        }
    }
}
