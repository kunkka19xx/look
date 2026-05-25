/// Syntax highlighting module for file preview.
/// Ported from macOS `SyntaxHighlighter.swift` — same tokenizer logic,
/// adapted for Tauri: outputs pre-built HTML with CSS classes.
///
/// Structure:
///   lang.rs      — language detection + keyword/comment definitions
///   tokenizer.rs — byte-level tokenizer producing spans
///   html.rs      — span-to-HTML renderer with CSS classes
mod html;
mod lang;
mod tokenizer;

use lang::Language;
use serde::Serialize;

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

/// Size caps — matches macOS `QuickLookPreviewService`.
const TEXT_FILE_SIZE_CAP: u64 = 512 * 1024; // 512 KB
const DEFAULT_FILE_SIZE_CAP: u64 = 20 * 1024 * 1024; // 20 MB

/// Display cap — matches macOS `TextFilePreview.displayByteCap`.
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

/// Read a file and return syntax-highlighted HTML.
/// Returns `None` if the file is missing, too large, or not a text file.
pub fn highlight_file(path: &str) -> Option<HighlightResult> {
    if !is_text_file(path) {
        return None;
    }

    let meta = std::fs::metadata(path).ok()?;
    if meta.len() > size_cap(path) {
        return None;
    }

    let data = std::fs::read(path).ok()?;
    let truncated = data.len() > DISPLAY_BYTE_CAP;
    let slice = if truncated {
        &data[..DISPLAY_BYTE_CAP]
    } else {
        &data
    };

    let lang = Language::from_path(path);
    let spans = tokenizer::tokenize(slice, lang);
    let html = html::render(slice, &spans);

    Some(HighlightResult { html, truncated })
}

/// Tauri command: highlight a file and return HTML + truncation flag.
#[tauri::command]
pub fn highlight_file_cmd(path: String) -> Option<HighlightResult> {
    highlight_file(&path)
}
