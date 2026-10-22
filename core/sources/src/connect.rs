//! The frames a connector speaks: line-delimited JSON over a child process's
//! stdin and stdout.
//!
//! A child rather than a socket because there is no port to allocate, no auth
//! to invent and no firewall prompt, and it dies with Look. This module is the
//! wire only - parsing, encoding, and what each frame means. Owning the process
//! is the engine's job.

use serde::{Deserialize, Serialize};

use crate::rows::SourceRow;

/// What Look says. `v` rides on `hello` alone: a connector that cannot read it
/// has nothing to answer, and every later frame is already inside a version it
/// agreed to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Ask {
    Hello { v: u32, look: String },
    Query { block: String, q: String },
    Preview { block: String, id: String, req: u64 },
    Bye,
}

/// What a connector says back, or says on its own: a `rows` frame nobody asked
/// for is a push, which is the whole liveness story.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Say {
    Ready {
        #[serde(default)]
        capabilities: Vec<String>,
    },
    Rows {
        block: String,
        rows: Vec<Row>,
    },
    Preview {
        block: String,
        id: String,
        /// The `req` of the ask this answers, echoed back. An answer without
        /// one is dropped: it cannot be told from a late reply to an older
        /// ask, which is the whole point of the number.
        #[serde(default)]
        req: Option<u64>,
        /// What `text` is, so the panel renders it rather than guessing.
        /// Absent means plain text, which is what a connector that never
        /// heard of this field meant.
        #[serde(default)]
        content_type: Content,
        text: String,
    },
    Error {
        message: String,
    },
}

/// The content types a preview may carry. An unknown one reads as `text`:
/// showing the source beats showing nothing, and a connector written against a
/// later Look must not blank the panel on an older one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Content {
    Markdown,
    #[default]
    #[serde(other)]
    Text,
}

/// One row as a connector writes it. The same fields a JSON row already has in
/// a `run` block's output, so a connector and a script describe a row alike.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Row {
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
}

/// The version Look speaks. A connector may answer anything; what it must not
/// do is assume a later one.
pub const PROTOCOL_VERSION: u32 = 1;

impl Ask {
    /// One line, newline included, ready to write.
    pub fn line(&self) -> String {
        let mut line = serde_json::to_string(self).unwrap_or_default();
        line.push('\n');
        line
    }
}

impl From<Row> for SourceRow {
    fn from(row: Row) -> Self {
        SourceRow {
            title: row.title.unwrap_or_else(|| row.id.clone()),
            id: row.id,
            subtitle: row.subtitle,
            path: row.path,
            icon: row.icon,
        }
    }
}

/// One line from a connector, or `None` when it is not a frame this version
/// understands. Unknown ops are ignored rather than fatal: a connector written
/// for a later Look must still work with this one.
pub fn read(line: &str) -> Option<Say> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    serde_json::from_str(line).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_ask_is_one_line() {
        let hello = Ask::Hello {
            v: PROTOCOL_VERSION,
            look: "0.7.5".into(),
        }
        .line();
        assert_eq!(hello, "{\"op\":\"hello\",\"v\":1,\"look\":\"0.7.5\"}\n");
        assert!(
            Ask::Query {
                block: "myapp.notes".into(),
                q: "inv".into(),
            }
            .line()
            .ends_with('\n')
        );
    }

    #[test]
    fn rows_arrive_as_rows() {
        let say = read(
            r#"{"op":"rows","block":"myapp.notes","rows":[{"id":"a","title":"A"},{"id":"b"}]}"#,
        )
        .expect("a rows frame");
        let Say::Rows { block, rows } = say else {
            panic!("expected rows");
        };
        assert_eq!(block, "myapp.notes");
        // A row with no title is titled by its id, exactly as a JSON row is.
        let rows: Vec<SourceRow> = rows.into_iter().map(SourceRow::from).collect();
        assert_eq!(rows[0].title, "A");
        assert_eq!(rows[1].title, "b");
    }

    #[test]
    fn an_unknown_op_is_ignored_rather_than_fatal() {
        // A connector written for a later Look must still work with this one.
        assert!(read(r#"{"op":"tile","id":"x"}"#).is_none());
        assert!(read("not json").is_none());
        assert!(read("").is_none());
    }

    #[test]
    fn a_preview_carries_its_content_type() {
        let say = read(
            r##"{"op":"preview","block":"myapp.notes","id":"n1","req":7,"content_type":"markdown","text":"# T"}"##,
        )
        .expect("a preview frame");
        assert_eq!(
            say,
            Say::Preview {
                block: "myapp.notes".into(),
                id: "n1".into(),
                req: Some(7),
                content_type: Content::Markdown,
                text: "# T".into(),
            }
        );
    }

    #[test]
    fn a_preview_that_omits_the_request_number_still_parses() {
        // It parses so the engine can say why it dropped it, rather than the
        // frame vanishing as unreadable.
        let say = read(r#"{"op":"preview","block":"b","id":"1","text":"x"}"#).expect("frame");
        let Say::Preview { req, .. } = say else {
            panic!("expected preview");
        };
        assert_eq!(req, None);
    }

    #[test]
    fn a_preview_without_a_content_type_is_text() {
        // What a connector written before the field existed meant.
        let say = read(r#"{"op":"preview","block":"b","id":"1","text":"plain"}"#).expect("frame");
        let Say::Preview { content_type, .. } = say else {
            panic!("expected preview");
        };
        assert_eq!(content_type, Content::Text);
    }

    #[test]
    fn an_unknown_content_type_reads_as_text() {
        // Showing the source beats blanking the panel.
        let say = read(r#"{"op":"preview","block":"b","id":"1","content_type":"html","text":"x"}"#)
            .expect("frame");
        let Say::Preview { content_type, .. } = say else {
            panic!("expected preview");
        };
        assert_eq!(content_type, Content::Text);
    }

    #[test]
    fn asking_for_a_preview_is_one_line() {
        assert_eq!(
            Ask::Preview {
                block: "myapp.notes".into(),
                id: "n1".into(),
                req: 7,
            }
            .line(),
            "{\"op\":\"preview\",\"block\":\"myapp.notes\",\"id\":\"n1\",\"req\":7}\n"
        );
    }

    #[test]
    fn a_missing_capability_list_is_no_capabilities() {
        assert_eq!(
            read(r#"{"op":"ready"}"#),
            Some(Say::Ready {
                capabilities: Vec::new()
            })
        );
    }
}
