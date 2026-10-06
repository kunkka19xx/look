//! What one list row is, and how the engine's results and the synthesized
//! rows (calc, URLs, pinned folders) become one. Mirrors `rowMeta` and the
//! catalog helpers of the webview, which mirror macOS `LauncherRowView`.

use std::path::PathBuf;

use linows_backend::clipboard::{ClipboardEntry, ClipboardImageEntry};
use linows_backend::look_answers::UrlTier;
use linows_backend::look_calc::Calculation;
use linows_backend::process::ProcRow;
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
    /// A `"` menu row: the prefix goes into the field.
    Prefix(String),
    /// A `:` menu row: the command screen it names.
    Command(String),
    /// A clip from the history, back onto the clipboard.
    Clip(Clip),
    /// A copied image, back onto the clipboard.
    ClipImage(ClipImage),
    /// A running process: Enter samples its CPU, Ctrl+D kills it.
    Process(Process),
    /// A Google autocomplete row: Enter searches for it.
    WebSuggestion(String),
}

/// A text clip, with what the preview says about it.
#[derive(Clone, Debug)]
pub struct Clip {
    pub text: String,
    /// What re-copying puts on the clipboard when it differs from the text.
    pub payload: Option<String>,
    pub timestamp: u64,
    pub chars: usize,
    pub lines: usize,
}

#[derive(Clone, Debug)]
pub struct ClipImage {
    pub hash: String,
    pub width: u32,
    pub height: u32,
    pub bytes: usize,
    pub timestamp: u64,
}

#[derive(Clone, Debug)]
pub struct Process {
    pub pid: u32,
    pub name: String,
    pub ports: Vec<u16>,
    /// The `.desktop` file when the process belongs to an app.
    pub icon_path: Option<String>,
}

