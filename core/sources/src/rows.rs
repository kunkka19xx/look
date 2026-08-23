//! The row wire formats shared by list and command sources.
//!
//! The default is `id<TAB>title<TAB>group`, where a bare line is a row whose id
//! and title are the same text. Tab separated rather than anything richer so a
//! naive `ls` or `awk` one-liner is already a valid source, while a script that
//! wants stable ranking can still say what the id is.
//!
//! `format = "json"` is the opt-in for the two fields tabs cannot carry:
//! `subtitle`, and `path`, which is what makes a row a real filesystem object
//! rather than a piece of text. It costs the author a `jq`, so it stays opt-in.

use serde_json::Value;

use crate::def::RowFormat;

/// The id is what actions receive and what usage is recorded against, so a row
/// can display one thing and act on another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRow {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    /// Section header this row sits under, within its own source.
    pub group: Option<String>,
    /// Filesystem target, when the row has one. Folder sources always do.
    pub path: Option<String>,
}

impl SourceRow {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            subtitle: None,
            group: None,
            path: None,
        }
    }
}

const FIELD_SEPARATOR: char = '\t';

/// Parses one line. Blank lines are not rows; a line that is only whitespace is
/// almost always an artifact of the script, not something the user wants to see.
pub fn parse_line(line: &str) -> Option<SourceRow> {
    let line = line.strip_suffix('\r').unwrap_or(line);
    if line.trim().is_empty() {
        return None;
    }

    let mut fields = line.split(FIELD_SEPARATOR);
    let id = fields.next()?.trim();
    if id.is_empty() {
        return None;
    }
    let title = fields
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let group = fields
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    Some(SourceRow {
        id: id.to_string(),
        title: title.unwrap_or(id).to_string(),
        subtitle: None,
        group: group.map(String::from),
        path: None,
    })
}

/// Parses stdout or a list file, dropping rows past `limit` so a runaway
/// producer cannot flood the index. The caller reports the truncation.
pub fn parse_lines(text: &str, limit: usize) -> (Vec<SourceRow>, bool) {
    let mut rows = Vec::new();
    for line in text.lines() {
        if rows.len() == limit {
            return (rows, true);
        }
        if let Some(row) = parse_line(line) {
            rows.push(row);
        }
    }
    (rows, false)
}

/// Field names a JSON row understands. Anything else is ignored rather than
/// refused, so a script can pipe its own richer output through untouched.
const KEY_ID: &str = "id";
const KEY_TITLE: &str = "title";
const KEY_SUBTITLE: &str = "subtitle";
const KEY_GROUP: &str = "group";
const KEY_PATH: &str = "path";

/// Parses rows in whichever encoding the block declared. `Err` means the text
/// could not be read at all, which the caller reports and, for a `run` block,
/// treats as a failed refresh that keeps the previous rows.
pub fn parse_rows(
    text: &str,
    limit: usize,
    format: RowFormat,
) -> Result<(Vec<SourceRow>, bool), String> {
    match format {
        RowFormat::Lines => Ok(parse_lines(text, limit)),
        RowFormat::Json => parse_json(text, limit),
    }
}

/// Accepts the three shapes a real command emits: one top-level array, one
/// object per line, or pretty-printed objects run together. All three are what
/// some tool already prints, and telling them apart is free.
pub fn parse_json(text: &str, limit: usize) -> Result<(Vec<SourceRow>, bool), String> {
    let mut rows = Vec::new();
    for value in serde_json::Deserializer::from_str(text).into_iter::<Value>() {
        let value = value.map_err(|err| format!("not valid JSON: {err}"))?;
        let items = match value {
            Value::Array(items) => items,
            single => vec![single],
        };
        for item in items {
            if rows.len() == limit {
                return Ok((rows, true));
            }
            if let Some(row) = parse_value(&item) {
                rows.push(row);
            }
        }
    }
    Ok((rows, false))
}

/// One JSON row. A bare string or number is a row whose id and title are the
/// same, so `["main", "dev"]` reads exactly like two bare lines.
fn parse_value(value: &Value) -> Option<SourceRow> {
    if let Some(text) = text_field(Some(value)) {
        return Some(SourceRow::new(text.clone(), text));
    }

    let object = value.as_object()?;
    let id = text_field(object.get(KEY_ID))?;
    Some(SourceRow {
        title: text_field(object.get(KEY_TITLE)).unwrap_or_else(|| id.clone()),
        id,
        subtitle: text_field(object.get(KEY_SUBTITLE)),
        group: text_field(object.get(KEY_GROUP)),
        path: text_field(object.get(KEY_PATH)),
    })
}

