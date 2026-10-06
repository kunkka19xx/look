//! What one list row is, and how the engine's results and the synthesized
//! rows (calc, URLs, pinned folders) become one. Mirrors `rowMeta` and the
//! catalog helpers of the webview, which mirror macOS `LauncherRowView`.

use linows_backend::look_answers::UrlTier;
use linows_backend::look_calc::Calculation;
use linows_backend::search::SearchResult;

/// What Enter does with the row.
#[derive(Clone, Debug)]
pub enum Open {
    /// The backend opens it and records the usage event named here.
    Path { usage: &'static str },
    /// The browser, then the URL history.
    Url(String),
    /// The answer goes to the clipboard, filed under its working.
    Calc { raw: String, expr: String },
}

/// Where the row's picture comes from.
#[derive(Clone, Debug)]
pub enum Icon {
    /// The backend resolves it from the row's kind and path; a glyph stands
    /// in until it lands, and stays on a miss.
    Resolve { glyph: &'static str },
    /// A fixed glyph, for rows that are not a file (URLs, the calculator).
    Glyph(&'static str),
}

#[derive(Clone, Debug)]
pub struct Row {
    pub id: String,
    pub kind: String,
    pub title: String,
    /// What the row is about: its parent folders, or a subtitle.
    pub context: String,
    /// What kind it is, one word in the right column. Empty for rows whose
    /// context says it.
    pub kind_label: String,
    pub path: String,
    pub score: i64,
    pub icon: Icon,
    pub open: Open,
    /// When the row was last opened through Look, for the preview.
    pub last_used: Option<i64>,
}

pub use crate::glyphs::{
    APP as GLYPH_APP, CALC as GLYPH_CALC, FILE as GLYPH_FILE, FOLDER as GLYPH_FOLDER,
    GLOBE as GLYPH_GLOBE,
};

const WEB_URL_ID: &str = "weburl:";
const CALC_ID: &str = "calc:";
const QUICK_FOLDER_ID: &str = "quickfolder:";
pub const WEB_URL_OPEN_SUBTITLE: &str = "Open in browser";
pub const WEB_URL_RECENT_SUBTITLE: &str = "Recently opened";
/// Pinned folders outrank everything the engine scored.
const QUICK_FOLDER_SCORE: i64 = 999_999;
const SETTINGS_SUBTITLE_PREFIXES: [&str; 2] = ["Windows Settings", "System Settings"];

fn kind_label(kind: &str) -> &str {
    match kind {
        "app" => "App",
        "file" => "File",
        "folder" => "Folder",
        "clipboard" => "Clipboard",
        "image" => "Image",
        "process" => "Process",
        "action" => "Action",
        other => other,
    }
}

/// The last three components of the row's parent directory, as on macOS
/// (`LauncherRowView.pathInfo`). Its own name is already the title; where it
/// lives is what tells ten `main.go` rows apart.
pub fn path_info(path: &str) -> String {
    let parent = match path.rfind(['/', '\\']) {
        Some(at) => &path[..at],
        None => "",
    };
    let components: Vec<&str> = parent
        .split(['/', '\\'])
        .filter(|c| !c.is_empty())
        .collect();
    let tail = components[components.len().saturating_sub(3)..].join("/");
    if tail.is_empty() {
        "/".to_string()
    } else if components.len() > 3 {
        format!(".../{tail}")
    } else {
        format!("/{tail}")
    }
}

fn usage_for(kind: &str) -> &'static str {
    match kind {
        "app" => "open_app",
        "folder" => "open_folder",
        _ => "open_file",
    }
}

impl Row {
    pub fn from_engine(r: SearchResult) -> Self {
        let subtitle = r.subtitle.unwrap_or_default();
        // Before the subtitle, because core writes the kind word into it.
        let (context, label) = if r.kind == "app" {
            (String::new(), kind_label("app").to_string())
        } else if !r.path.is_empty() && (r.kind == "file" || r.kind == "folder") {
            (path_info(&r.path), kind_label(&r.kind).to_string())
        } else if let Some(prefix) = SETTINGS_SUBTITLE_PREFIXES
            .iter()
            .find(|p| subtitle.starts_with(&format!("{p} ")))
        {
            (String::new(), prefix.to_string())
        } else if !subtitle.is_empty() {
            (subtitle, String::new())
        } else {
            (String::new(), kind_label(&r.kind).to_string())
        };
        let glyph = match r.kind.as_str() {
            "folder" => GLYPH_FOLDER,
            "file" => GLYPH_FILE,
            _ => GLYPH_APP,
        };
        Self {
            open: Open::Path {
                usage: usage_for(&r.kind),
            },
            id: r.id,
            kind: r.kind,
            title: r.title,
            context,
            kind_label: label,
            path: r.path,
            score: r.score,
            icon: Icon::Resolve { glyph },
            last_used: r.last_used_at_unix_s,
        }
    }

