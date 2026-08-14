//! The action planner: ONE source of truth for the prompt, the tool aliases,
//! and the model-output mapping, for every shell. Latency contract as designed
//! on macOS: temperature 0, tool alias enum in the schema, title/match-only
//! params, single-shot.
//!
//! The prompt is SHARDED by `domain::of`: a request the prefilter can place
//! sees only that domain's tools plus that domain's rules, and everything else
//! sees the whole table. The shared preamble stays the token prefix of every
//! variant so Ollama's prefix cache still covers most of the prompt.
//!
//! Date seam: the shell resolves natural time phrases (macOS: NSDataDetector)
//! AFTER this returns; for add tools the shell injects `when` = the raw query
//! when it resolves. This module never does date math on phrases.

use serde_json::{Value, json};

use crate::domain::{self, Domain};
use crate::{chat, ollama, plan};

/// One row per tool: alias the model emits, the real tool id, the domain that
/// gates it, and the prompt line describing it. Adding a tool is a row here.
///
/// Table order is prompt order and it is load-bearing: a 7B model reads the
/// list positionally, and alphabetizing these lines cost 3 points of tool
/// accuracy in the eval. Add new rows next to their domain siblings.
pub struct Tool {
    pub alias: &'static str,
    pub id: &'static str,
    pub domain: Domain,
    /// A signal the raw request must carry for this tool to be offered at all.
    /// The second half of the prefilter: narrowing by domain removes tools the
    /// request cannot want, and this removes tools it cannot support. What is
    /// never in the schema can never be emitted, which beats asking a 7B model
    /// to respect a precondition stated in prose.
    pub requires: Option<fn(&str) -> bool>,
    pub line: &'static str,
}

pub const TOOLS: [Tool; 10] = [
    Tool {
        alias: "event",
        id: "calendar.add_event",
        domain: Domain::Calendar,
        requires: None,
        line: r#"- "event": add a calendar event - ANY named activity or errand counts ("go to the office", "lunch with Sarah"). params: title (clean short title; drop the leading verb, filler words, and all date/time words; capitalize the first word)."#,
    },
    Tool {
        alias: "reminder",
        id: "reminder.add",
        domain: Domain::Reminder,
        requires: None,
        // Self-contained BY NECESSITY: the reminder shard does not include the
        // "event" line, so a "same rules" back-reference would dangle.
        line: r#"- "reminder": add a reminder. params: title (clean short title; drop the leading verb, filler words, and all date/time words; capitalize the first word)."#,
    },
    Tool {
        alias: "cancel",
        id: "calendar.cancel_event",
        domain: Domain::Calendar,
        requires: None,
        line: r#"- "cancel": remove an EXISTING event. params: match (the words that identify which event, e.g. "dentist")."#,
    },
    Tool {
        alias: "move",
        id: "calendar.move_event",
        domain: Domain::Calendar,
        requires: None,
        line: r#"- "move": reschedule an EXISTING event. params: match, when (the NEW time phrase copied verbatim, e.g. "4pm", "friday 9am")."#,
    },
    Tool {
        alias: "complete",
        id: "reminder.complete",
        domain: Domain::Reminder,
        requires: None,
        line: r#"- "complete": mark an EXISTING reminder done. params: match."#,
    },
    Tool {
        alias: "delete",
        id: "reminder.remove",
        domain: Domain::Reminder,
        requires: None,
        line: r#"- "delete": remove an EXISTING reminder from the list. params: match."#,
    },
    Tool {
        alias: "snooze",
        id: "reminder.snooze",
        domain: Domain::Reminder,
        requires: None,
        line: r#"- "snooze": push an EXISTING reminder to a later time. params: match, when (the new time phrase verbatim)."#,
    },
    Tool {
        alias: "block",
        id: "calendar.block_time",
        domain: Domain::Calendar,
        requires: Some(crate::resolve::has_duration_phrase),
        line: r#"- "block": reserve UNNAMED free/focus time, only when the request names a duration and no activity ("block 2 hours friday"). A named activity is "event", never "block". params: duration (e.g. "2 hours", "90 minutes"), when (the day/window phrase like "friday" or "this week")."#,
    },
    Tool {
        alias: "recall",
        id: "files.recall",
        domain: Domain::Files,
        requires: None,
        line: r#"- "recall": find the user's OWN files on this machine ("the pdf i downloaded", "find my screenshots from friday"). params (all optional, include what the request names): terms (file name/content words), types (kind words like "pdf", "screenshot", "image"), when (the time phrase verbatim), location ("downloads", "desktop" or "documents")."#,
    },
    Tool {
        alias: "textop",
        id: "clipboard.textop",
        domain: Domain::Clipboard,
        requires: None,
        line: r#"- "textop": transform the text on the clipboard ("make this shorter", "translate my copied text to german"). params: instruction (a one-sentence imperative, e.g. "Translate the text to German.")."#,
    },
];

