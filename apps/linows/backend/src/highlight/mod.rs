/// Syntax highlighting module for file preview.
/// Ported from macOS `SyntaxHighlighter.swift` - same tokenizer logic. The
/// tokenizer hands the shell spans and the shell draws the text runs.
///
/// Structure:
///   lang.rs      - language detection + keyword/comment definitions
///   tokenizer.rs - byte-level tokenizer producing spans
mod lang;
mod tokenizer;

use lang::Language;

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

fn file_extension(path: &str) -> String {
    path.rsplit('.').next().unwrap_or("").to_ascii_lowercase()
}

fn is_text_file(path: &str) -> bool {
    let ext = file_extension(path);
    TEXT_EXTENSIONS.contains(&ext.as_str())
}

/// Whether the preview draws this file as prose rather than as source.
pub fn is_markdown(path: &str) -> bool {
    MARKDOWN_EXTENSIONS.contains(&file_extension(path).as_str())
}

const MARKDOWN_EXTENSIONS: &[&str] = &["md", "markdown"];

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
    // Opening a FIFO blocks.
    if !meta.is_file() || meta.len() > size_cap(path) {
        return None;
    }
    let mut data = Vec::new();
    std::io::Read::read_to_end(
        &mut std::io::Read::take(std::fs::File::open(path).ok()?, DISPLAY_BYTE_CAP as u64 + 1),
        &mut data,
    )
    .ok()?;
    let truncated = data.len() > DISPLAY_BYTE_CAP;
    data.truncate(DISPLAY_BYTE_CAP);
    Some((data, truncated))
}

/// Token spans for text whose language a fence tag names. An unknown or
/// missing tag highlights nothing.
pub fn highlight_text(text: &str, language: Option<&str>) -> Vec<Span> {
    match language.and_then(Language::from_name) {
        Some(lang) => tokenizer::tokenize(text.as_bytes(), lang),
        None => Vec::new(),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fence_tag_names_a_language_or_nothing() {
        assert!(!highlight_text("fn main() {}", Some("rust")).is_empty());
        assert!(!highlight_text("fn main() {}", Some("rs")).is_empty());
        assert!(
            highlight_text("def", Some("python"))
                .iter()
                .any(|span| span.start == 0 && span.end == 3 && span.token == TokenType::Keyword),
            "a tag names its own language, not the first one"
        );
        assert!(highlight_text("fn main() {}", Some("brainfuck")).is_empty());
        assert!(highlight_text("fn main() {}", None).is_empty());
        assert!(is_markdown("/notes/today.md") && !is_markdown("/notes/today.rst"));
    }
}
