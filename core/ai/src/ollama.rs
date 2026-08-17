//! Blocking Ollama client on the system `curl`, mirroring the transport
//! doctrine of `core/answers/src/http.rs`: no async runtime, no HTTP crate.
//! POST-with-JSON only; streaming (P5) reads a curl child's stdout lines.

use std::io::Write;
use std::process::{Command, Stdio};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// POSTs `body` as JSON to `url`, returning the raw response body, or None on
/// any failure (spawn error, non-2xx, non-UTF-8). `timeout_secs` caps it.
pub fn post_json(url: &str, body: &str, timeout_secs: u32) -> Option<String> {
    let mut command = Command::new("curl");
    command
        .arg("-sS")
        .arg("--fail")
        .arg("--connect-timeout")
        .arg(crate::chat::CONNECT_TIMEOUT_SECS.to_string())
        .arg("--max-time")
        .arg(timeout_secs.to_string())
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
    #[cfg(target_os = "windows")]
    command.creation_flags(CREATE_NO_WINDOW);

    let mut child = command.spawn().ok()?;
    let write_ok = child
        .stdin
        .take()
        .map(|mut stdin| stdin.write_all(body.as_bytes()).is_ok())
        .unwrap_or(false);
    if !write_ok {
        // Kill + wait so a failed request never leaves a zombie curl.
        let _ = child.kill();
        let _ = child.wait();
        return None;
    }
    let output = child.wait_with_output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

/// One parsed NDJSON line of a `/api/chat` response.
pub struct StreamLine {
    pub delta: String,
    /// The line carried hidden reasoning instead of answer text. A model that
    /// ignores `think: false` (or a shell linked against a core that never
    /// sent it) spends the whole budget here and returns no content, which
    /// otherwise reads as an unexplained empty answer.
    pub thinking: bool,
    pub done: bool,
    /// The server's own error message: Ollama emits `{"error":"..."}` both
    /// in-stream (e.g. model OOM mid-generation) and as the body of a non-2xx
    /// response ("model 'x' not found").
    pub error: Option<String>,
    /// Done because the `num_predict` cap was hit (`done_reason: "length"`),
    /// so the answer is cut off, not complete.
    pub truncated: bool,
}

/// One NDJSON line of a streamed `/api/chat` response, reduced to the delta
/// text, the done flag, and any server error. None for non-JSON lines.
pub fn parse_stream_line(line: &str) -> Option<StreamLine> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    let root: serde_json::Value = serde_json::from_str(trimmed).ok()?;
    let error = root
        .get("error")
        .and_then(|e| e.as_str())
        .filter(|e| !e.is_empty())
        .map(String::from);
    let done = root.get("done").and_then(|d| d.as_bool()).unwrap_or(false);
    let truncated = root.get("done_reason").and_then(|d| d.as_str()) == Some("length");
    let field = |name: &str| -> &str {
        root.get("message")
            .and_then(|m| m.get(name))
            .and_then(|c| c.as_str())
            .unwrap_or("")
    };
    let delta = field("content").to_string();
    let thinking = !field("thinking").is_empty();
    Some(StreamLine {
        delta,
        thinking,
        done,
        error,
        truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::parse_stream_line;

    #[test]
    fn stream_line_parses_delta_and_done() {
        let line = parse_stream_line(r#"{"message":{"content":"Hel"},"done":false}"#).unwrap();
        assert_eq!(line.delta, "Hel");
        assert!(!line.done);
        assert!(line.error.is_none());
        let line = parse_stream_line(r#"{"message":{"content":""},"done":true}"#).unwrap();
        assert!(line.done);
        assert!(parse_stream_line("   ").is_none());
    }

    #[test]
    fn stream_line_tells_hidden_reasoning_from_answer_text() {
        // What a reasoning model emits when `think: false` did not reach it:
        // content stays empty for the whole budget.
        let line =
            parse_stream_line(r#"{"message":{"content":"","thinking":"Let me"},"done":false}"#)
                .unwrap();
        assert!(line.thinking);
        assert!(line.delta.is_empty());
        let line = parse_stream_line(r#"{"message":{"content":"Xin"},"done":false}"#).unwrap();
        assert!(!line.thinking);
    }

    #[test]
    fn stream_line_surfaces_server_errors() {
        let line = parse_stream_line(r#"{"error":"model 'x' not found"}"#).unwrap();
        assert_eq!(line.error.as_deref(), Some("model 'x' not found"));
        assert!(!line.done);
    }

    #[test]
    fn stream_line_flags_length_truncation() {
        let line =
            parse_stream_line(r#"{"message":{"content":""},"done":true,"done_reason":"length"}"#)
                .unwrap();
        assert!(line.done);
        assert!(line.truncated);
        let line =
            parse_stream_line(r#"{"message":{"content":""},"done":true,"done_reason":"stop"}"#)
                .unwrap();
        assert!(!line.truncated);
    }
}
