//! Streaming chat sessions on the curl transport (P5): `start` spawns a curl
//! child POSTing to Ollama's `/api/chat` with `stream:true` and a reader
//! thread accumulates cumulative text; shells `poll` for snapshots (an easy
//! fit for both FFI and Tauri) and `cancel` kills the child. No async runtime.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use serde_json::{Value, json};

use crate::ollama;

#[derive(Default)]
struct SessionState {
    text: String,
    done: bool,
    error: Option<String>,
    /// The `num_predict` cap ended the answer mid-thought.
    truncated: bool,
}

struct Session {
    state: Arc<Mutex<SessionState>>,
    child: Arc<Mutex<Option<Child>>>,
}

static SESSIONS: OnceLock<Mutex<HashMap<u64, Session>>> = OnceLock::new();
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn sessions() -> MutexGuard<'static, HashMap<u64, Session>> {
    SESSIONS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// A session that is already finished and carrying `message`, so a caller that
/// cannot even be attempted reports WHY through the normal poll path instead of
/// looking like a silent model failure.
fn failed_session(message: &str) -> u64 {
    let state = Arc::new(Mutex::new(SessionState {
        done: true,
        error: Some(message.to_string()),
        ..SessionState::default()
    }));
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    sessions().insert(
        id,
        Session {
            state,
            child: Arc::new(Mutex::new(None)),
        },
    );
    id
}

/// Kill + wait so a child never outlives its session as a zombie. Both are
/// needed: kill alone leaves the exited process unreaped.
fn reap(slot: &Mutex<Option<Child>>) {
    let mut slot = slot.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(mut child) = slot.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

/// Starts a streamed chat; `messages_json` is the full message array
/// (system/context/history assembled by the shell). `options_json` tunes the
/// generation per surface (`{num_predict, temperature, timeout_secs}`, any
/// subset; empty for defaults) so every Ollama caller shares this one
/// transport instead of hand-rolling its own HTTP. Returns a session id, or 0
/// when the request could not even be spawned.
pub fn start(host: &str, model: &str, messages_json: &str, options_json: &str) -> u64 {
    // A blank host builds the URL "/api/chat", which curl cannot resolve. The
    // reader then sees zero lines and reports "returned no answer", blaming the
    // model for a setting that was never filled in.
    if host.trim().is_empty() {
        return failed_session("no Ollama host is configured (Settings > AI)");
    }
    let Ok(messages) = serde_json::from_str::<Value>(messages_json) else {
        return 0;
    };
    let opts: Value = serde_json::from_str(options_json).unwrap_or(Value::Null);
    let num_predict = opts
        .get("num_predict")
        .and_then(|v| v.as_i64())
        .unwrap_or(512);
    let temperature = opts
        .get("temperature")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let timeout_secs = opts
        .get("timeout_secs")
        .and_then(|v| v.as_u64())
        .unwrap_or(300) as u32;
    // Ollama defaults the context to 4096 and SILENTLY truncates a prompt past
    // it, so a surface that attaches a file has to ask for the room it needs.
    // Omitted unless the caller sets it: a needlessly large KV cache costs
    // memory on every request.
    let num_ctx = opts.get("num_ctx").and_then(|v| v.as_i64());
    let mut options = json!({ "temperature": temperature, "num_predict": num_predict });
    if let Some(num_ctx) = num_ctx {
        options["num_ctx"] = json!(num_ctx);
    }
    let body = json!({
        "model": model,
        "messages": messages,
        "stream": true,
        // Reasoning models (qwen3, deepseek-r1, ...) emit hidden `thinking`
        // tokens by default. In a launcher that is pure cost: the budget is
        // spent before any answer text appears, so a planner capped at 80
        // tokens returns NOTHING and a chat answer arrives truncated. Ollama
        // ignores this field for models without a thinking mode.
        "think": false,
        "options": options,
        "keep_alive": "30m",
    })
    .to_string();
    let url = format!("{}/api/chat", host.trim_end_matches('/'));
    start_request(&url, &body, timeout_secs)
}

/// Bounds the connect phase only, so an unreachable host fails in seconds
/// rather than holding the UI for the whole answer timeout.
pub(crate) const CONNECT_TIMEOUT_SECS: u32 = 5;

/// Spawns a curl session POSTing `body` to `url`; the reader thread accumulates
/// `message.content` deltas (streamed NDJSON and single-response lines both
/// parse). Returns a pollable session id, or 0 when the spawn/write fails.
pub(crate) fn start_request(url: &str, body: &str, max_time_secs: u32) -> u64 {
    // No `--fail`: a non-2xx body is Ollama's own `{"error":"..."}` JSON, which
    // the reader surfaces instead of discarding.
    let mut command = Command::new("curl");
    command
        .arg("-sS")
        .arg("--no-buffer")
        .arg("--connect-timeout")
        .arg(CONNECT_TIMEOUT_SECS.to_string())
        .arg("--max-time")
        .arg(max_time_secs.to_string())
        .arg("-H")
        .arg("Content-Type: application/json")
        .arg("-X")
        .arg("POST")
        .arg("--data-binary")
        .arg("@-")
        .arg(url)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    let Ok(mut child) = command.spawn() else {
        return 0;
    };
    let stdin = child.stdin.take();
    let stdout = child.stdout.take();
    let child_slot = Arc::new(Mutex::new(Some(child)));
    let write_ok = stdin
        .map(|mut stdin| stdin.write_all(body.as_bytes()).is_ok())
        .unwrap_or(false);
    let Some(stdout) = stdout.filter(|_| write_ok) else {
        reap(&child_slot);
        return 0;
    };

    let state = Arc::new(Mutex::new(SessionState::default()));
    let reader_state = Arc::clone(&state);
    let reader_child = Arc::clone(&child_slot);
    std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        let mut saw_done = false;
        let mut saw_thinking = false;
        for line in reader.lines() {
            let Ok(line) = line else { break };
            let Some(parsed) = ollama::parse_stream_line(&line) else {
                continue;
            };
            if let Some(error) = parsed.error {
                if let Ok(mut s) = reader_state.lock() {
                    s.error = Some(error);
                }
                break;
            }
            saw_thinking |= parsed.thinking;
            if !parsed.delta.is_empty()
                && let Ok(mut s) = reader_state.lock()
            {
                s.text.push_str(&parsed.delta);
            }
            if parsed.done {
                saw_done = true;
                if parsed.truncated
                    && let Ok(mut s) = reader_state.lock()
                {
                    s.truncated = true;
                }
                break;
            }
        }
        // Reap here so the child never outlives the stream, even when no poll
        // ever observes `done` (the map entry may linger; the process may not).
        reap(&reader_child);
        if let Ok(mut s) = reader_state.lock() {
            // Empty text IS no response, even when lines parsed fine: a
            // reasoning model with thinking left on emits only hidden tokens,
            // which would otherwise leave the placeholder spinning forever.
            // Name that case, or it reads as an unexplained failure of the
            // model rather than a budget spent somewhere invisible.
            if s.error.is_none() && s.text.is_empty() {
                s.error = Some(if saw_thinking {
                    "the model spent its whole token budget on hidden reasoning".into()
                } else {
                    "the model returned no answer".to_string()
                });
            } else if s.error.is_none() && !saw_done && !s.text.is_empty() {
                // The stream died mid-answer (e.g. curl's --max-time): partial
                // text must not read as a complete answer.
                s.error = Some("connection ended before the answer finished".into());
            }
            s.done = true;
        }
    });

    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    sessions().insert(
        id,
        Session {
            state,
            child: child_slot,
        },
    );
    id
}

