//! Markdown for every surface that shows it: the preview pane, the AI answer
//! views, and a connector's `preview` frame.
//!
//! Deliberately not CommonMark. Anything it does not recognise stays literal:
//! a stray asterisk is a blemish, a dropped line is a lie.

/// A run of inline text with one style. Styles do not nest.
#[derive(Debug, PartialEq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Span {
    Text { text: String },
    Bold { text: String },
    Italic { text: String },
    Code { text: String },
    Link { text: String, href: String },
}

#[derive(Debug, PartialEq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Block {
    Heading {
        level: u8,
        spans: Vec<Span>,
    },
    Paragraph {
        spans: Vec<Span>,
    },
    List {
        ordered: bool,
        start: usize,
        items: Vec<Vec<Span>>,
    },
    Code {
        language: Option<String>,
        text: String,
    },
}

/// Drops a leading YAML frontmatter block.
fn strip_frontmatter(raw: &str) -> &str {
    let Some(rest) = raw.strip_prefix("---") else {
        return raw;
    };
    let rest = rest.strip_prefix('\r').unwrap_or(rest);
    let Some(rest) = rest.strip_prefix('\n') else {
        return raw;
    };
    let mut at = 0;
    for line in rest.split_inclusive('\n') {
        at += line.len();
        match line.trim_end_matches(['\n', '\r']).trim_end() {
            "---" | "..." => return &rest[at..],
            _ => {}
        }
    }
    // Never closed, so it was not frontmatter.
    raw
}

pub fn blocks(raw: &str) -> Vec<Block> {
    let mut out = Vec::new();
    let mut paragraph: Vec<&str> = Vec::new();
    // Raw, not parsed: a continuation is appended before anything is read.
    let mut items: Vec<String> = Vec::new();
    let mut ordered = false;
    let mut start = 1;
    let mut code: Vec<&str> = Vec::new();
    let mut language: Option<String> = None;
    // The opening fence's length, so a longer fence can quote a shorter one.
    let mut fence: Option<usize> = None;

    for line in strip_frontmatter(raw).split('\n') {
        let line = line.trim_end_matches('\r');
        let trimmed = line.trim();

        let ticks = trimmed.len() - trimmed.trim_start_matches('`').len();
        match fence {
            // ```` quotes ```, and ```rust inside a block is code.
            Some(open) if ticks >= open && trimmed[ticks..].trim().is_empty() => {
                out.push(Block::Code {
                    language: language.take(),
                    text: code.join("\n"),
                });
                code.clear();
                fence = None;
                continue;
            }
            Some(_) => {
                code.push(line);
                continue;
            }
            None if ticks >= MIN_FENCE && !trimmed[ticks..].contains('`') => {
                close(&mut out, &mut paragraph, &mut items, ordered, start);
                language = trimmed[ticks..]
                    .split_whitespace()
                    .next()
                    .map(str::to_lowercase);
                fence = Some(ticks);
                continue;
            }
            None => {}
        }

        // A rule and a blank line both end whatever was open; neither draws.
        if trimmed.is_empty() || is_rule(trimmed) {
            close(&mut out, &mut paragraph, &mut items, ordered, start);
            continue;
        }

        if let Some((level, text)) = heading(trimmed) {
            close(&mut out, &mut paragraph, &mut items, ordered, start);
            out.push(Block::Heading {
                level,
                spans: spans(text),
            });
            continue;
        }

        if let Some((number, text)) = bullet(trimmed) {
            end_paragraph(&mut out, &mut paragraph);
            // Switching between numbered and not starts a second list.
            if !items.is_empty() && number.is_some() != ordered {
                end_list(&mut out, &mut items, ordered, start);
            }
            if items.is_empty() {
                start = number.unwrap_or(1);
            }
            ordered = number.is_some();
            items.push(text.to_string());
            continue;
        }

        // A hard-wrapped bullet is one bullet. The indent is read from
        // `line`, which still has it.
        if !items.is_empty()
            && (line.starts_with(' ') || line.starts_with('\t'))
            && let Some(last) = items.last_mut()
        {
            last.push(' ');
            last.push_str(trimmed);
            continue;
        }

        end_list(&mut out, &mut items, ordered, start);
        paragraph.push(trimmed);
    }

    if fence.is_some() && !code.is_empty() {
        out.push(Block::Code {
            language: language.take(),
            text: code.join("\n"),
        });
    }
    close(&mut out, &mut paragraph, &mut items, ordered, start);
    out
}

