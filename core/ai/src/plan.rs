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
    parse_plan(content)
}

/// Decodes a plan from model text, tolerating the two ways small local models
/// break their own JSON even under a schema: junk after the value (a stray
/// code fence) and a missing closing brace. Measured on qwen3.5:4b, where a
/// strict decode threw away otherwise-correct plans as declines.
pub fn parse_plan(content: &str) -> Option<ActionPlan> {
    let content = content.trim().trim_start_matches("```json").trim();
    // Reads the first complete value and ignores whatever trails it.
    let first = serde_json::Deserializer::from_str(content)
        .into_iter::<ActionPlan>()
        .next()
        .and_then(Result::ok);
    first.or_else(|| serde_json::from_str(&closed(content)?).ok())
}

/// The text with any still-open braces and brackets closed, or None when
/// nothing was left open (so a genuinely malformed value is not retried).
fn closed(content: &str) -> Option<String> {
    let mut stack: Vec<char> = Vec::new();
    let (mut in_string, mut escaped) = (false, false);
    for c in content.chars() {
        match c {
            _ if escaped => escaped = false,
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            '{' | '[' if !in_string => stack.push(if c == '{' { '}' } else { ']' }),
            // A closer that does not match the open one is malformed beyond
            // repair, not an unfinished value.
            '}' | ']' if !in_string => match stack.pop() {
                Some(open) if open == c => {}
                _ => return None,
            },
            _ => {}
        }
    }
    if in_string || stack.is_empty() {
        return None;
    }
    let mut out = String::from(content);
    out.extend(stack.iter().rev());
    Some(out)
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
                                "terms": { "type": "string" },
                                "types": { "type": "string" },
                                "location": { "type": "string" },
                                "instruction": { "type": "string" },
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
    fn recovers_the_plans_small_models_mangle() {
        // Trailing junk after a complete value (qwen3.5:4b emits a lone fence).
        let fenced =
            parse_plan(r#"{"steps":[{"tool":"reminder","params":{"title":"Buy milk"}}]}`"#)
                .unwrap();
        assert_eq!(fenced.steps[0].tool, "reminder");
        // Missing the closing brace of the outer object.
        let unclosed =
            parse_plan(r#"{"steps":[{"tool":"reminder","params":{"title":"buy milk"}}]"#).unwrap();
        assert_eq!(unclosed.steps[0].params["title"], "buy milk");
        let fence =
            parse_plan("```json\n{\"steps\":[{\"tool\":\"cancel\",\"params\":{}}]}").unwrap();
        assert_eq!(fence.steps[0].tool, "cancel");
        // A brace inside a string is text, not structure, so only the real
        // outer brace is added back.
        let braces = parse_plan(
            r#"{"steps":[{"tool":"textop","params":{"instruction":"Close the { brace"}}]"#,
        )
        .unwrap();
        assert_eq!(braces.steps[0].params["instruction"], "Close the { brace");
        // A generation cut off mid-string is NOT repaired: inventing the rest
        // of an instruction or title would be worse than declining.
        assert!(
            parse_plan(r#"{"steps":[{"tool":"textop","params":{"instruction":"Trans"#).is_none()
        );
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
