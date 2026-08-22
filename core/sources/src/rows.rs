//! The row wire format shared by list and command sources.
//!
//! `id<TAB>title<TAB>group`, where a bare line is a row whose id and title are
//! the same text. Tab separated rather than anything richer so a naive `ls` or
//! `awk` one-liner is already a valid source, while a script that wants stable
//! ranking can still say what the id is.

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
}