/// JSON for the FFI/Tauri boundary.
pub fn blocks_json(raw: &str) -> String {
    serde_json::to_string(&blocks(raw)).unwrap_or_else(|_| "[]".into())
}

fn close(
    out: &mut Vec<Block>,
    paragraph: &mut Vec<&str>,
    items: &mut Vec<String>,
    ordered: bool,
    start: usize,
) {
    end_paragraph(out, paragraph);
    end_list(out, items, ordered, start);
}

fn end_paragraph(out: &mut Vec<Block>, paragraph: &mut Vec<&str>) {
    if paragraph.is_empty() {
        return;
    }
    // Soft-wrapped lines are one paragraph, so a note reflows to the pane.
    let joined = paragraph.join(" ");
    paragraph.clear();
    out.push(Block::Paragraph {
        spans: spans(&joined),
    });
}

fn end_list(out: &mut Vec<Block>, items: &mut Vec<String>, ordered: bool, start: usize) {
    if items.is_empty() {
        return;
    }
    out.push(Block::List {
        ordered,
        start,
        items: std::mem::take(items)
            .iter()
            .map(|item| spans(item))
            .collect(),
    });
}

fn is_rule(line: &str) -> bool {
    let bare: &str = &line.replace(' ', "");
    bare.len() >= 3
        && (bare.chars().all(|c| c == '-')
            || bare.chars().all(|c| c == '*')
            || bare.chars().all(|c| c == '_'))
}

fn heading(line: &str) -> Option<(u8, &str)> {
    let level = line.len() - line.trim_start_matches('#').len();
    if level == 0 || level > 6 {
        return None;
    }
    let text = line[level..].strip_prefix(' ')?.trim();
    // `## Title ##` is the closed form, but only when a space precedes the
    // hashes: in `# C#` the hash is part of the word.
    let closed = text.trim_end_matches('#');
    Some((
        level as u8,
        match closed.ends_with(char::is_whitespace) {
            true => closed.trim_end(),
            false => text,
        },
    ))
}

/// The item's number when it is an ordered one, and its text.
fn bullet(line: &str) -> Option<(Option<usize>, &str)> {
    for marker in ['-', '*', '+'] {
        if let Some(rest) = line.strip_prefix(marker)
            && let Some(rest) = rest.strip_prefix(' ')
        {
            return Some((None, rest.trim()));
        }
    }
    // CommonMark's own cap. Past it the marker is literal text, which also
    // keeps `start` from overflowing the shell that adds an index to it.
    let digits = line.len() - line.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    if digits == 0 || digits > MAX_MARKER_DIGITS {
        return None;
    }
    let rest = &line[digits..];
    let rest = rest.strip_prefix('.').or_else(|| rest.strip_prefix(')'))?;
    Some((line[..digits].parse().ok(), rest.strip_prefix(' ')?.trim()))
}

fn spans(raw: &str) -> Vec<Span> {
    let mut out = Vec::new();
    let mut plain = String::new();
    let mut rest = raw;
    // Asked once: a link needs both, and every `[` would otherwise pay
    // for a search that cannot succeed.
    let links = raw.contains(']') && raw.contains(')');
    let mut before: Option<char> = None;
    while !rest.is_empty() {
        if let Some((span, tail)) = marked(rest, before, links) {
            if !plain.is_empty() {
                out.push(Span::Text {
                    text: std::mem::take(&mut plain),
                });
            }
            out.push(span);
            before = rest[..rest.len() - tail.len()].chars().next_back();
            rest = tail;
            continue;
        }
        let mut chars = rest.chars();
        if let Some(char) = chars.next() {
            plain.push(char);
            before = Some(char);
        }
        rest = chars.as_str();
    }
    if !plain.is_empty() {
        out.push(Span::Text { text: plain });
    }
    out
}