/// Snapshot of a session: `{"text": <cumulative>, "done": bool, "error"?,
/// "truncated"?}`. `truncated` marks an answer the `num_predict` cap cut off.
/// A finished session is removed after the poll that observes `done`.
pub fn poll(id: u64) -> Option<String> {
    let remove;
    let snapshot = {
        let map = sessions();
        let session = map.get(&id)?;
        let state = session.state.lock().unwrap_or_else(|e| e.into_inner());
        remove = state.done;
        let mut out = json!({ "text": state.text, "done": state.done });
        if let Some(error) = &state.error {
            out["error"] = Value::String(error.clone());
        }
        if state.truncated {
            out["truncated"] = Value::Bool(true);
        }
        out.to_string()
    };
    if remove {
        cancel(id);
    }
    Some(snapshot)
}

/// Kills the child (aborting Ollama generation) and drops the session.
pub fn cancel(id: u64) {
    let session = sessions().remove(&id);
    if let Some(session) = session {
        reap(&session.child);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bad_input_and_unknown_ids() {
        assert_eq!(start("http://localhost:1", "m", "not json", ""), 0);
        assert!(poll(999_999).is_none());
        cancel(999_999); // must not panic
    }

    #[test]
    fn a_blank_host_names_the_missing_setting() {
        // Otherwise the URL becomes "/api/chat", curl returns nothing, and the
        // panel blames the model for an empty Settings field.
        let id = start("  ", "m", "[]", "{}");
        assert_ne!(id, 0);
        let snapshot = poll(id).unwrap();
        assert!(snapshot.contains("\"done\":true"), "{snapshot}");
        assert!(
            snapshot.contains("no Ollama host is configured"),
            "{snapshot}"
        );
        // The poll that observed done removed it, like any other session.
        assert!(poll(id).is_none());
    }

    #[test]
    fn failed_request_reaches_done_and_is_removed() {
        // Connection refused: curl exits fast, the reader thread reaps it and
        // marks the session done with an error.
        let id = start("http://127.0.0.1:1", "m", "[]", "");
        assert_ne!(id, 0);
        let mut last = String::new();
        // Outlasts CONNECT_TIMEOUT_SECS: a platform that drops the SYN rather
        // than refusing it takes the whole connect budget to fail.
        let attempts = (CONNECT_TIMEOUT_SECS as usize + 5) * 20;
        for _ in 0..attempts {
            let Some(snapshot) = poll(id) else { break };
            last = snapshot;
            if last.contains("\"done\":true") {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(last.contains("\"done\":true"), "never finished: {last}");
        assert!(last.contains("error"));
        // The poll that observed done removed the session.
        assert!(poll(id).is_none());
    }
}