const PREAMBLE: &str = concat!(
    "Classify the request into tools and extract their params. ",
    "One step per action: a request naming two actions gets two steps.\n",
    "Tools:"
);

const FOOTER: &str = r#"Pronouns and references are valid match values: "remove it" -> match "it"; "cancel this event" -> match "this event".
Reply with JSON only: {"steps":[{"tool":"...","params":{...}}]}.
If it is none of these, reply {"steps":[]}."#;

/// Rules that only load with their own domain. These are the ones a flat
/// prompt cannot afford: every rule here costs accuracy on the tools it does
/// not describe, which is why the vocabulary stalled at ten.
fn domain_rules(domain: Domain) -> &'static str {
    match domain {
        Domain::Calendar => concat!(
            "The request is about the calendar.\n",
            r#"Default to "event". Pick "cancel" or "move" only for wording about something that already exists ("cancel", "call off", "reschedule", "move ... to"), and "block" only when a LENGTH of time is stated ("2 hours"); a clock time is not a length."#,
        ),
        Domain::Reminder => concat!(
            "The request is about the reminder list, never the calendar.\n",
            r#"Adding is the default. Only pick "complete", "delete", or "snooze" when the request refers to a reminder that already exists."#,
        ),
        Domain::Clipboard => {
            "The user wants the text on their clipboard transformed. Write the instruction as an imperative about \"the text\"."
        }
        Domain::Files => "",
    }
}

/// The tools offered for a request: its domain's slice (or all of them), minus
/// any whose precondition the request does not meet. Table order (see `Tool`).
fn tools_for(domain: Option<Domain>, user: &str) -> Vec<&'static Tool> {
    TOOLS
        .iter()
        .filter(|t| domain.is_none_or(|d| t.domain == d))
        .filter(|t| t.requires.is_none_or(|met| met(user)))
        .collect()
}

/// The system prompt for one domain, or the whole vocabulary for None.
pub fn system_prompt(domain: Option<Domain>, user: &str) -> String {
    let mut out = String::from(PREAMBLE);
    for tool in tools_for(domain, user) {
        out.push('\n');
        out.push_str(tool.line);
    }
    if let Some(rules) = domain.map(domain_rules).filter(|r| !r.is_empty()) {
        out.push('\n');
        out.push_str(rules);
    }
    out.push('\n');
    out.push_str(FOOTER);
    out
}

fn alias_to_tool(alias: &str) -> Option<&'static str> {
    TOOLS.iter().find(|t| t.alias == alias).map(|t| t.id)
}

/// Request body for the planning call (also used to warm the prompt cache).
pub fn request_body(model: &str, user: &str) -> String {
    let domain = domain::of(user);
    // Prompt keeps table order; the schema enum is sorted so the same domain
    // always produces a byte-identical `format` block.
    let mut aliases: Vec<&str> = tools_for(domain, user).iter().map(|t| t.alias).collect();
    aliases.sort_unstable();
    json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system_prompt(domain, user) },
            { "role": "user", "content": user },
        ],
        "stream": false,
        // A reasoning model would spend the whole 80-token budget on hidden
        // thinking and return no JSON at all. Ignored by non-thinking models.
        "think": false,
        "options": { "temperature": 0, "num_predict": 80 },
        "keep_alive": "30m",
        "format": plan::chat_format(&aliases),
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
/// `{"done":true,"calls":[{tool,params}, ...]}` - empty for "not an action"
/// or a failure, several for a compound request.
/// None for unknown ids. Like chat, the poll that observes done removes it.
pub fn poll(id: u64) -> Option<String> {
    chat::poll(id).map(|snapshot| map_snapshot(&snapshot))
}

/// Kills the request (Ollama aborts generation on disconnect).
pub fn cancel(id: u64) {
    chat::cancel(id);
}

/// Every usable step of a plan, in order. A step the resolver rejects is
/// DROPPED rather than failing the plan: half a compound request done, with the
/// preview showing exactly which half, beats refusing the whole thing.
pub fn resolve_steps(parsed: plan::ActionPlan) -> Vec<Value> {
    parsed.steps.iter().filter_map(resolve_step).collect()
}

fn map_snapshot(snapshot: &str) -> String {
    let root: Value = serde_json::from_str(snapshot).unwrap_or(Value::Null);
    if !root["done"].as_bool().unwrap_or(false) {
        return r#"{"done":false}"#.into();
    }
    let calls = root["text"]
        .as_str()
        .and_then(plan::parse_plan)
        .map(resolve_steps)
        .unwrap_or_default();
    json!({ "done": true, "calls": calls }).to_string()
}