/// A string or a number, trimmed, with empty treated as absent so the fallbacks
/// match the line format's. Numbers because the tools that already emit JSON
/// emit them: an issue number, a pid, a port.
fn text_field(value: Option<&Value>) -> Option<String> {
    let text = match value? {
        Value::String(text) => text.trim().to_string(),
        Value::Number(number) => number.to_string(),
        _ => return None,
    };
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bare_line_is_its_own_id_and_title() {
        let row = parse_line("look").unwrap();
        assert_eq!(row.id, "look");
        assert_eq!(row.title, "look");
        assert_eq!(row.group, None);
    }

    #[test]
    fn id_and_title_can_differ_so_a_row_shows_one_thing_and_acts_on_another() {
        let row = parse_line("look\tlook (main, 3 windows)\tSession").unwrap();
        assert_eq!(row.id, "look");
        assert_eq!(row.title, "look (main, 3 windows)");
        assert_eq!(row.group.as_deref(), Some("Session"));
    }

    #[test]
    fn blank_and_idless_lines_are_not_rows() {
        assert!(parse_line("").is_none());
        assert!(parse_line("   ").is_none());
        assert!(parse_line("\tonly a title").is_none());
    }

    #[test]
    fn empty_trailing_fields_fall_back_rather_than_showing_blanks() {
        let row = parse_line("look\t\t").unwrap();
        assert_eq!(row.title, "look");
        assert_eq!(row.group, None);
    }

    #[test]
    fn windows_line_endings_do_not_end_up_in_the_text() {
        let (rows, _) = parse_lines("alpha\r\nbeta\r\n", 10);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].title, "beta");
    }

    #[test]
    fn output_past_the_limit_is_dropped_and_reported() {
        let text = (0..10)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let (rows, truncated) = parse_lines(&text, 4);
        assert_eq!(rows.len(), 4);
        assert!(truncated);

        let (rows, truncated) = parse_lines(&text, 100);
        assert_eq!(rows.len(), 10);
        assert!(!truncated);
    }

    #[test]
    fn a_json_row_carries_the_fields_tabs_cannot() {
        let (rows, _) = parse_json(
            r#"{"id":"look","title":"Look","subtitle":"3 uncommitted","group":"This week","path":"/dev/look"}"#,
            10,
        )
        .unwrap();
        let row = &rows[0];
        assert_eq!(row.id, "look");
        assert_eq!(row.title, "Look");
        assert_eq!(row.subtitle.as_deref(), Some("3 uncommitted"));
        assert_eq!(row.group.as_deref(), Some("This week"));
        assert_eq!(row.path.as_deref(), Some("/dev/look"));
    }

    #[test]
    fn an_array_and_one_object_per_line_read_the_same() {
        let array = r#"[{"id":"a"},{"id":"b"}]"#;
        let stream = "{\"id\":\"a\"}\n{\"id\":\"b\"}\n";
        let pretty = "{\n  \"id\": \"a\"\n}\n{\n  \"id\": \"b\"\n}\n";

        let (from_array, _) = parse_json(array, 10).unwrap();
        let (from_stream, _) = parse_json(stream, 10).unwrap();
        let (from_pretty, _) = parse_json(pretty, 10).unwrap();
        assert_eq!(from_array, from_stream);
        assert_eq!(from_array, from_pretty);
        assert_eq!(from_array.len(), 2);
    }

    #[test]
    fn a_bare_string_or_number_is_its_own_id_and_title() {
        let (rows, _) = parse_json(r#"["main", 412]"#, 10).unwrap();
        assert_eq!(rows[0].id, "main");
        assert_eq!(rows[0].title, "main");
        assert_eq!(rows[1].id, "412");
        assert_eq!(rows[1].title, "412");
    }

    #[test]
    fn an_id_that_is_a_number_still_ranks_and_a_missing_one_is_not_a_row() {
        let (rows, _) = parse_json(
            r#"[{"id":412,"title":"Crash on launch"},{"title":"no id"}]"#,
            10,
        )
        .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "412");
        assert_eq!(rows[0].title, "Crash on launch");
    }

    #[test]
    fn keys_the_row_format_does_not_know_are_ignored_rather_than_refused() {
        let (rows, _) = parse_json(r#"{"id":"a","Image":"postgres:16"}"#, 10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].title, "a");
    }

    #[test]
    fn malformed_json_is_reported_rather_than_read_as_no_rows() {
        // Silence would look like a source that produced nothing, which for a
        // `run` block means "keep the old rows" and hides the typo forever.
        assert!(parse_json("{\"id\": }", 10).is_err());
        assert!(parse_json("not json at all", 10).is_err());
    }

    #[test]
    fn json_output_past_the_limit_is_dropped_and_reported() {
        let text = format!(
            "[{}]",
            (0..10)
                .map(|i| format!(r#"{{"id":"{i}"}}"#))
                .collect::<Vec<_>>()
                .join(",")
        );
        let (rows, truncated) = parse_json(&text, 4).unwrap();
        assert_eq!(rows.len(), 4);
        assert!(truncated);
    }

    #[test]
    fn the_declared_format_picks_the_parser() {
        let (rows, _) = parse_rows("look\tLook", 10, RowFormat::Lines).unwrap();
        assert_eq!(rows[0].title, "Look");
        let (rows, _) = parse_rows(r#"{"id":"look","title":"Look"}"#, 10, RowFormat::Json).unwrap();
        assert_eq!(rows[0].title, "Look");
    }
}
