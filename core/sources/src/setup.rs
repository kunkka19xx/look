//! Values an example asks you for, declared by whoever wrote it.
//!
//! The readiness check infers what cannot work here, which reaches a missing
//! folder and a missing program and stops there. It cannot see that `vault=look`
//! in an Obsidian URL is somebody's vault name: that string is valid, it works
//! on the author's machine, and it silently opens nothing on yours.
//!
//! So the author says. A `[[setup]]` field names a block and a key, and the
//! installer offers a folder picker or a text box instead of asking you to go
//! and find the line.
//!
//! Two properties the shape protects:
//!
//! - The file still works as written. A field points at a real value rather
//!   than a placeholder, so copying the `.toml` by hand is unaffected and an
//!   example is never shipped in a broken state to be filled in later.
//! - Comments survive. Filling one in rewrites the bytes of one value, located
//!   by span, rather than re-serialising the document. In these examples the
//!   prose is most of the file and re-emitting it would throw that away.

use std::collections::BTreeMap;
use std::ops::Range;

use serde::Deserialize;

/// The key holding the fields. Reserved: it is the one top-level name that is
/// not a block.
pub(crate) const KEY_SETUP: &str = "setup";

/// What kind of answer a field wants, which decides the control shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FieldKind {
    /// A directory, picked rather than typed.
    Folder,
    /// A file, likewise.
    File,
    #[default]
    Text,
    /// One of `options`.
    Choice,
}

impl FieldKind {
    /// The name the wire and the declaration both use.
    pub fn name(self) -> &'static str {
        match self {
            Self::Folder => "folder",
            Self::File => "file",
            Self::Text => "text",
            Self::Choice => "choice",
        }
    }
}

/// One value the example asks for.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SetupField {
    /// Shown next to the control. The author's words, which is the whole point
    /// of declaring this rather than inferring it.
    pub label: String,
    #[serde(default, rename = "type")]
    pub kind: FieldKind,
    /// The block the value lives in.
    pub block: String,
    /// The key within it.
    pub key: String,
    /// The exact text to replace inside that value, for a value that is part
    /// of a command rather than all of it. Must appear exactly once.
    #[serde(rename = "match")]
    pub matching: Option<String>,
    /// For `choice`.
    #[serde(default)]
    pub options: Vec<String>,
    /// Shown as a hint under the control.
    pub note: Option<String>,
}

/// The fields a declaration asks for, in the order written.
///
/// Errs only on text that is not TOML at all. A malformed field is reported
/// rather than fatal, because the rest of the file is still installable.
pub fn parse_setup(contents: &str) -> Result<(Vec<SetupField>, Vec<String>), String> {
    #[derive(Deserialize)]
    struct Root {
        #[serde(default)]
        setup: Vec<toml::Value>,
    }

    let root: Root = toml::from_str(contents).map_err(|err| err.to_string())?;
    let mut fields = Vec::new();
    let mut problems = Vec::new();
    for (index, raw) in root.setup.into_iter().enumerate() {
        match raw.try_into::<SetupField>() {
            Ok(field) => fields.push(field),
            Err(err) => problems.push(format!("[[{KEY_SETUP}]] #{}: {err}", index + 1)),
        }
    }
    Ok((fields, problems))
}

/// Where a key's value sits in the original text, as a byte range covering the
/// value and nothing else: not the key, not a trailing comment.
pub fn locate(contents: &str, block: &str, key: &str) -> Option<Range<usize>> {
    type Spans = BTreeMap<String, BTreeMap<String, toml::Spanned<toml::Value>>>;

    let tables: Spans = toml::from_str(&without_setup(contents)).ok()?;
    Some(tables.get(block)?.get(key)?.span())
}

/// The same document with every `[[setup]]` section blanked out, byte for
/// byte, so spans taken from it still address the original text.
///
/// Needed because spans come from deserializing a map of tables, and the
/// fields are an array of tables sitting among them. Blanking rather than
/// deleting is the whole trick: delete and every span after the first field
/// is wrong by however long the section was.
pub(crate) fn without_setup(contents: &str) -> String {
    let mut out = String::with_capacity(contents.len());
    let mut blanking = false;
    for line in contents.split_inclusive('\n') {
        let start = line.trim_start();
        if start.starts_with("[[") {
            blanking = header_names(start) == Some(KEY_SETUP);
        } else if start.starts_with('[') {
            blanking = false;
        }
        if blanking {
            // Same byte length, same line breaks, nothing to parse. Counted
            // in bytes rather than characters: an accented letter in a label
            // is two or more, and one space each would shorten the document
            // and shift every span taken after it.
            for ch in line.chars() {
                if ch == '\n' {
                    out.push('\n');
                } else {
                    out.extend(std::iter::repeat_n(' ', ch.len_utf8()));
                }
            }
        } else {
            out.push_str(line);
        }
    }
    out
}

