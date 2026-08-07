//! The action planner: ONE source of truth for the prompt, the tool aliases,
//! and the model-output mapping, for every shell. Latency contract as designed
//! on macOS: static prompt (Ollama prompt-prefix cache), temperature 0, tool
//! alias enum in the schema, title/match-only params, single-shot.
//!
//! Date seam: the shell resolves natural time phrases (macOS: NSDataDetector)
//! AFTER this returns; for add tools the shell injects `when` = the raw query
//! when it resolves. This module never does date math on phrases.

use serde_json::{Value, json};

use crate::{ollama, plan};

pub const ALIASES: [(&str, &str); 8] = [
    ("event", "calendar.add_event"),
    ("reminder", "reminder.add"),
    ("cancel", "calendar.cancel_event"),
    ("move", "calendar.move_event"),
    ("complete", "reminder.complete"),
    ("delete", "reminder.remove"),
    ("snooze", "reminder.snooze"),
    ("block", "calendar.block_time"),
];

pub const SYSTEM_PROMPT: &str = r#"Classify the request into ONE tool and extract its params:
- "event": add a calendar event. params: title (clean short title; drop the leading verb, filler words, and all date/time words; capitalize the first word).
- "reminder": add a reminder. params: title (same rules).
- "cancel": remove an EXISTING event. params: match (the words that identify which event, e.g. "dentist").
- "move": reschedule an EXISTING event. params: match, when (the NEW time phrase copied verbatim, e.g. "4pm", "friday 9am").
- "complete": mark an EXISTING reminder done. params: match.
- "delete": remove an EXISTING reminder from the list. params: match.
- "snooze": push an EXISTING reminder to a later time. params: match, when (the new time phrase verbatim).
- "block": reserve free focus time. params: duration (e.g. "2 hours", "90 minutes"), when (the day/window phrase like "friday" or "this week").
Pronouns and references are valid match values: "remove it" -> match "it"; "cancel this event" -> match "this event".
Reply with JSON only: {"steps":[{"tool":"...","params":{...}}]}.
If it is none of these, reply {"steps":[]}."#;

fn alias_to_tool(alias: &str) -> Option<&'static str> {
    ALIASES.iter().find(|(a, _)| *a == alias).map(|(_, t)| *t)
}

fn sorted_aliases() -> Vec<&'static str> {
    let mut aliases: Vec<&str> = ALIASES.iter().map(|(a, _)| *a).collect();
    aliases.sort_unstable();
    aliases
}

/// Request body for the planning call (also used to warm the prompt cache).
pub fn request_body(model: &str, user: &str) -> String {
    json!({
        "model": model,
        "messages": [
            { "role": "system", "content": SYSTEM_PROMPT },
            { "role": "user", "content": user },
        ],
        "stream": false,
        "options": { "temperature": 0, "num_predict": 80 },
        "keep_alive": "30m",
        "format": plan::chat_format(&sorted_aliases()),
    })
    .to_string()
}

/// Maps a model step to `{tool, params}` (real tool id, validated fields), or
/// None when unusable. The shell adds the date-injection for add tools.
pub fn resolve_step(step: &plan::PlanStep) -> Option<Value> {
    let tool = alias_to_tool(&step.tool)?;
    let get = |key: &str| -> Option<String> {
        step.params
            .get(key)
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
    };

    let params = match tool {
        "calendar.add_event" | "reminder.add" => json!({ "title": get("title")? }),
        "calendar.move_event" | "reminder.snooze" => {
            json!({ "match": get("match")?, "when": get("when")? })
        }
        "calendar.block_time" => {
            let mut p = json!({ "duration": get("duration")? });
            if let Some(when) = get("when") {
                p["when"] = Value::String(when);
            }
            if let Some(title) = get("title") {
                p["title"] = Value::String(title);
            }
            p
        }
        _ => json!({ "match": get("match").or_else(|| get("title"))? }),
    };
    Some(json!({ "tool": tool, "params": params }))
}

/// Full planning call: POST, parse, map. None = no capable answer / not an
/// action (empty steps) / transport failure. Blocking (call off-thread).
pub fn plan(host: &str, model: &str, query: &str) -> Option<Value> {
    let url = format!("{}/api/chat", host.trim_end_matches('/'));
    let body = ollama::post_json(&url, &request_body(model, query), 30)?;
    let parsed = plan::parse_chat_response(&body)?;
    let step = parsed.steps.first()?;
    resolve_step(step)
}

/// Primes the model + Ollama's prompt-prefix cache with the exact planner
/// prompt so the first real plan skips load and prompt processing.
pub fn warm(host: &str, model: &str) {
    let url = format!("{}/api/chat", host.trim_end_matches('/'));
    let _ = ollama::post_json(&url, &request_body(model, "hi"), 30);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(tool: &str, params: Value) -> plan::PlanStep {
        plan::PlanStep {
            tool: tool.into(),
            params: params.as_object().cloned().unwrap_or_default(),
        }
    }

    #[test]
    fn add_needs_title() {
        let call = resolve_step(&step("event", json!({"title": "Dentist"}))).unwrap();
        assert_eq!(call["tool"], "calendar.add_event");
        assert_eq!(call["params"]["title"], "Dentist");
        assert!(resolve_step(&step("event", json!({}))).is_none());
    }

    #[test]
    fn move_needs_match_and_when() {
        let call = resolve_step(&step("move", json!({"match": "sync", "when": "4pm"}))).unwrap();
        assert_eq!(call["tool"], "calendar.move_event");
        assert_eq!(call["params"]["when"], "4pm");
        assert!(resolve_step(&step("move", json!({"match": "sync"}))).is_none());
    }

    #[test]
    fn mutates_accept_title_as_match_fallback() {
        let call = resolve_step(&step("cancel", json!({"title": "dentist"}))).unwrap();
        assert_eq!(call["tool"], "calendar.cancel_event");
        assert_eq!(call["params"]["match"], "dentist");
        let del = resolve_step(&step("delete", json!({"match": "walk dog"}))).unwrap();
        assert_eq!(del["tool"], "reminder.remove");
    }

    #[test]
    fn snooze_needs_match_and_when() {
        let call = resolve_step(&step("snooze", json!({"match": "milk", "when": "8am"}))).unwrap();
        assert_eq!(call["tool"], "reminder.snooze");
        assert_eq!(call["params"]["when"], "8am");
    }

    #[test]
    fn block_needs_duration() {
        let call = resolve_step(&step(
            "block",
            json!({"duration": "2 hours", "when": "friday"}),
        ))
        .unwrap();
        assert_eq!(call["tool"], "calendar.block_time");
        assert_eq!(call["params"]["duration"], "2 hours");
        assert_eq!(call["params"]["when"], "friday");
        assert!(resolve_step(&step("block", json!({"when": "friday"}))).is_none());
    }

    #[test]
    fn unknown_alias_is_none() {
        assert!(resolve_step(&step("bogus", json!({"match": "x"}))).is_none());
    }

    #[test]
    fn request_body_shape() {
        let body: Value = serde_json::from_str(&request_body("m", "q")).unwrap();
        assert_eq!(body["options"]["temperature"], 0);
        assert_eq!(body["stream"], false);
        assert_eq!(body["messages"][0]["role"], "system");
        assert!(body["format"]["properties"]["steps"].is_object());
    }
}
