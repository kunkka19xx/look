//! Minimal markdown segmentation for chat answers: splits fenced code blocks
//! (``` ... ```) from prose so the UI can render code monospaced and pass prose
//! through inline styling. Deliberately not a full markdown engine; works
//! mid-stream (an unclosed fence renders as code).

#[derive(Debug, PartialEq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Segment {
    Text {
        text: String,
    },
    Code {
        text: String,
        language: Option<String>,
    },
}

pub fn segments(raw: &str) -> Vec<Segment> {
    let mut out = Vec::new();
    let mut buffer: Vec<&str> = Vec::new();
    let mut in_code = false;
    let mut language: Option<String> = None;

    fn flush(
        out: &mut Vec<Segment>,
        buffer: &mut Vec<&str>,
        in_code: bool,
        language: &Option<String>,
    ) {
        let joined = buffer.join("\n");
        buffer.clear();
        if joined.trim().is_empty() {
            return;
        }
        out.push(if in_code {
            Segment::Code {
                text: joined,
                language: language.clone(),
            }
        } else {
            Segment::Text { text: joined }
        });
    }

    for line in raw.split('\n') {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("```") {
            flush(&mut out, &mut buffer, in_code, &language);
            if !in_code {
                // Opening fence may carry a language tag: ```swift
                language = rest
                    .trim()
                    .split_whitespace()
                    .next()
                    .map(|tag| tag.to_lowercase());
            } else {
                language = None;
            }
            in_code = !in_code;
        } else {
            buffer.push(line);
        }
    }
    flush(&mut out, &mut buffer, in_code, &language);
    out
}

/// JSON for the FFI/Tauri boundary: `[{"kind":"text","text":...} |
/// {"kind":"code","text":...,"language":...}]`.
pub fn segments_json(raw: &str) -> String {
    serde_json::to_string(&segments(raw)).unwrap_or_else(|_| "[]".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(s: &str) -> Segment {
        Segment::Text { text: s.into() }
    }
    fn code(s: &str, lang: Option<&str>) -> Segment {
        Segment::Code {
            text: s.into(),
            language: lang.map(Into::into),
        }
    }

    #[test]
    fn plain_text_is_one_segment() {
        assert_eq!(segments("hello world"), vec![text("hello world")]);
    }

    #[test]
    fn fenced_code_splits_into_three_segments() {
        assert_eq!(
            segments("Before\n```swift\nlet x = 1\n```\nAfter"),
            vec![
                text("Before"),
                code("let x = 1", Some("swift")),
                text("After")
            ]
        );
    }

    #[test]
    fn unclosed_fence_renders_as_code() {
        // Mid-stream: the closing fence hasn't arrived yet.
        assert_eq!(
            segments("Intro\n```\nfn main() {"),
            vec![text("Intro"), code("fn main() {", None)]
        );
    }

    #[test]
    fn multiple_code_blocks() {
        assert_eq!(
            segments("```rust\na\n```\nmiddle\n```\nb\n```"),
            vec![code("a", Some("rust")), text("middle"), code("b", None)]
        );
    }

    #[test]
    fn empty_segments_are_dropped() {
        assert!(segments("```\n```").is_empty());
        assert!(segments("\n\n").is_empty());
    }

    #[test]
    fn json_shape() {
        assert_eq!(
            segments_json("hi\n```swift\nx\n```"),
            r#"[{"kind":"text","text":"hi"},{"kind":"code","text":"x","language":"swift"}]"#
        );
    }
}
