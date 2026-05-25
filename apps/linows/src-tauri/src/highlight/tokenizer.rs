/// Tokenizer that produces spans from source text.
/// Ported from macOS `SyntaxHighlighter.swift` — same logic, same token rules.
use super::lang::Language;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenType {
    Keyword,
    String,
    Comment,
    Number,
}

#[derive(Debug, Clone)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub token: TokenType,
}

/// Tokenize source text for the given language.
/// Returns a list of spans for highlighted tokens only — plain text is implicit
/// (any byte range not covered by a span is plain text).
pub fn tokenize(source: &[u8], lang: Language) -> Vec<Span> {
    if lang == Language::Plain {
        return Vec::new();
    }

    let keywords: HashSet<&str> = lang.keywords().iter().copied().collect();
    let (line_cmt, block_cmt) = lang.comment_delimiters();
    let len = source.len();
    let mut spans = Vec::new();
    let mut i = 0;

    while i < len {
        // Line comment
        if let Some(pfx) = line_cmt
            && source[i..].starts_with(pfx.as_bytes())
        {
            let start = i;
            i += pfx.len();
            while i < len && source[i] != b'\n' {
                i += 1;
            }
            spans.push(Span {
                start,
                end: i,
                token: TokenType::Comment,
            });
            continue;
        }

        // Block comment
        if let Some((open, close)) = block_cmt
            && source[i..].starts_with(open.as_bytes())
        {
            let start = i;
            i += open.len();
            loop {
                if i >= len {
                    break;
                }
                if source[i..].starts_with(close.as_bytes()) {
                    i += close.len();
                    break;
                }
                i += 1;
            }
            spans.push(Span {
                start,
                end: i,
                token: TokenType::Comment,
            });
            continue;
        }

        let c = source[i];

        // String: " ' `
        if c == b'"' || c == b'\'' || c == b'`' {
            let start = i;
            i += 1;
            while i < len {
                let k = source[i];
                if k == b'\\' && i + 1 < len {
                    i += 2;
                    continue;
                }
                if k == c {
                    i += 1;
                    break;
                }
                if k == b'\n' {
                    break;
                }
                i += 1;
            }
            spans.push(Span {
                start,
                end: i,
                token: TokenType::String,
            });
            continue;
        }

        // Number
        if c.is_ascii_digit() {
            let start = i;
            while i < len {
                let k = source[i];
                if k.is_ascii_digit()
                    || k == b'.'
                    || k == b'_'
                    || k == b'x'
                    || (b'a'..=b'f').contains(&k)
                    || (b'A'..=b'F').contains(&k)
                {
                    i += 1;
                } else {
                    break;
                }
            }
            spans.push(Span {
                start,
                end: i,
                token: TokenType::Number,
            });
            continue;
        }

        // Identifier / keyword
        if c.is_ascii_alphabetic() || c == b'_' {
            let start = i;
            while i < len && (source[i].is_ascii_alphanumeric() || source[i] == b'_') {
                i += 1;
            }
            let word = std::str::from_utf8(&source[start..i]).unwrap_or("");
            if keywords.contains(word) {
                spans.push(Span {
                    start,
                    end: i,
                    token: TokenType::Keyword,
                });
            }
            continue;
        }

        i += 1;
    }

    spans
}
