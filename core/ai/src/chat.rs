//! Streaming chat sessions on the curl transport (P5): `start` spawns a curl
//! child POSTing to Ollama's `/api/chat` with `stream:true` and a reader
//! thread accumulates cumulative text; shells `poll` for snapshots (an easy
//! fit for both FFI and Tauri) and `cancel` kills the child. No async runtime.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use serde_json::{Value, json};

use crate::ollama;

#[derive(Default)]
struct SessionState {
    text: String,
    done: bool,
    error: Option<String>,
}

struct Session {
    state: Arc<Mutex<SessionState>>,
    child: Arc<Mutex<Option<Child>>>,
}

static SESSIONS: OnceLock<Mutex<HashMap<u64, Session>>> = OnceLock::new();
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn sessions() -> &'static Mutex<HashMap<u64, Session>> {
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Starts a streamed chat; `messages_json` is the full message array
/// (system/context/history assembled by the shell). Returns a session id, or
/// 0 when the request could not even be spawned.
pub fn start(host: &str, model: &str, messages_json: &str) -> u64 {
    let Ok(messages) = serde_json::from_str::<Value>(messages_json) else {
        return 0;
    };
    let body = json!({
        "model": model,
        "messages": messages,
        "stream": true,
        "options": { "temperature": 0, "num_predict": 512 },
        "keep_alive": "30m",
    })
    .to_string();
    let url = format!("{}/api/chat", host.trim_end_matches('/'));

    let mut command = Command::new("curl");
    command
        .arg("-sS")
        .arg("--fail")
        .arg("--no-buffer")
        .arg("--max-time")
        .arg("300")
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

    let Ok(mut child) = command.spawn() else { return 0 };
    let Some(mut stdin) = child.stdin.take() else { return 0 };
    if stdin.write_all(body.as_bytes()).is_err() {
        return 0;
    }
    drop(stdin);
    let Some(stdout) = child.stdout.take() else { return 0 };

    let state = Arc::new(Mutex::new(SessionState::default()));
    let reader_state = Arc::clone(&state);
    std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        let mut saw_output = false;
        for line in reader.lines() {
            let Ok(line) = line else { break };
            let Some((delta, done)) = ollama::parse_stream_line(&line) else { continue };
            saw_output = true;
            if !delta.is_empty() {
                if let Ok(mut s) = reader_state.lock() {
                    s.text.push_str(&delta);
                }
            }
            if done {
                break;
            }
        }
        if let Ok(mut s) = reader_state.lock() {
            if !saw_output && s.text.is_empty() {
                s.error = Some("no response".into());
            }
            s.done = true;
        }
    });

    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut map) = sessions().lock() {
        map.insert(id, Session { state, child: Arc::new(Mutex::new(Some(child))) });
    }
    id
}

/// Snapshot of a session: `{"text": <cumulative>, "done": bool, "error"?}`.
/// A finished session is removed after the poll that observes `done`.
pub fn poll(id: u64) -> Option<String> {
    let remove;
    let snapshot = {
        let map = sessions().lock().ok()?;
        let session = map.get(&id)?;
        let state = session.state.lock().ok()?;
        remove = state.done;
        let mut out = json!({ "text": state.text, "done": state.done });
        if let Some(error) = &state.error {
            out["error"] = Value::String(error.clone());
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
    if let Ok(mut map) = sessions().lock() {
        if let Some(session) = map.remove(&id) {
            if let Ok(mut child) = session.child.lock() {
                if let Some(mut c) = child.take() {
                    let _ = c.kill();
                    let _ = c.wait();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bad_input_and_unknown_ids() {
        assert_eq!(start("http://localhost:1", "m", "not json"), 0);
        assert!(poll(999_999).is_none());
        cancel(999_999); // must not panic
    }
}