const MIN_FENCE: usize = 3;
const MAX_MARKER_DIGITS: usize = 9;

const MAX_LINK_PART: usize = 999;

/// One styled run at the head of `rest`, or None when the marker never closes,
/// which leaves it to be taken as literal text.
fn marked(rest: &str, before: Option<char>, links: bool) -> Option<(Span, &str)> {
    if let Some(body) = rest.strip_prefix("**") {
        let end = body.find("**")?;
        // `****` is four asterisks, not bold nothing: an empty run would
        // make the delimiters vanish.
        if end == 0 {
            return None;
        }
        return Some((
            Span::Bold {
                text: body[..end].into(),
            },
            &body[end + 2..],
        ));
    }
    if let Some(body) = rest.strip_prefix('`') {
        let end = body.find('`')?;
        if end == 0 {
            return None;
        }
        return Some((
            Span::Code {
                text: body[..end].into(),
            },
            &body[end + 1..],
        ));
    }
    for marker in ['*', '_'] {
        if let Some(body) = rest.strip_prefix(marker) {
            // `snake_case_name` is one word and `2 * 3` is arithmetic.
            if marker == '_' && before.is_some_and(char::is_alphanumeric) {
                return None;
            }
            if body.starts_with(char::is_whitespace) {
                return None;
            }
            let end = body.find(marker)?;
            if end == 0 {
                return None;
            }
            return Some((
                Span::Italic {
                    text: body[..end].into(),
                },
                &body[end + marker.len_utf8()..],
            ));
        }
    }
    if links && let Some(body) = rest.strip_prefix('[') {
        // CommonMark caps a label at 999, and unbounded this is quadratic
        // on a file full of unmatched `[`.
        let limit = body
            .char_indices()
            .nth(MAX_LINK_PART)
            .map_or(body.len(), |(at, _)| at);
        let close = body[..limit].find(']')?;
        let href = body[close + 1..].strip_prefix('(')?;
        let stop = href
            .char_indices()
            .nth(MAX_LINK_PART)
            .map_or(href.len(), |(at, _)| at);
        let end = href[..stop].find(')')?;
        return Some((
            Span::Link {
                text: body[..close].into(),
                href: href[..end].into(),
            },
            &href[end + 1..],
        ));
    }
    None
}