/// `[[setup]]` -> `setup`, for a header line only. `[[setup]] # why` is a
/// header too, so the comment comes off before the brackets are matched.
fn header_names(line: &str) -> Option<&str> {
    let line = line.trim_end();
    let line = match line.split_once('#') {
        Some((before, _)) if before.trim_end().ends_with("]]") => before.trim_end(),
        _ => line,
    };
    line.strip_prefix("[[")?.strip_suffix("]]").map(str::trim)
}

/// What each field would replace, for showing the current value in the form.
pub fn current(contents: &str, field: &SetupField) -> Option<String> {
    let span = locate(contents, &field.block, &field.key)?;
    let text = contents.get(span)?;
    match &field.matching {
        Some(matching) => contents_contains_once(text, matching).then(|| matching.clone()),
        // The quotes belong to the TOML, not to the value.
        None => Some(text.trim_matches(['"', '\'']).to_string()),
    }
}

/// What a TOML string may open with.
const QUOTES: [char; 2] = ['"', '\''];

/// How an answer has to be written where it lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Writing {
    /// Over the whole value, quotes and all, so it brings its own.
    WholeString,
    /// Inside `"..."`, where the basic-string escapes apply.
    InsideBasic,
    /// Inside `'...'`, where nothing is escaped and nothing may be: a
    /// backslash stays a backslash and an apostrophe cannot appear.
    InsideLiteral,
}

/// The exact bytes a field replaces: the whole value, or the `match` inside
/// it. One answer to "where does this field point", so a checker and an
/// installer cannot disagree about whether a declaration is usable.
fn target(contents: &str, field: &SetupField) -> Result<(Range<usize>, Writing), String> {
    let span = locate(contents, &field.block, &field.key).ok_or_else(|| {
        format!(
            "{}: [{}] has no `{}` to fill in",
            field.label, field.block, field.key
        )
    })?;
    let text = contents
        .get(span.clone())
        .ok_or_else(|| format!("{}: its value is not readable", field.label))?;

    let Some(matching) = &field.matching else {
        // A whole-value fill writes a quoted string over the span, so the
        // value has to already be one. Over `dirs = [...]` or `depth = 2` it
        // would leave a string where the loader wants a list or a number,
        // and the block would stop loading.
        if !text.starts_with(QUOTES) {
            return Err(format!(
                "{}: [{}] `{}` is not a string, so there is nothing to fill in. \
                 Use `match` to name the part of it to replace",
                field.label, field.block, field.key
            ));
        }
        return Ok((span, Writing::WholeString));
    };
    if !contents_contains_once(text, matching) {
        return Err(format!(
            "{}: {matching:?} is not in [{}] `{}` exactly once",
            field.label, field.block, field.key
        ));
    }
    // Decided here, where the enclosing value is still in hand: the
    // substring's own span says nothing about the quotes around it. The
    // nearest quote before the match, not the first character of the value,
    // because a `do` value is an array and the strings are inside it.
    let offset = text.find(matching).unwrap_or(0);
    let writing = match text[..offset].rfind(QUOTES) {
        Some(at) if text[at..].starts_with('\'') => Writing::InsideLiteral,
        _ => Writing::InsideBasic,
    };
    let at = span.start + offset;
    Ok((at..at + matching.len(), writing))
}

/// Every field checked against the document it points into, for a checker run
/// before anybody installs it. Empty means all of them resolve.
pub fn validate(contents: &str, fields: &[SetupField]) -> Vec<String> {
    let mut problems = Vec::new();
    let mut claimed: Vec<Range<usize>> = Vec::new();
    for field in fields {
        match target(contents, field) {
            Err(message) => {
                problems.push(message);
                continue;
            }
            Ok((span, _)) => {
                if let Some(other) = claimed.iter().find(|other| overlaps(other, &span)) {
                    problems.push(format!(
                        "{}: it fills the same text as another field ({other:?})",
                        field.label
                    ));
                    continue;
                }
                claimed.push(span);
            }
        }
        if field.kind != FieldKind::Choice {
            continue;
        }
        if field.options.is_empty() {
            problems.push(format!("{}: a `choice` field needs `options`", field.label));
        } else if let Some(now) = current(contents, field)
            && !field.options.contains(&now)
        {
            // Otherwise the form opens on a value it cannot show, and the
            // author has told the user to pick from a list their own example
            // is not in.
            problems.push(format!(
                "{}: the declared value {now:?} is not one of `options`",
                field.label
            ));
        }
    }
    problems
}

