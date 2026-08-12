//! The action planner: ONE source of truth for the prompt, the tool aliases,
//! and the model-output mapping, for every shell. Latency contract as designed
//! on macOS: static prompt (Ollama prompt-prefix cache), temperature 0, tool
//! alias enum in the schema, title/match-only params, single-shot.
//!
//! Date seam: the shell resolves natural time phrases (macOS: NSDataDetector)
//! AFTER this returns; for add tools the shell injects `when` = the raw query
//! when it resolves. This module never does date math on phrases.

use serde_json::{Value, json};

use crate::{chat, ollama, plan};

pub const ALIASES: [(&str, &str); 10] = [
    ("event", "calendar.add_event"),
    ("reminder", "reminder.add"),
    ("cancel", "calendar.cancel_event"),
    ("move", "calendar.move_event"),
    ("complete", "reminder.complete"),
    ("delete", "reminder.remove"),
    ("snooze", "reminder.snooze"),
    ("block", "calendar.block_time"),
    ("recall", "files.recall"),
    ("textop", "clipboard.textop"),
];

pub const SYSTEM_PROMPT: &str = r#"Classify the request into ONE tool and extract its params:
- "event": add a calendar event - ANY named activity or errand counts ("go to the office", "lunch with Sarah"). params: title (clean short title; drop the leading verb, filler words, and all date/time words; capitalize the first word).
- "reminder": add a reminder. params: title (same rules).
- "cancel": remove an EXISTING event. params: match (the words that identify which event, e.g. "dentist").
- "move": reschedule an EXISTING event. params: match, when (the NEW time phrase copied verbatim, e.g. "4pm", "friday 9am").
- "complete": mark an EXISTING reminder done. params: match.
- "delete": remove an EXISTING reminder from the list. params: match.
- "snooze": push an EXISTING reminder to a later time. params: match, when (the new time phrase verbatim).
- "block": reserve UNNAMED free/focus time, only when the request names a duration and no activity ("block 2 hours friday"). A named activity is "event", never "block". params: duration (e.g. "2 hours", "90 minutes"), when (the day/window phrase like "friday" or "this week").
- "recall": find the user's OWN files on this machine ("the pdf i downloaded", "find my screenshots from friday"). params (all optional, include what the request names): terms (file name/content words), types (kind words like "pdf", "screenshot", "image"), when (the time phrase verbatim), location ("downloads", "desktop" or "documents").
- "textop": transform the text on the clipboard ("make this shorter", "translate my copied text to german"). params: instruction (a one-sentence imperative, e.g. "Translate the text to German.").
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
        // A reasoning model would spend the whole 80-token budget on hidden
        // thinking and return no JSON at all. Ignored by non-thinking models.
        "think": false,
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
        "files.recall" => {
            // At least one facet, or it is not a usable recall.
            let mut p = json!({});
            for key in ["terms", "types", "when", "location"] {
                if let Some(value) = get(key) {
                    p[key] = Value::String(value);
                }
            }
            let empty = p.as_object().is_some_and(|o| o.is_empty());
            if empty {
                return None;
            }
            p
        }
        "clipboard.textop" => json!({ "instruction": get("instruction")? }),
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

/// Starts a cancellable planning session on the chat transport (the response
/// is one line; the session's accumulated text is the plan JSON). Returns a
/// session id for `poll`/`cancel`, or 0 when the request could not be spawned.
pub fn start(host: &str, model: &str, query: &str) -> u64 {
    let url = format!("{}/api/chat", host.trim_end_matches('/'));
    chat::start_request(&url, &request_body(model, query), 30)
}

/// Snapshot of a planning session: `{"done":false}` while in flight, then
/// `{"done":true,"call":{tool,params}|null}` (null = not an action / failure).
/// None for unknown ids. Like chat, the poll that observes done removes it.
pub fn poll(id: u64) -> Option<String> {
    chat::poll(id).map(|snapshot| map_snapshot(&snapshot))
}

/// Kills the request (Ollama aborts generation on disconnect).
pub fn cancel(id: u64) {
    chat::cancel(id);
}

fn map_snapshot(snapshot: &str) -> String {
    let root: Value = serde_json::from_str(snapshot).unwrap_or(Value::Null);
    if !root["done"].as_bool().unwrap_or(false) {
        return r#"{"done":false}"#.into();
    }
    let call = root["text"]
        .as_str()
        .and_then(|content| serde_json::from_str::<plan::ActionPlan>(content).ok())
        .and_then(|parsed| parsed.steps.into_iter().next())
        .and_then(|step| resolve_step(&step));
    json!({ "done": true, "call": call }).to_string()
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
    fn recall_needs_at_least_one_facet() {
        let call = resolve_step(&step(
            "recall",
            json!({"types": "pdf", "when": "last week", "location": "downloads"}),
        ))
        .unwrap();
        assert_eq!(call["tool"], "files.recall");
        assert_eq!(call["params"]["types"], "pdf");
        assert_eq!(call["params"]["when"], "last week");
        assert!(call["params"].get("terms").is_none());
        assert!(resolve_step(&step("recall", json!({}))).is_none());
    }

    #[test]
    fn textop_needs_instruction() {
        let call = resolve_step(&step(
            "textop",
            json!({"instruction": "Translate the text to German."}),
        ))
        .unwrap();
        assert_eq!(call["tool"], "clipboard.textop");
        assert!(resolve_step(&step("textop", json!({}))).is_none());
    }

    #[test]
    fn snapshot_maps_pending_plan_decline_and_error() {
        assert_eq!(
            map_snapshot(r#"{"text":"","done":false}"#),
            r#"{"done":false}"#
        );

        let done = map_snapshot(
            r#"{"text":"{\"steps\":[{\"tool\":\"event\",\"params\":{\"title\":\"Dentist\"}}]}","done":true}"#,
        );
        let root: Value = serde_json::from_str(&done).unwrap();
        assert_eq!(root["done"], true);
        assert_eq!(root["call"]["tool"], "calendar.add_event");
        assert_eq!(root["call"]["params"]["title"], "Dentist");

        // A decline (empty steps) and a transport error both map to call: null.
        let decline = map_snapshot(r#"{"text":"{\"steps\":[]}","done":true}"#);
        assert!(serde_json::from_str::<Value>(&decline).unwrap()["call"].is_null());
        let error = map_snapshot(r#"{"text":"","done":true,"error":"no response"}"#);
        assert!(serde_json::from_str::<Value>(&error).unwrap()["call"].is_null());
    }

    #[test]
    fn failed_session_reaches_done_with_null_call() {
        // Connection refused: the session finishes with done and no call.
        let id = start("http://127.0.0.1:1", "m", "add lunch");
        assert_ne!(id, 0);
        let mut last = String::new();
        for _ in 0..100 {
            let Some(snapshot) = poll(id) else { break };
            last = snapshot;
            if last.contains("\"done\":true") {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let root: Value = serde_json::from_str(&last).unwrap();
        assert_eq!(root["done"], true);
        assert!(root["call"].is_null());
        // The poll that observed done removed the session.
        assert!(poll(id).is_none());
    }

    #[test]
    fn cancel_removes_the_session() {
        let id = start("http://127.0.0.1:1", "m", "add lunch");
        assert_ne!(id, 0);
        cancel(id);
        assert!(poll(id).is_none());
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