/// Where the row's picture comes from.
#[derive(Clone, Debug)]
pub enum Icon {
    /// The backend resolves it from the row's kind and path; a glyph stands
    /// in until it lands, and stays on a miss.
    Resolve { glyph: &'static str },
    /// A fixed glyph, for rows that are not a file (URLs, the calculator).
    Glyph(&'static str),
    /// A picture on disk: a copied image's thumbnail, a declared icon file.
    File(PathBuf),
    /// A declared glyph that draws as text, an emoji.
    Text(String),
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
const PREFIX_HINT_ID: &str = "prefixhint:";
const COMMAND_HINT_ID: &str = "cmdhint:";
const CLIP_ID: &str = "clip:";
const CLIP_IMAGE_ID: &str = "clipimg:";
const PROCESS_ID: &str = "proc:";
const WEB_SUGGEST_ID: &str = "websuggest:";
pub const KIND_ACTION: &str = "action";
pub const WEB_SUGGEST_SUBTITLE: &str = "Search Google";
const IMAGE_EXTENSIONS: [&str; 8] = [
    ".png", ".jpg", ".jpeg", ".gif", ".svg", ".webp", ".bmp", ".ico",
];
pub const KIND_CLIPBOARD: &str = "clipboard";
pub const KIND_PROCESS: &str = "process";
/// The first 80 characters, as the list shows a clip.
const CLIP_TITLE_MAX_CHARS: usize = 80;
const CLIP_EMPTY_TITLE: &str = "(empty)";
/// What a clip is named after when the session will not say which app was
/// in front.
const CLIP_IMAGE_UNKNOWN_SOURCE: &str = "screen";
/// The path every clip row shares; the id is what tells them apart.
const CLIPBOARD_PATH: &str = "clipboard://history";
const CLIPBOARD_IMAGES_PATH: &str = "clipboard://images";
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

    /// A `"` menu row, carrying its own glyph.
    pub fn prefix_hint(prefix: &str, arg_hint: &str, description: &str, score: i64) -> Self {
        Self {
            id: format!("{PREFIX_HINT_ID}{prefix}"),
            kind: "app".into(),
            title: format!("{prefix}{arg_hint}"),
            context: description.to_string(),
            kind_label: String::new(),
            path: String::new(),
            score,
            icon: Icon::Glyph(crate::glyphs::SEARCH),
            open: Open::Prefix(prefix.to_string()),
            last_used: None,
        }
    }

    /// A `:` menu row.
    pub fn command_hint(id: &str, detail: &str, glyph: &'static str, score: i64) -> Self {
        Self {
            id: format!("{COMMAND_HINT_ID}{id}"),
            kind: "app".into(),
            title: id.to_string(),
            context: detail.to_string(),
            kind_label: String::new(),
            path: String::new(),
            score,
            icon: Icon::Glyph(glyph),
            open: Open::Command(id.to_string()),
            last_used: None,
        }
    }

    /// One clip from the history, titled by its first line.
    pub fn clip(entry: ClipboardEntry, index: usize) -> Self {
        let mut title: String = entry
            .text
            .replace('\n', "  ")
            .chars()
            .take(CLIP_TITLE_MAX_CHARS)
            .collect();
        if title.is_empty() {
            title = CLIP_EMPTY_TITLE.to_string();
        }
        let context = format!(
            "Clipboard  \u{2022}  {} chars  \u{2022}  {} lines  \u{2022}  {}",
            entry.char_count,
            entry.line_count,
            format_short_date(entry.timestamp)
        );
        Self {
            id: format!("{CLIP_ID}{}:{index}", entry.timestamp),
            kind: KIND_CLIPBOARD.into(),
            title,
            context,
            kind_label: kind_label(KIND_CLIPBOARD).to_string(),
            path: CLIPBOARD_PATH.into(),
            score: 0,
            icon: Icon::Glyph(crate::glyphs::CLIPBOARD),
            open: Open::Clip(Clip {
                text: entry.text,
                payload: entry.payload,
                timestamp: entry.timestamp,
                chars: entry.char_count,
                lines: entry.line_count,
            }),
            last_used: None,
        }
    }

    /// A copied image, named after where it came from and when, drawn as
    /// its own thumbnail.
    pub fn clip_image(entry: ClipboardImageEntry, thumb: String) -> Self {
        let date = format_short_date(entry.timestamp);
        let source = entry.source.as_deref().unwrap_or(CLIP_IMAGE_UNKNOWN_SOURCE);
        Self {
            id: format!("{CLIP_IMAGE_ID}{}", entry.hash),
            kind: KIND_CLIPBOARD.into(),
            title: format!("Image from {source}, {date}"),
            context: format!(
                "Image  \u{2022}  {}\u{d7}{}  \u{2022}  {}  \u{2022}  {date}",
                entry.width,
                entry.height,
                crate::preview::format_size(entry.byte_size as u64)
            ),
            kind_label: kind_label("image").to_string(),
            path: CLIPBOARD_IMAGES_PATH.into(),
            score: 0,
            icon: Icon::File(PathBuf::from(thumb)),
            open: Open::ClipImage(ClipImage {
                hash: entry.hash,
                width: entry.width,
                height: entry.height,
                bytes: entry.byte_size,
                timestamp: entry.timestamp,
            }),
            last_used: None,
        }
    }

    /// A `ps"` row: the app's icon when the process has one, the chip glyph
    /// otherwise.
    pub fn process(p: ProcRow) -> Self {
        let icon = match &p.icon_source {
            Some(_) => Icon::Resolve {
                glyph: crate::glyphs::CPU,
            },
            None => Icon::Glyph(crate::glyphs::CPU),
        };
        Self {
            id: format!("{PROCESS_ID}{}", p.pid),
            kind: KIND_PROCESS.into(),
            title: p.name.clone(),
            context: process_pid_label(p.pid, &p.ports),
            kind_label: kind_label(KIND_PROCESS).to_string(),
            // The icon resolves from the app's desktop file, as the webview
            // asks for it.
            path: p.icon_source.clone().unwrap_or_default(),
            score: 0,
            icon,
            open: Open::Process(Process {
                pid: p.pid,
                name: p.name,
                ports: p.ports,
                icon_path: p.icon_source,
            }),
            last_used: None,
        }
    }

    /// A Google autocomplete row, below everything local.
    pub fn web_suggestion(text: &str, index: usize) -> Self {
        Self {
            id: format!("{WEB_SUGGEST_ID}{text}"),
            kind: "app".into(),
            title: text.to_string(),
            context: WEB_SUGGEST_SUBTITLE.to_string(),
            kind_label: String::new(),
            path: String::new(),
            score: -1 - index as i64,
            icon: Icon::Glyph(crate::glyphs::SEARCH),
            open: Open::WebSuggestion(text.to_string()),
            last_used: None,
        }
    }

    /// One row of a level: the producer's order is the score, Enter runs the
    /// block's verbs whatever the row carries.
    pub fn level_row(
        row: linows_backend::look_engine::sources::LevelRow,
        position: usize,
        total: usize,
        block_name: Option<&str>,
        block_icon: Option<&str>,
        home: Option<&str>,
    ) -> Self {
        let path = row.path.unwrap_or_default();
        let context = if path.is_empty() {
            row.subtitle.clone()
        } else {
            path_info(&path)
        };
        let mut out = Self {
            id: row.candidate_id,
            kind: KIND_ACTION.into(),
            title: row.title,
            context,
            kind_label: block_name.unwrap_or("Action").to_string(),
            path,
            score: (total - position) as i64,
            icon: Icon::Glyph(crate::glyphs::ZAP),
            open: Open::Path { usage: "open_file" },
            last_used: None,
        };
        // The row's own icon wins over its block's.
        out.icon = declared_icon(&out, row.icon.as_deref().or(block_icon), home);
        out
    }

    /// A block's row from the index: the kind label says which block, and
    /// the icon is what the block declared.
    pub fn dress_source(
        &mut self,
        block_name: Option<&str>,
        block_icon: Option<&str>,
        home: Option<&str>,
    ) {
        if let Some(name) = block_name {
            self.kind_label = name.to_string();
        }
        self.context = if self.path.is_empty() {
            self.context.clone()
        } else {
            path_info(&self.path)
        };
        self.icon = declared_icon(self, block_icon, home);
    }

    /// Whether the row IS the file it names: a branch row is `main`, whose
    /// path is the repo every branch shares, so that folder's icon would lie.
    pub fn is_its_path(&self) -> bool {
        self.path
            .rsplit(['/', '\\'])
            .next()
            .is_some_and(|last| last.eq_ignore_ascii_case(&self.title))
    }

    /// The kind the icon pipeline resolves by: a process wears its app's
    /// icon, found through the desktop file in its path.
    pub fn icon_kind(&self) -> &str {
        match self.open {
            Open::Process(_) => "app",
            _ => &self.kind,
        }
    }

    /// Whether side actions apply: a menu row has nothing behind it.
    pub fn is_hint(&self) -> bool {
        matches!(self.open, Open::Prefix(_) | Open::Command(_))
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

/// What a block row draws as: a declared image file, a declared text glyph,
/// the file it names, or the bolt that says Enter performs steps. A macOS
/// symbol name has no counterpart here and falls through.
fn declared_icon(row: &Row, declared: Option<&str>, home: Option<&str>) -> Icon {
    let declared = declared.map(str::trim).filter(|d| !d.is_empty());
    if let Some(declared) = declared {
        let lowered = declared.to_lowercase();
        let is_path =
            declared.starts_with('/') || declared.starts_with("~/") || declared.starts_with('.');
        if is_path && IMAGE_EXTENSIONS.iter().any(|ext| lowered.ends_with(ext)) {
            let expanded = match (declared.strip_prefix("~/"), home) {
                (Some(rest), Some(home)) => format!("{home}/{rest}"),
                _ => declared.to_string(),
            };
            return Icon::File(PathBuf::from(expanded));
        }
        let symbol_name = declared
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.');
        if !symbol_name {
            return Icon::Text(declared.to_string());
        }
    }
    if row.is_its_path() {
        Icon::Resolve { glyph: GLYPH_FILE }
    } else {
        Icon::Glyph(crate::glyphs::ZAP)
    }
}

/// `PID: 1234 \u{2022} Port: 3000 8080`, shared with the kill list.
pub fn process_pid_label(pid: u32, ports: &[u16]) -> String {
    if ports.is_empty() {
        format!("PID: {pid}")
    } else {
        let ports: Vec<String> = ports.iter().map(u16::to_string).collect();
        format!("PID: {pid} \u{2022} Port: {}", ports.join(" "))
    }
}

/// "10/6/26, 9:54 AM", the webview's short locale date.
pub fn format_short_date(timestamp: u64) -> String {
    local_time(timestamp)
        .map(|t| t.format("%-m/%-d/%y, %-I:%M %p").to_string())
        .unwrap_or_default()
}

/// "Oct 6, 2026 at 9:54:12 AM", the preview's medium date.
pub fn format_medium_date(timestamp: u64) -> String {
    local_time(timestamp)
        .map(|t| t.format("%b %-d, %Y at %-I:%M:%S %p").to_string())
        .unwrap_or_default()
}

fn local_time(timestamp: u64) -> Option<chrono::DateTime<chrono::Local>> {
    use chrono::TimeZone;
    chrono::Local.timestamp_opt(timestamp as i64, 0).single()
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
    fn pid_label_lists_ports_only_when_held() {
        assert_eq!(process_pid_label(42, &[]), "PID: 42");
        assert_eq!(
            process_pid_label(42, &[3000, 8080]),
            "PID: 42 \u{2022} Port: 3000 8080"
        );
    }

    #[test]
    fn merge_keeps_local_ahead_on_ties() {
        let out = merge_by_score(vec![row("a", "file", 5)], vec![Row::url("x.com", "", 5)]);
        assert_eq!(out[0].title, "a");
    }
}