/// Prose and fenced code, nothing else: what a streaming chat answer needs,
/// where a block structure cannot be known until the fence closes.
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
                language = rest.split_whitespace().next().map(|tag| tag.to_lowercase());
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

    fn text(s: &str) -> Span {
        Span::Text { text: s.into() }
    }

    #[test]
    fn frontmatter_is_dropped() {
        let note = "---\ntitle: Meeting\ntags: [a]\n---\nBody line";
        assert_eq!(strip_frontmatter(note), "Body line");
        assert_eq!(
            blocks(note),
            vec![Block::Paragraph {
                spans: vec![text("Body line")]
            }]
        );
    }

    #[test]
    fn an_unclosed_frontmatter_fence_is_not_frontmatter() {
        // Otherwise a note opening with a rule loses everything after it.
        let note = "---\njust text\nmore";
        assert_eq!(strip_frontmatter(note), note);
    }

    #[test]
    fn a_leading_rule_is_not_mistaken_for_frontmatter() {
        assert_eq!(strip_frontmatter("---"), "---");
        assert!(blocks("---\n").is_empty());
    }

    #[test]
    fn headings_carry_their_level() {
        assert_eq!(
            blocks("# One\n### Three"),
            vec![
                Block::Heading {
                    level: 1,
                    spans: vec![text("One")]
                },
                Block::Heading {
                    level: 3,
                    spans: vec![text("Three")]
                },
            ]
        );
        // Seven is past the scale, so it is prose that happens to start with #.
        assert_eq!(
            blocks("####### Seven"),
            vec![Block::Paragraph {
                spans: vec![text("####### Seven")]
            }]
        );
    }

    #[test]
    fn a_trailing_hash_is_kept_unless_it_closes_the_heading() {
        // `# C#` is a language, not a closed heading.
        assert_eq!(
            blocks("# C#"),
            vec![Block::Heading {
                level: 1,
                spans: vec![text("C#")]
            }]
        );
        // The closed form has a space before the hashes.
        assert_eq!(
            blocks("## Title ##"),
            vec![Block::Heading {
                level: 2,
                spans: vec![text("Title")]
            }]
        );
    }

    #[test]
    fn an_unmatched_bracket_stays_literal() {
        assert_eq!(spans("a [b c"), vec![text("a [b c")]);
        // And a real link beside one still parses.
        assert_eq!(
            spans("[x](y) ["),
            vec![
                Span::Link {
                    text: "x".into(),
                    href: "y".into()
                },
                text(" ["),
            ]
        );
    }

    #[test]
    fn a_destination_longer_than_the_cap_is_not_a_link() {
        let long = format!("[a]({})", "x".repeat(MAX_LINK_PART + 1));
        assert_eq!(spans(&long), vec![text(&long)]);
    }

    #[test]
    fn a_label_longer_than_the_cap_is_not_a_link() {
        // Bounded so a line of unmatched `[` cannot make every one of them
        // scan to the end.
        let long = format!("[{}](h)", "x".repeat(MAX_LINK_PART + 1));
        assert_eq!(spans(&long), vec![text(&long)]);
        let ok = format!("[{}](h)", "x".repeat(8));
        assert_eq!(
            spans(&ok),
            vec![Span::Link {
                text: "x".repeat(8),
                href: "h".into()
            }]
        );
    }

    #[test]
    fn a_hash_without_a_space_is_not_a_heading() {
        assert_eq!(
            blocks("#hashtag"),
            vec![Block::Paragraph {
                spans: vec![text("#hashtag")]
            }]
        );
    }

    #[test]
    fn soft_wrapped_lines_join_into_one_paragraph() {
        assert_eq!(
            blocks("one\ntwo\n\nthree"),
            vec![
                Block::Paragraph {
                    spans: vec![text("one two")]
                },
                Block::Paragraph {
                    spans: vec![text("three")]
                },
            ]
        );
    }

    #[test]
    fn bullets_group_into_one_list() {
        assert_eq!(
            blocks("- a\n- b"),
            vec![Block::List {
                ordered: false,
                start: 1,
                items: vec![vec![text("a")], vec![text("b")]]
            }]
        );
    }

    #[test]
    fn an_indented_continuation_joins_its_item() {
        // A hard-wrapped bullet is one bullet. Before this, the second line
        // closed the list and became an orphaned paragraph under it.
        assert_eq!(
            blocks("- first line\n  second line\n- next"),
            vec![Block::List {
                ordered: false,
                start: 1,
                items: vec![vec![text("first line second line")], vec![text("next")]]
            }]
        );
    }

    #[test]
    fn several_continuation_lines_join_with_single_spaces() {
        assert_eq!(
            blocks("- one\n  two\n  three"),
            vec![Block::List {
                ordered: false,
                start: 1,
                items: vec![vec![text("one two three")]]
            }]
        );
        // A tab indents as well as spaces do.
        assert_eq!(
            blocks("- one\n\ttwo"),
            vec![Block::List {
                ordered: false,
                start: 1,
                items: vec![vec![text("one two")]]
            }]
        );
    }

    #[test]
    fn inline_markup_spans_a_continuation() {
        // The item is parsed once it is whole, so a marker opened on one line
        // and closed on the next still pairs up.
        assert_eq!(
            blocks("- a **bold\n  phrase** ends it"),
            vec![Block::List {
                ordered: false,
                start: 1,
                items: vec![vec![
                    text("a "),
                    Span::Bold {
                        text: "bold phrase".into()
                    },
                    text(" ends it"),
                ]]
            }]
        );
    }

    #[test]
    fn an_unindented_line_still_ends_the_list() {
        // The continuation rule must not swallow the paragraph after a list.
        assert_eq!(
            blocks("- a\nprose"),
            vec![
                Block::List {
                    ordered: false,
                    start: 1,
                    items: vec![vec![text("a")]]
                },
                Block::Paragraph {
                    spans: vec![text("prose")]
                },
            ]
        );
    }

    #[test]
    fn an_indented_line_with_no_list_open_is_prose() {
        assert_eq!(
            blocks("intro\n  indented"),
            vec![Block::Paragraph {
                spans: vec![text("intro indented")]
            }]
        );
    }

    #[test]
    fn an_oversized_marker_is_not_a_list() {
        let huge = format!("{}. a", "9".repeat(MAX_MARKER_DIGITS + 1));
        assert_eq!(
            blocks(&huge),
            vec![Block::Paragraph {
                spans: vec![text(&huge)]
            }]
        );
    }

    #[test]
    fn a_loose_list_keeps_counting() {
        // Blank lines between steps are ordinary in notes, and each run would
        // otherwise restart at 1.
        let lists = blocks("1. a\n\n2. b\n\n3. c");
        let starts: Vec<usize> = lists
            .iter()
            .map(|block| match block {
                Block::List { start, .. } => *start,
                other => panic!("expected a list, got {other:?}"),
            })
            .collect();
        assert_eq!(starts, [1, 2, 3]);
    }

    #[test]
    fn inline_code_does_not_open_a_fence() {
        // A backtick after the run means inline code, not an info string.
        // Opening here would swallow the rest of the file.
        let found = blocks("```npm install``` to set up\nnext line");
        assert!(
            matches!(found.as_slice(), [Block::Paragraph { .. }]),
            "{found:?}"
        );
    }

    #[test]
    fn numbered_and_bulleted_lists_do_not_merge() {
        assert_eq!(
            blocks("- a\n1. b"),
            vec![
                Block::List {
                    ordered: false,
                    start: 1,
                    items: vec![vec![text("a")]]
                },
                Block::List {
                    ordered: true,
                    start: 1,
                    items: vec![vec![text("b")]]
                },
            ]
        );
    }

    #[test]
    fn a_paragraph_ends_a_list() {
        assert_eq!(
            blocks("- a\nprose"),
            vec![
                Block::List {
                    ordered: false,
                    start: 1,
                    items: vec![vec![text("a")]]
                },
                Block::Paragraph {
                    spans: vec![text("prose")]
                },
            ]
        );
    }

    #[test]
    fn fenced_code_keeps_its_lines_and_language() {
        assert_eq!(
            blocks("intro\n```swift\nlet x = 1\n\nlet y = 2\n```"),
            vec![
                Block::Paragraph {
                    spans: vec![text("intro")]
                },
                Block::Code {
                    language: Some("swift".into()),
                    text: "let x = 1\n\nlet y = 2".into()
                },
            ]
        );
    }

    #[test]
    fn an_unclosed_fence_still_yields_its_code() {
        assert_eq!(
            blocks("```\nfn main() {"),
            vec![Block::Code {
                language: None,
                text: "fn main() {".into()
            }]
        );
    }

    #[test]
    fn markdown_inside_a_fence_stays_literal() {
        assert_eq!(
            blocks("```\n# not a heading\n- not a bullet\n```"),
            vec![Block::Code {
                language: None,
                text: "# not a heading\n- not a bullet".into()
            }]
        );
    }

    #[test]
    fn inline_styles_split_a_line() {
        assert_eq!(
            spans("a **b** c `d` e"),
            vec![
                text("a "),
                Span::Bold { text: "b".into() },
                text(" c "),
                Span::Code { text: "d".into() },
                text(" e"),
            ]
        );
    }

    #[test]
    fn a_link_carries_its_href() {
        assert_eq!(
            spans("see [docs](http://x.y) now"),
            vec![
                text("see "),
                Span::Link {
                    text: "docs".into(),
                    href: "http://x.y".into()
                },
                text(" now"),
            ]
        );
    }

    #[test]
    fn emphasis_never_eats_a_character_it_read() {
        // Identifiers and arithmetic are ordinary note content, and the
        // delimiters here are not emphasis in anyone's reading.
        assert_eq!(spans("snake_case_name"), vec![text("snake_case_name")]);
        assert_eq!(spans("2 * 3 * 4"), vec![text("2 * 3 * 4")]);
        assert_eq!(spans("a **** b"), vec![text("a **** b")]);
        assert_eq!(spans("a `` b"), vec![text("a `` b")]);
        // The real forms still work.
        assert_eq!(spans("_it_"), vec![Span::Italic { text: "it".into() }]);
        assert_eq!(spans("a _it_ b")[1], Span::Italic { text: "it".into() });
        assert_eq!(spans("**b**"), vec![Span::Bold { text: "b".into() }]);
    }

    #[test]
    fn a_longer_fence_quotes_a_shorter_one() {
        // A note showing how to write a code block. The inner fence must not
        // end the outer one.
        let doc = "````md\n```js\nlet x = 1\n```\n````";
        assert_eq!(
            blocks(doc),
            vec![Block::Code {
                language: Some("md".into()),
                text: "```js\nlet x = 1\n```".into()
            }]
        );
    }

    #[test]
    fn a_fence_with_an_info_string_does_not_close_a_block() {
        // Only a bare run closes; ```rust inside a block is code.
        assert_eq!(
            blocks("```\n```rust\nfn main() {}\n```"),
            vec![Block::Code {
                language: None,
                text: "```rust\nfn main() {}".into()
            }]
        );
    }

    #[test]
    fn an_unclosed_marker_stays_literal() {
        // Dropping it would delete the user's text; a stray asterisk will not.
        assert_eq!(spans("2 * 3 = 6"), vec![text("2 * 3 = 6")]);
        assert_eq!(spans("**bold"), vec![text("**bold")]);
        assert_eq!(spans("a `b"), vec![text("a `b")]);
    }

    #[test]
    fn bold_wins_over_italic() {
        assert_eq!(spans("**b**"), vec![Span::Bold { text: "b".into() }]);
        assert_eq!(spans("*i*"), vec![Span::Italic { text: "i".into() }]);
    }

    #[test]
    fn a_star_bullet_is_not_italic() {
        assert_eq!(
            blocks("* item"),
            vec![Block::List {
                ordered: false,
                start: 1,
                items: vec![vec![text("item")]]
            }]
        );
    }

    #[test]
    fn multibyte_text_survives_inline_scanning() {
        assert_eq!(
            spans("héllo **wörld** 日本"),
            vec![
                text("héllo "),
                Span::Bold {
                    text: "wörld".into()
                },
                text(" 日本"),
            ]
        );
    }

    #[test]
    fn carriage_returns_do_not_leak_into_text() {
        assert_eq!(
            blocks("# Title\r\nbody\r\n"),
            vec![
                Block::Heading {
                    level: 1,
                    spans: vec![text("Title")]
                },
                Block::Paragraph {
                    spans: vec![text("body")]
                },
            ]
        );
    }

    #[test]
    fn block_json_shape() {
        assert_eq!(
            blocks_json("# T\n- a"),
            r#"[{"kind":"heading","level":1,"spans":[{"kind":"text","text":"T"}]},{"kind":"list","ordered":false,"start":1,"items":[[{"kind":"text","text":"a"}]]}]"#
        );
    }

    fn seg_text(s: &str) -> Segment {
        Segment::Text { text: s.into() }
    }
    fn seg_code(s: &str, lang: Option<&str>) -> Segment {
        Segment::Code {
            text: s.into(),
            language: lang.map(Into::into),
        }
    }

    #[test]
    fn plain_text_is_one_segment() {
        assert_eq!(segments("hello world"), vec![seg_text("hello world")]);
    }

    #[test]
    fn fenced_code_splits_into_three_segments() {
        assert_eq!(
            segments("Before\n```swift\nlet x = 1\n```\nAfter"),
            vec![
                seg_text("Before"),
                seg_code("let x = 1", Some("swift")),
                seg_text("After")
            ]
        );
    }

    #[test]
    fn unclosed_fence_renders_as_code() {
        assert_eq!(
            segments("Intro\n```\nfn main() {"),
            vec![seg_text("Intro"), seg_code("fn main() {", None)]
        );
    }

    #[test]
    fn multiple_code_blocks() {
        assert_eq!(
            segments("```rust\na\n```\nmiddle\n```\nb\n```"),
            vec![
                seg_code("a", Some("rust")),
                seg_text("middle"),
                seg_code("b", None)
            ]
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
