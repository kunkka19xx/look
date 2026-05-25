/// Converts tokenized spans into an HTML string with CSS classes.
/// The frontend renders this via `innerHTML` — no JS DOM manipulation needed.
///
/// CSS classes:
///   .tk-kw  → keyword   (--syntax-keyword)
///   .tk-str → string    (--syntax-string)
///   .tk-cm  → comment   (--syntax-comment)
///   .tk-num → number    (--syntax-number)
use super::tokenizer::{Span, TokenType};

fn css_class(token: TokenType) -> &'static str {
    match token {
        TokenType::Keyword => "tk-kw",
        TokenType::String => "tk-str",
        TokenType::Comment => "tk-cm",
        TokenType::Number => "tk-num",
    }
}

/// Build an HTML string from source bytes and token spans.
/// Plain text (not covered by any span) is HTML-escaped and emitted as-is.
/// Token spans are wrapped in `<span class="...">`.
pub fn render(source: &[u8], spans: &[Span]) -> String {
    let len = source.len();

    // Pre-allocate: HTML is typically ~1.3x the source size.
    let mut out = String::with_capacity(len + len / 3);
    let mut pos = 0;

    for span in spans {
        // Emit plain text before this span
        if span.start > pos {
            escape_into(&String::from_utf8_lossy(&source[pos..span.start]), &mut out);
        }

        // Emit the highlighted span
        out.push_str("<span class=\"");
        out.push_str(css_class(span.token));
        out.push_str("\">");
        escape_into(
            &String::from_utf8_lossy(&source[span.start..span.end]),
            &mut out,
        );
        out.push_str("</span>");

        pos = span.end;
    }

    // Emit trailing plain text
    if pos < len {
        escape_into(&String::from_utf8_lossy(&source[pos..]), &mut out);
    }

    out
}

/// HTML-escape a string slice into the output buffer.
/// Only escapes &, <, > — the minimum needed for safe HTML content.
fn escape_into(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
}