    /// Pinned above the results whenever the query is arithmetic. Title is
    /// the answer, context the expression it was parsed from.
    pub fn calc(expr: &str, calc: Calculation) -> Self {
        Self {
            id: format!("{CALC_ID}{}", calc.raw),
            kind: "app".into(),
            title: calc.display,
            context: format!("{expr}  \u{2022}  Enter to copy"),
            kind_label: String::new(),
            path: calc.raw.clone(),
            score: i64::MAX,
            icon: Icon::Glyph(GLYPH_CALC),
            open: Open::Calc {
                raw: calc.raw,
                expr: expr.to_string(),
            },
            last_used: None,
        }
    }

    /// "Open <url>": the live classification or a remembered address.
    pub fn url(url: &str, subtitle: &str, score: i64) -> Self {
        Self {
            id: format!("{WEB_URL_ID}{url}"),
            kind: "app".into(),
            title: url.to_string(),
            context: subtitle.to_string(),
            kind_label: String::new(),
            path: url.to_string(),
            score,
            icon: Icon::Glyph(GLYPH_GLOBE),
            open: Open::Url(url.to_string()),
            last_used: None,
        }
    }

    fn quick_folder(title: &str, path: &str) -> Self {
        let is_trash = title == "Trash" || title == "Recycle Bin";
        Self {
            id: format!("{QUICK_FOLDER_ID}{}", title.to_lowercase()),
            kind: "folder".into(),
            title: title.to_string(),
            context: if is_trash {
                "Pinned \u{b7} Ctrl+D to empty".into()
            } else {
                "Pinned home folder".into()
            },
            kind_label: String::new(),
            path: path.to_string(),
            score: QUICK_FOLDER_SCORE,
            icon: Icon::Resolve {
                glyph: GLYPH_FOLDER,
            },
            open: Open::Path {
                usage: "open_folder",
            },
            last_used: None,
        }
    }
}

const MIN_QUICK_FOLDER_PREFIX: usize = 2;

/// Pin the home folders whose name starts with the query above the results.
/// A same-titled app already won the backend's type-priority ranking, so the
/// pin lands right below it instead of displacing it.
pub fn prepend_quick_folders(
    mut rows: Vec<Row>,
    query: &str,
    folders: &[(String, String)],
) -> Vec<Row> {
    let q = query.trim().to_lowercase();
    if q.len() < MIN_QUICK_FOLDER_PREFIX {
        return rows;
    }
    let mut insert_at = 0;
    for (title, path) in folders {
        if !title.to_lowercase().starts_with(&q) || rows.iter().any(|r| r.path == *path) {
            continue;
        }
        let pinned = Row::quick_folder(title, path);
        match rows
            .iter()
            .position(|r| r.kind == "app" && r.title.eq_ignore_ascii_case(title))
        {
            None => {
                rows.insert(insert_at, pinned);
                insert_at += 1;
            }
            Some(rival) => rows.insert(rival + 1, pinned),
        }
    }
    rows
}

/// Stable score-descending merge: a remembered URL rises exactly as far as
/// its frecency earns against local matches, and on ties the local row stays
/// ahead. Mirrors macOS `LauncherView+URLResults.mergeByScore`.
pub fn merge_by_score(local: Vec<Row>, recents: Vec<Row>) -> Vec<Row> {
    if recents.is_empty() {
        return local;
    }
    let mut merged = Vec::with_capacity(local.len() + recents.len());
    let mut local = local.into_iter().peekable();
    let mut recents = recents.into_iter().peekable();
    while let (Some(l), Some(r)) = (local.peek(), recents.peek()) {
        if r.score > l.score {
            merged.extend(recents.next());
        } else {
            merged.extend(local.next());
        }
    }
    merged.extend(local);
    merged.extend(recents);
    merged
}

/// A structural match (scheme, port, path, localhost) cannot be a file or a
/// search, so it ranks top; a bare `host.tld` must never steal the default
/// slot from a real local result, so it sits after the first one.
pub fn place_url_row(url: Row, tier: UrlTier, mut ranked: Vec<Row>) -> Vec<Row> {
    let at = match tier {
        UrlTier::Structural => 0,
        _ => ranked.len().min(1),
    };
    ranked.insert(at, url);
    ranked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_info_keeps_the_last_three_parents() {
        assert_eq!(path_info("/home/k/src/look/main.go"), ".../k/src/look");
        assert_eq!(path_info("/home/k/a.txt"), "/home/k");
        assert_eq!(path_info("/a.txt"), "/");
    }

    fn row(title: &str, kind: &str, score: i64) -> Row {
        Row::from_engine(SearchResult {
            id: format!("{kind}:{title}"),
            kind: kind.into(),
            title: title.into(),
            subtitle: None,
            path: format!("/x/{title}"),
            score,
            icon: None,
            last_used_at_unix_s: None,
        })
    }

    #[test]
    fn a_pinned_folder_sits_below_its_app_namesake() {
        let rows = vec![row("Documents", "app", 10), row("notes", "file", 5)];
        let folders = vec![("Documents".to_string(), "/home/k/Documents".to_string())];
        let out = prepend_quick_folders(rows, "doc", &folders);
        assert_eq!(out[0].kind, "app");
        assert!(out[1].id.starts_with("quickfolder:"));
    }

    #[test]
    fn merge_keeps_local_ahead_on_ties() {
        let out = merge_by_score(vec![row("a", "file", 5)], vec![Row::url("x.com", "", 5)]);
        assert_eq!(out[0].title, "a");
    }
}