/// `contents` with every answered field replaced.
///
/// `answers` runs parallel to `fields`; `None` leaves that value as the author
/// wrote it. Applied back to front so an earlier replacement cannot shift a
/// later field's span out from under it.
pub fn fill(
    contents: &str,
    fields: &[SetupField],
    answers: &[Option<String>],
) -> Result<String, Vec<String>> {
    let mut edits: Vec<(Range<usize>, String)> = Vec::new();
    let mut problems = Vec::new();

    for (index, field) in fields.iter().enumerate() {
        let Some(Some(answer)) = answers.get(index) else {
            continue;
        };
        match target(contents, field) {
            // A whole value carries its own quotes; a substring sits inside
            // somebody else's, and which kind decides how it is escaped.
            Ok((span, writing)) => {
                // A literal string has no escapes at all, so an answer that
                // would need one cannot go in it.
                if writing == Writing::InsideLiteral && answer.contains(['\'', '\n']) {
                    problems.push(format!(
                        "{}: {answer:?} cannot go inside a '...' string, which has \
                         no way to escape an apostrophe or a newline",
                        field.label
                    ));
                    continue;
                }
                let text = match writing {
                    Writing::WholeString => quoted(answer),
                    Writing::InsideBasic => escaped(answer),
                    Writing::InsideLiteral => answer.clone(),
                };
                edits.push((span, text));
            }
            Err(message) => problems.push(message),
        }
    }

    // Two fields can land on the same text: two whole-value fields on one
    // key, or a whole value and a `match` inside it. The edits are applied
    // back to front, which assumes they do not overlap; applying them anyway
    // corrupts the file or panics on a stale range, across the FFI.
    edits.sort_by_key(|(span, _)| std::cmp::Reverse(span.start));
    for pair in edits.windows(2) {
        if overlaps(&pair[1].0, &pair[0].0) {
            problems.push(format!(
                "two fields fill the same text ({:?} and {:?})",
                pair[1].0, pair[0].0
            ));
        }
    }

    if !problems.is_empty() {
        return Err(problems);
    }
    let mut out = contents.to_string();
    for (span, replacement) in edits {
        out.replace_range(span, &replacement);
    }
    Ok(out)
}

fn overlaps(one: &Range<usize>, other: &Range<usize>) -> bool {
    one.start < other.end && other.start < one.end
}

fn contents_contains_once(text: &str, needle: &str) -> bool {
    !needle.is_empty() && text.matches(needle).count() == 1
}

/// A TOML basic string. The answer is a path or a word the user typed, so the
/// escapes a basic string needs are the only ones that can occur.
fn quoted(value: &str) -> String {
    format!("\"{}\"", escaped(value))
}

