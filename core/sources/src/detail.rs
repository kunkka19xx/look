//! What fills the panel for a row, as blocks rather than as one string.
//!
//! A `preview` command may print these instead of text, and a connector will
//! answer with the same shape. Every block is something the app already draws:
//! the fields and list come from a control's info, code from the steps.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetailField {
    pub label: String,
    pub value: String,
}

/// Deliberately the shape of a control's `QuickActionListItem`: an item with an
/// `id` can be acted on, one without is a read-only row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetailItem {
    #[serde(default)]
    pub id: Option<String>,
    pub label: String,
    #[serde(default)]
    pub on: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DetailBlock {
    Fields {
        rows: Vec<DetailField>,
    },
    List {
        items: Vec<DetailItem>,
    },
    Text {
        text: String,
    },
    Code {
        #[serde(default)]
        language: Option<String>,
        text: String,
    },
}

/// Blocks, when the output is a list of them. Anything else is `None` and stays
/// the text it always was: a command printing its own JSON must not be read as
/// a layout because it happened to start with a bracket.
pub fn parse(text: &str) -> Option<Vec<DetailBlock>> {
    let text = text.trim();
    if !text.starts_with('[') {
        return None;
    }
    let blocks: Vec<DetailBlock> = serde_json::from_str(text).ok()?;
    (!blocks.is_empty()).then_some(blocks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_list_of_blocks_parses() {
        let blocks = parse(
            r#"[{"type":"fields","rows":[{"label":"Vault","value":"look"}]},
                {"type":"list","items":[{"label":"one"},{"id":"2","label":"two","on":true}]}]"#,
        )
        .expect("blocks");
        assert_eq!(blocks.len(), 2);
        assert_eq!(
            blocks[0],
            DetailBlock::Fields {
                rows: vec![DetailField {
                    label: "Vault".into(),
                    value: "look".into()
                }]
            }
        );
        let DetailBlock::List { items } = &blocks[1] else {
            panic!("second block is a list");
        };
        assert_eq!(items[0].id, None);
        assert_eq!(items[1].on, Some(true));
    }

    #[test]
    fn other_json_is_not_a_layout() {
        // A command that prints its own JSON keeps being text.
        assert!(parse(r#"[{"name":"main","sha":"abc"}]"#).is_none());
        assert!(parse(r#"{"type":"fields","rows":[]}"#).is_none());
        assert!(parse("plain output").is_none());
        assert!(parse("[]").is_none());
    }

    #[test]
    fn an_unknown_block_type_refuses_the_whole_layout() {
        // Half a panel is worse than the text the command meant to print.
        assert!(parse(r#"[{"type":"fields","rows":[]},{"type":"chart"}]"#).is_none());
    }
}
