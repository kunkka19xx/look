//! ONE routing ladder for submitted AI-mode input, shared by every shell so
//! the precedence is code, not convention:
//!
//! ```text
//! memory -> join -> call -> textop -> files -> explicit -> plan -> chat
//! ```
//!
//! Deterministic tiers run first, most-precise first (memory and textop match
//! exact prefixes/verbs; the files parser is looser). `plan` means "ask the
//! model, then fall through to chat" and only appears when a model is
//! available. Memory is deterministic-only BY DESIGN: the model never writes
//! memory, so a weak planner can't pollute durable facts. An `explicit` call
//! the shell fails to resolve falls through to plan/chat shell-side (target
//! resolution needs the shell's date seam and calendar stores).

use std::path::Path;

use serde_json::json;

use crate::{calling, explicit, files, meeting, memory, textops};

/// Route `input` (the `>` already consumed) and return the decision as JSON:
/// `{"route":"memory","feedback":...}` | `{"route":"join","name":...}` |
/// `{"route":"call","name":...,"modality":...}` |
/// `{"route":"textop","label":...,"instruction":...}` | `{"route":"files"}` |
/// `{"route":"explicit","call":{tool,params}}` | `{"route":"plan"}` |
/// `{"route":"chat"}`.
/// The memory tier executes the command (it is the handler, not a preview).
pub fn route_json(
    memory_path: &Path,
    input: &str,
    model_available: bool,
    now_epoch: i64,
) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return json!({ "route": "chat" }).to_string();
    }
    if let Some(feedback) = memory::handle_command(memory_path, trimmed) {
        return json!({ "route": "memory", "feedback": feedback }).to_string();
    }
    // Above the planner, which reads "join the standup" as ADD an event called
    // "the standup" - a confirm bar for a meeting that already exists. The
    // shell resolves the name against the calendar and falls through to chat
    // when nothing answers to it, so an unmatched name costs nothing.
    if let Some(request) = meeting::join_query(trimmed) {
        return json!({ "route": "join", "name": request.name }).to_string();
    }
    // Same reasoning as `join`: asked to plan "call mom", a 7B model proposes
    // adding an EVENT called "call mom". The shell resolves the name against
    // the address book.
    if let Some(request) = calling::call_query(trimmed) {
        return json!({
            "route": "call",
            "name": request.name,
            "modality": request.modality.map(|modality| modality.id()),
        })
        .to_string();
    }
    if let Some(op) = textops::parse(trimmed) {
        return json!({ "route": "textop", "label": op.label, "instruction": op.instruction })
            .to_string();
    }
    if files::parse(trimmed, now_epoch).is_some() {
        return json!({ "route": "files" }).to_string();
    }
    if let Some(call) = explicit::parse(&format!(">{trimmed}"), model_available) {
        return json!({ "route": "explicit", "call": call }).to_string();
    }
    if model_available {
        return json!({ "route": "plan" }).to_string();
    }
    json!({ "route": "chat" }).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn temp_memory() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("look-route-test-{}.json", std::process::id()))
    }

    fn decoded(json: &str) -> Value {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn ladder_precedence() {
        let path = temp_memory();
        let _ = std::fs::remove_file(&path);
        const NOW: i64 = 1_754_000_000;

        let memory = decoded(&route_json(&path, "remember I prefer metric", true, NOW));
        assert_eq!(memory["route"], "memory");
        assert!(memory["feedback"].as_str().unwrap().contains("metric"));

        let textop = decoded(&route_json(&path, "summarize", true, NOW));
        assert_eq!(textop["route"], "textop");
        assert!(!textop["instruction"].as_str().unwrap().is_empty());

        let files = decoded(&route_json(&path, "pdfs from last week", true, NOW));
        assert_eq!(files["route"], "files");

        let explicit = decoded(&route_json(&path, "add lunch @ 12pm", true, NOW));
        assert_eq!(explicit["route"], "explicit");
        assert_eq!(explicit["call"]["tool"], "calendar.add_event");

        let plan = decoded(&route_json(&path, "move my dentist to friday", true, NOW));
        assert_eq!(plan["route"], "plan");

        let chat = decoded(&route_json(&path, "move my dentist to friday", false, NOW));
        assert_eq!(chat["route"], "chat");
        assert_eq!(
            decoded(&route_json(&path, "   ", true, NOW))["route"],
            "chat"
        );

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn join_routes_ahead_of_the_planner() {
        let path = temp_memory();
        let _ = std::fs::remove_file(&path);
        const NOW: i64 = 1_754_000_000;

        let bare = decoded(&route_json(&path, "join", true, NOW));
        assert_eq!(bare["route"], "join");
        assert!(bare["name"].is_null());

        // The phrasing that the planner used to turn into "add an event called
        // Testing Meeting".
        let named = decoded(&route_json(&path, "Join Testing Meeting", true, NOW));
        assert_eq!(named["route"], "join");
        assert_eq!(named["name"], "testing");

        // Still routed with no model configured: it never needed one.
        assert_eq!(
            decoded(&route_json(&path, "join", false, NOW))["route"],
            "join"
        );

        // "remember to join the standup" is a memory write, not a join.
        let memory = decoded(&route_json(
            &path,
            "remember to join the standup",
            true,
            NOW,
        ));
        assert_eq!(memory["route"], "memory");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn call_routes_ahead_of_the_planner() {
        let path = temp_memory();
        let _ = std::fs::remove_file(&path);
        const NOW: i64 = 1_754_000_000;

        // The phrasing the planner would otherwise turn into "add an event
        // called call mom".
        let bare = decoded(&route_json(&path, "call mom", true, NOW));
        assert_eq!(bare["route"], "call");
        assert_eq!(bare["name"], "mom");
        assert!(
            bare["modality"].is_null(),
            "unsaid, for the shell to default"
        );

        let named = decoded(&route_json(&path, "facetime sarah lee", true, NOW));
        assert_eq!(named["route"], "call");
        assert_eq!(named["name"], "sarah lee");
        assert_eq!(named["modality"], "face_time_video");

        // "remind me to call mom @ 5pm" is a reminder, not a call: the line
        // does not OPEN with the verb.
        let reminder = decoded(&route_json(&path, "remind me to call mom @ 5pm", true, NOW));
        assert_eq!(reminder["route"], "explicit");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn memory_wins_over_files_wording() {
        // "remember ..." containing file words must store a fact, not search.
        let path = temp_memory();
        let _ = std::fs::remove_file(&path);
        let out = decoded(&route_json(
            &path,
            "remember I keep the tax pdfs in documents",
            true,
            1_754_000_000,
        ));
        assert_eq!(out["route"], "memory");
        let _ = std::fs::remove_file(&path);
    }
}