/// A TOML basic string's contents. Every control character, not the few that
/// came to mind: the answer arrives from the shell across JSON and is written
/// to disk, and a stray `\r` in it makes the installed file stop parsing.
fn escaped(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            ch if ch.is_control() => out.push_str(&format!("\\u{:04X}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROJECTS: &str = r#"
# Every project in ~/dev. This comment has to survive.
[dev-projects]
name = "Projects"
dir  = "~/dev"            # and so does this one
edit = "code {path}"

[[setup]]
label = "Your projects folder"
type  = "folder"
block = "dev-projects"
key   = "dir"

[[setup]]
label = "Editor command"
block = "dev-projects"
key   = "edit"
match = "code"
"#;

    fn fields_of(text: &str) -> Vec<SetupField> {
        let (fields, problems) = parse_setup(text).expect("parses");
        assert!(problems.is_empty(), "{problems:?}");
        fields
    }

    #[test]
    fn a_declaration_says_what_it_wants_and_in_what_shape() {
        let fields = fields_of(PROJECTS);
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].label, "Your projects folder");
        assert_eq!(fields[0].kind, FieldKind::Folder);
        assert_eq!(fields[0].key, "dir");
        assert_eq!(fields[1].kind, FieldKind::Text);
        assert_eq!(fields[1].matching.as_deref(), Some("code"));
    }

    #[test]
    fn the_current_value_is_what_the_author_wrote() {
        let fields = fields_of(PROJECTS);
        assert_eq!(current(PROJECTS, &fields[0]).as_deref(), Some("~/dev"));
        assert_eq!(current(PROJECTS, &fields[1]).as_deref(), Some("code"));
    }

    #[test]
    fn filling_a_whole_value_leaves_every_comment_where_it_was() {
        let fields = fields_of(PROJECTS);
        let out = fill(
            PROJECTS,
            &fields,
            &[Some("/Users/me/work".to_string()), None],
        )
        .expect("fills");

        assert!(
            out.contains(r#"dir  = "/Users/me/work"            # and so does this one"#),
            "{out}"
        );
        assert!(out.contains("# Every project in ~/dev. This comment has to survive."));
        assert!(out.contains(r#"edit = "code {path}""#), "untouched: {out}");
    }

    #[test]
    fn filling_a_substring_leaves_the_rest_of_the_command() {
        let fields = fields_of(PROJECTS);
        let out = fill(PROJECTS, &fields, &[None, Some("nvim".to_string())]).expect("fills");
        assert!(out.contains(r#"edit = "nvim {path}""#), "{out}");
        assert!(out.contains(r#"dir  = "~/dev""#), "untouched: {out}");
    }

    #[test]
    fn two_fields_at_once_do_not_shift_each_other() {
        let fields = fields_of(PROJECTS);
        let out = fill(
            PROJECTS,
            &fields,
            &[Some("/tmp/p".to_string()), Some("hx".to_string())],
        )
        .expect("fills");
        assert!(out.contains(r#"dir  = "/tmp/p""#), "{out}");
        assert!(out.contains(r#"edit = "hx {path}""#), "{out}");
    }

    #[test]
    fn a_value_inside_a_url_is_reachable() {
        let text = r#"
[obsnote]
do = ["open -g \"obsidian://open?vault=look\""]

[[setup]]
label = "Obsidian vault name"
block = "obsnote"
key   = "do"
match = "vault=look"
"#;
        let fields = fields_of(text);
        let out = fill(text, &fields, &[Some("vault=notes".to_string())]).expect("fills");
        assert!(out.contains("obsidian://open?vault=notes"), "{out}");
    }

    #[test]
    fn a_match_that_is_not_unique_is_refused_rather_than_guessed() {
        let text = r#"
[a]
do = ["cp x x"]

[[setup]]
label = "Which x"
block = "a"
key   = "do"
match = "x"
"#;
        let fields = fields_of(text);
        let err = fill(text, &fields, &[Some("y".to_string())]).expect_err("refuses");
        assert!(err[0].contains("exactly once"), "{err:?}");
    }

    #[test]
    fn a_field_pointing_at_nothing_is_reported() {
        let text = r#"
[a]
do = ["true"]

[[setup]]
label = "Nowhere"
block = "a"
key   = "dir"
"#;
        let fields = fields_of(text);
        let err = fill(text, &fields, &[Some("/tmp".to_string())]).expect_err("refuses");
        assert!(err[0].contains("no `dir`"), "{err:?}");
    }

    #[test]
    fn an_unanswered_field_changes_nothing() {
        let fields = fields_of(PROJECTS);
        assert_eq!(
            fill(PROJECTS, &fields, &[None, None]).expect("fills"),
            PROJECTS
        );
        assert_eq!(fill(PROJECTS, &fields, &[]).expect("fills"), PROJECTS);
    }

    #[test]
    fn an_answer_with_a_quote_in_it_cannot_break_out_of_the_string() {
        let fields = fields_of(PROJECTS);
        let out =
            fill(PROJECTS, &fields, &[Some("/tmp/a\"b\\c".to_string()), None]).expect("fills");
        // It has to survive a round trip as the value it was, not as syntax.
        let (back, _) = parse_setup(&out).expect("still parses");
        assert!(!back.is_empty());
        assert_eq!(
            current(&out, &fields[0]).as_deref(),
            Some("/tmp/a\\\"b\\\\c")
        );
    }

    #[test]
    fn a_checker_catches_a_field_that_points_nowhere() {
        let text =
            "[a]\ndo = [\"true\"]\n\n[[setup]]\nlabel = \"L\"\nblock = \"a\"\nkey = \"dir\"\n";
        let problems = validate(text, &fields_of(text));
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("no `dir`"), "{problems:?}");
    }

    #[test]
    fn a_choice_must_offer_the_value_the_author_declared() {
        let text = r#"
[a]
open = "open -na Ghostty"

[[setup]]
label = "Your terminal"
type  = "choice"
block = "a"
key   = "open"
match = "Ghostty"
options = ["Alacritty", "kitty"]
"#;
        let problems = validate(text, &fields_of(text));
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("not one of"), "{problems:?}");
    }

    #[test]
    fn a_choice_with_no_options_is_refused() {
        let text = "[a]\nopen = \"x\"\n\n[[setup]]\nlabel = \"L\"\ntype = \"choice\"\nblock = \"a\"\nkey = \"open\"\n";
        let problems = validate(text, &fields_of(text));
        assert!(problems[0].contains("needs `options`"), "{problems:?}");
    }

    #[test]
    fn a_sound_declaration_validates_clean() {
        assert!(validate(PROJECTS, &fields_of(PROJECTS)).is_empty());
    }

    #[test]
    fn a_multibyte_label_does_not_shift_the_spans_after_it() {
        // Blanking the section by characters rather than bytes shortened the
        // document, and every span taken after it pointed a few bytes early.
        let text = r#"
[[setup]]
label = "Choose a folder … café"
block = "notes"
key   = "dir"

[notes]
dir = "~/old"
"#;
        let fields = fields_of(text);
        assert_eq!(current(text, &fields[0]).as_deref(), Some("~/old"));
        let out = fill(text, &fields, &[Some("/tmp/new".to_string())]).expect("fills");
        assert!(out.contains(r#"dir = "/tmp/new""#), "{out}");
    }

    #[test]
    fn a_comment_on_the_header_still_marks_the_section() {
        let text = r#"
[[setup]] # what you change
label = "Folder"
block = "notes"
key   = "dir"

[notes]
dir = "~/old"
"#;
        let fields = fields_of(text);
        assert_eq!(current(text, &fields[0]).as_deref(), Some("~/old"));
        assert!(validate(text, &fields).is_empty());
    }

    #[test]
    fn a_whole_value_fill_is_refused_on_something_that_is_not_a_string() {
        let text = r#"
[notes]
dirs  = ["~/a", "~/b"]
depth = 2

[[setup]]
label = "Folders"
block = "notes"
key   = "dirs"
"#;
        let fields = fields_of(text);
        let problems = validate(text, &fields);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("not a string"), "{problems:?}");
    }

    #[test]
    fn two_fields_on_the_same_text_are_refused_rather_than_applied() {
        let text = r#"
[notes]
dir = "~/old"

[[setup]]
label = "One"
block = "notes"
key   = "dir"

[[setup]]
label = "Two"
block = "notes"
key   = "dir"
"#;
        let fields = fields_of(text);
        assert!(!validate(text, &fields).is_empty());
        let err =
            fill(text, &fields, &[Some("/a".into()), Some("/b".into())]).expect_err("refuses");
        assert!(err.iter().any(|one| one.contains("same text")), "{err:?}");
    }

    #[test]
    fn an_answer_inside_a_literal_string_keeps_its_backslashes() {
        let text = r#"
[notes]
do = ['open C:\tools\x']

[[setup]]
label = "Tool"
block = "notes"
key   = "do"
match = 'C:\tools\x'
"#;
        let fields = fields_of(text);
        let out = fill(text, &fields, &[Some(r"D:in\y".to_string())]).expect("fills");
        // One backslash each, not two: nothing is escaped inside '...'.
        assert!(out.contains(r"open D:in\y"), "{out}");
    }

    #[test]
    fn an_apostrophe_cannot_be_written_into_a_literal_string() {
        let text = r#"
[notes]
do = ['echo hello']

[[setup]]
label = "Word"
block = "notes"
key   = "do"
match = 'hello'
"#;
        let fields = fields_of(text);
        let err = fill(text, &fields, &[Some("it's".to_string())]).expect_err("refuses");
        assert!(err[0].contains("apostrophe"), "{err:?}");
    }

    #[test]
    fn a_control_character_in_an_answer_cannot_break_the_file() {
        let home_text = r#"
[notes]
dir = "~/old"

[[setup]]
label = "Folder"
block = "notes"
key   = "dir"
"#;
        let fields = fields_of(home_text);
        let out = fill(home_text, &fields, &[Some("a\rb\u{1}c".to_string())]).expect("fills");
        // It has to still be a file of blocks afterwards.
        let (back, problems) = parse_setup(&out).expect("still parses");
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(back.len(), 1);
    }

    #[test]
    fn a_file_with_no_setup_asks_for_nothing() {
        let (fields, problems) = parse_setup("[a]\ndo = [\"true\"]\n").expect("parses");
        assert!(fields.is_empty());
        assert!(problems.is_empty());
    }

    #[test]
    fn a_malformed_field_is_reported_and_the_others_still_load() {
        let text = r#"
[a]
do = ["true"]

[[setup]]
label = "Fine"
block = "a"
key   = "do"

[[setup]]
block = "a"
"#;
        let (fields, problems) = parse_setup(text).expect("parses");
        assert_eq!(fields.len(), 1);
        assert_eq!(problems.len(), 1, "{problems:?}");
    }
}