/// Primes the model + Ollama's prompt-prefix cache. The warm query is chosen
/// to produce the WIDEST prompt (no domain, block's precondition met), since
/// every shard is a subset of it and shares its leading tokens.
pub fn warm(host: &str, model: &str) {
    let url = format!("{}/api/chat", host.trim_end_matches('/'));
    let _ = ollama::post_json(&url, &request_body(model, "hold 2 hours"), 30);
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
        assert_eq!(root["calls"][0]["tool"], "calendar.add_event");
        assert_eq!(root["calls"][0]["params"]["title"], "Dentist");

        // A decline (empty steps) and a transport error both map to call: null.
        let decline = map_snapshot(r#"{"text":"{\"steps\":[]}","done":true}"#);
        let decline_calls = serde_json::from_str::<Value>(&decline).unwrap();
        assert_eq!(decline_calls["calls"].as_array().map(Vec::len), Some(0));
        let error = map_snapshot(r#"{"text":"","done":true,"error":"no response"}"#);
        let error_calls = serde_json::from_str::<Value>(&error).unwrap();
        assert_eq!(error_calls["calls"].as_array().map(Vec::len), Some(0));
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
        assert_eq!(root["calls"].as_array().map(Vec::len), Some(0));
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

    fn offered(user: &str) -> Vec<String> {
        let body: Value = serde_json::from_str(&request_body("m", user)).unwrap();
        serde_json::from_value(
            body["format"]["properties"]["steps"]["items"]["properties"]["tool"]["enum"].clone(),
        )
        .unwrap()
    }

    #[test]
    fn a_placed_request_sees_only_its_own_domain() {
        assert_eq!(
            offered("delete the buy milk reminder"),
            ["complete", "delete", "reminder", "snooze"]
        );
        // No length of time, so the calendar shard is block-less here.
        assert_eq!(
            offered("cancel my dentist appointment"),
            ["cancel", "event", "move"]
        );
        assert_eq!(
            offered("block 2 hours friday"),
            ["block", "cancel", "event", "move"]
        );
        assert_eq!(offered("make this shorter"), ["textop"]);

        // The prompt narrows with the schema: no stray tool descriptions.
        let prompt = system_prompt(Some(Domain::Clipboard), "make this shorter");
        assert!(prompt.contains(r#""textop""#));
        assert!(!prompt.contains(r#""event""#));
        assert!(!prompt.contains(r#""recall""#));
    }

    #[test]
    fn an_unplaceable_request_sees_every_tool() {
        // "2 hours" keeps block's precondition met, so this is the full table.
        let all = "hold 2 hours for lunch with sarah tomorrow at noon";
        assert_eq!(offered(all).len(), TOOLS.len());
        let prompt = system_prompt(None, all);
        for tool in &TOOLS {
            assert!(prompt.contains(tool.line), "{}", tool.alias);
        }
    }

    #[test]
    fn an_unmet_precondition_withholds_the_tool() {
        // No length of time stated, so "block" is not even in the vocabulary:
        // the model cannot misfile an add as a time block.
        let without_length = offered("add gym session tomorrow 6am");
        assert!(
            !without_length.iter().any(|a| a == "block"),
            "{without_length:?}"
        );
        assert!(without_length.iter().any(|a| a == "event"));
        assert!(!system_prompt(None, "add gym session tomorrow 6am").contains(r#""block""#));

        assert!(offered("block 2 hours friday").iter().any(|a| a == "block"));
    }

    #[test]
    fn every_prompt_shares_the_preamble_and_footer() {
        // The shared prefix is what keeps Ollama's prompt cache useful across
        // shards, and the footer is what keeps the decline escape hatch.
        for domain in [
            None,
            Some(Domain::Calendar),
            Some(Domain::Reminder),
            Some(Domain::Files),
            Some(Domain::Clipboard),
        ] {
            let prompt = system_prompt(domain, "block 2 hours friday");
            assert!(prompt.starts_with(PREAMBLE));
            assert!(prompt.ends_with(FOOTER));
        }
    }

    #[test]
    fn aliases_and_ids_are_unique() {
        let mut aliases: Vec<&str> = TOOLS.iter().map(|t| t.alias).collect();
        aliases.sort_unstable();
        let count = aliases.len();
        aliases.dedup();
        assert_eq!(aliases.len(), count);
        for tool in &TOOLS {
            assert_eq!(alias_to_tool(tool.alias), Some(tool.id));
            assert!(tool.line.contains(tool.alias), "{}", tool.alias);
        }
    }
}
