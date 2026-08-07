//! The planner's wire format (see docs/ai-action-contracts.md): the model emits
//! `{"steps":[{"tool":"<alias>","params":{...}}]}` forced by a JSON Schema in
//! Ollama's `format` field. `steps` is an array from day one so multi-action
//! plans are a consumer change, never a wire change.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct PlanStep {
    pub tool: String,
    #[serde(default)]
    pub params: serde_json::Map<String, Value>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct ActionPlan {
    pub steps: Vec<PlanStep>,
}

/// Decodes the `message.content` JSON string of a non-streamed `/api/chat`
/// response into an `ActionPlan`. None on any shape mismatch, so a garbled
/// response reads as "no plan".
pub fn parse_chat_response(body: &str) -> Option<ActionPlan> {
    let root: Value = serde_json::from_str(body).ok()?;
    let content = root.get("message")?.get("content")?.as_str()?;
    serde_json::from_str(content).ok()
}

/// The JSON Schema handed to Ollama's `format` field. `tool` is an enum of the
/// registered aliases so the model cannot invent one; `params` carries the
/// minimal vocabulary (title/match/when), with per-tool requirements validated
/// in tool resolution, not the schema.
pub fn chat_format(tool_aliases: &[&str]) -> Value {
    json!({
        "type": "object",
        "properties": {
            "steps": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "tool": { "type": "string", "enum": tool_aliases },
                        "params": {
                            "type": "object",
                            "properties": {
                                "title": { "type": "string" },
                                "match": { "type": "string" },
                                "when": { "type": "string" },
                                "duration": { "type": "string" },
                            },
                            "required": [],
                        },
                    },
                    "required": ["tool", "params"],
                },
            },
        },
        "required": ["steps"],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_steps_from_chat_content() {
        let body = r#"{"message":{"role":"assistant","content":"{\"steps\":[{\"tool\":\"event\",\"params\":{\"title\":\"Dentist\"}}]}"}}"#;
        let plan = parse_chat_response(body).unwrap();
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].tool, "event");
        assert_eq!(plan.steps[0].params["title"], "Dentist");
    }

    #[test]
    fn empty_steps_is_a_decline() {
        let body = r#"{"message":{"content":"{\"steps\":[]}"}}"#;
        assert!(parse_chat_response(body).unwrap().steps.is_empty());
    }

    #[test]
    fn garbage_is_none() {
        assert!(parse_chat_response(r#"{"message":{"content":"sorry"}}"#).is_none());
        assert!(parse_chat_response("{}").is_none());
        assert!(parse_chat_response("not json").is_none());
    }

    #[test]
    fn schema_constrains_tool_and_params() {
        let schema = chat_format(&["cancel", "event"]);
        let step = &schema["properties"]["steps"]["items"];
        assert_eq!(
            step["properties"]["tool"]["enum"],
            json!(["cancel", "event"])
        );
        let params = &step["properties"]["params"];
        assert_eq!(params["required"], json!([]));
        assert!(params["properties"].get("title").is_some());
        assert!(params["properties"].get("match").is_some());
        assert!(params["properties"].get("when").is_some());
    }
}
