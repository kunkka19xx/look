/// Syntax highlighting module for file preview.
/// Ported from macOS `SyntaxHighlighter.swift` - same tokenizer logic. Two
/// outputs over one tokenizer: HTML with CSS classes for the webview, and
/// the spans themselves for a shell that draws text runs.
///
/// Structure:
///   lang.rs      - language detection + keyword/comment definitions
///   tokenizer.rs - byte-level tokenizer producing spans
///   html.rs      - span-to-HTML renderer with CSS classes
mod html;
mod lang;
mod tokenizer;

use lang::Language;
use serde::Serialize;

pub use tokenizer::{Span, TokenType};

/// Text extensions eligible for syntax preview.
/// Matches macOS `QuickLookPreviewService.textExtensions`.
const TEXT_EXTENSIONS: &[&str] = &[
    "txt", "md", "markdown", "rst", "log", "csv", "tsv", "json", "yaml", "yml", "toml", "ini",
    "conf", "cfg", "env", "xml", "html", "htm", "css", "scss", "sass", "less", "js", "mjs", "cjs",
    "ts", "tsx", "jsx", "py", "rb", "go", "rs", "swift", "c", "cc", "cpp", "cxx", "h", "hh", "hpp",
    "hxx", "m", "mm", "java", "kt", "kts", "scala", "groovy", "sh", "bash", "zsh", "fish", "sql",
    "lua", "php", "pl", "r", "clj", "ex", "exs", "erl", "hs", "ml", "fs", "fsx", "dart", "vue",
    "svelte", "zig", "nim", "v", "odin",
];

/// Size caps - matches macOS `QuickLookPreviewService`.
const TEXT_FILE_SIZE_CAP: u64 = 512 * 1024; // 512 KB
const DEFAULT_FILE_SIZE_CAP: u64 = 20 * 1024 * 1024; // 20 MB

/// Display cap - matches macOS `TextFilePreview.displayByteCap`.
const DISPLAY_BYTE_CAP: usize = 64 * 1024; // 64 KB

#[derive(Serialize)]
pub struct HighlightResult {
    pub html: String,
    pub truncated: bool,
}

fn file_extension(path: &str) -> String {
    path.rsplit('.').next().unwrap_or("").to_ascii_lowercase()
}

fn is_text_file(path: &str) -> bool {
    let ext = file_extension(path);
    TEXT_EXTENSIONS.contains(&ext.as_str())
}

fn size_cap(path: &str) -> u64 {
    if is_text_file(path) {
        TEXT_FILE_SIZE_CAP
    } else {
        DEFAULT_FILE_SIZE_CAP
    }
}

/// The file's text with its token spans, for a shell that styles runs itself.
/// Spans are byte offsets into `text`; a file that is not valid UTF-8 keeps
/// its text (lossily) and loses the spans, since the offsets would no longer
/// line up.
pub struct HighlightRuns {
    pub text: String,
    pub spans: Vec<Span>,
    pub truncated: bool,
}

/// The bytes the preview shows: the file up to the display cap. `None` if the
/// file is missing, too large, or not a text file.
fn preview_bytes(path: &str) -> Option<(Vec<u8>, bool)> {
    if !is_text_file(path) {
        return None;
    }
    let meta = std::fs::metadata(path).ok()?;
    if meta.len() > size_cap(path) {
        return None;
    }
    let mut data = std::fs::read(path).ok()?;
    let truncated = data.len() > DISPLAY_BYTE_CAP;
    data.truncate(DISPLAY_BYTE_CAP);
    Some((data, truncated))
}

/// Read a file and return syntax-highlighted HTML.
/// Returns `None` if the file is missing, too large, or not a text file.
pub fn highlight_file(path: &str) -> Option<HighlightResult> {
    let (data, truncated) = preview_bytes(path)?;
    let spans = tokenizer::tokenize(&data, Language::from_path(path));
    let html = html::render(&data, &spans);
    Some(HighlightResult { html, truncated })
}

/// Read a file and return its text with token spans.
pub fn highlight_runs(path: &str) -> Option<HighlightRuns> {
    let (data, truncated) = preview_bytes(path)?;
    let spans = tokenizer::tokenize(&data, Language::from_path(path));
    let (text, spans) = match String::from_utf8(data) {
        Ok(text) => (text, spans),
        Err(err) => (
            String::from_utf8_lossy(err.as_bytes()).into_owned(),
            Vec::new(),
        ),
    };
    Some(HighlightRuns {
        text,
        spans,
        truncated,
    })
}

/// Highlight a file and return HTML + truncation flag.
pub fn highlight_file_cmd(path: &str) -> Option<HighlightResult> {
    highlight_file(path)
}

/// Highlight text the app already holds, as shell.
///
/// A block's steps ARE shell (`specs/user-sources.md` §2.8), and the panel
/// shows them the way an AI answer's code is shown. Never truncated: what is
/// about to run is shown in full or the panel is lying about it.
pub fn highlight_shell_cmd(source: &str) -> HighlightResult {
    let bytes = source.as_bytes();
    let spans = tokenizer::tokenize(bytes, Language::Shell);
    HighlightResult {
        html: html::render(bytes, &spans),
        truncated: false,
    }
}
