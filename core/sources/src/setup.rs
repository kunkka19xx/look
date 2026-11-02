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
fn without_setup(contents: &str) -> String {
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
            // Same length, same line breaks, nothing to parse.
            out.extend(line.chars().map(|ch| if ch == '\n' { '\n' } else { ' ' }));
        } else {
            out.push_str(line);
        }
    }
    out
}

/// `[[setup]]` -> `setup`, for a header line only.
fn header_names(line: &str) -> Option<&str> {
    line.trim_end()
        .strip_prefix("[[")?
        .strip_suffix("]]")
        .map(str::trim)
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

/// The exact bytes a field replaces: the whole value, or the `match` inside
/// it. One answer to "where does this field point", so a checker and an
/// installer cannot disagree about whether a declaration is usable.
fn target(contents: &str, field: &SetupField) -> Result<(Range<usize>, bool), String> {
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
        return Ok((span, true));
    };
    if !contents_contains_once(text, matching) {
        return Err(format!(
            "{}: {matching:?} is not in [{}] `{}` exactly once",
            field.label, field.block, field.key
        ));
    }
    let at = span.start + text.find(matching).unwrap_or(0);
    Ok((at..at + matching.len(), false))
}

/// Every field checked against the document it points into, for a checker run
/// before anybody installs it. Empty means all of them resolve.
pub fn validate(contents: &str, fields: &[SetupField]) -> Vec<String> {
    let mut problems = Vec::new();
    for field in fields {
        if let Err(message) = target(contents, field) {
            problems.push(message);
            continue;
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
            // somebody else's.
            Ok((span, whole)) => edits.push((
                span,
                if whole {
                    quoted(answer)
                } else {
                    escaped(answer)
                },
            )),
            Err(message) => problems.push(message),
        }
    }

    if !problems.is_empty() {
        return Err(problems);
    }

    edits.sort_by_key(|(span, _)| std::cmp::Reverse(span.start));
    let mut out = contents.to_string();
    for (span, replacement) in edits {
        out.replace_range(span, &replacement);
    }
    Ok(out)
}

fn contents_contains_once(text: &str, needle: &str) -> bool {
    !needle.is_empty() && text.matches(needle).count() == 1
}

/// A TOML basic string. The answer is a path or a word the user typed, so the
/// escapes a basic string needs are the only ones that can occur.
fn quoted(value: &str) -> String {
    format!("\"{}\"", escaped(value))
}

fn escaped(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
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
        let text = "[a]\ndo = [\"x\"]\n\n[[setup]]\nlabel = \"L\"\ntype = \"choice\"\nblock = \"a\"\nkey = \"do\"\n";
        let problems = validate(text, &fields_of(text));
        assert!(problems[0].contains("needs `options`"), "{problems:?}");
    }

    #[test]
    fn a_sound_declaration_validates_clean() {
        assert!(validate(PROJECTS, &fields_of(PROJECTS)).is_empty());
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
