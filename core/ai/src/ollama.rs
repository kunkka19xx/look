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
    child.stdin.take()?.write_all(body.as_bytes()).ok()?;
    let output = child.wait_with_output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

/// One NDJSON line of a streamed `/api/chat` response, reduced to the delta
/// text and the done flag. None for lines with no message payload.
pub fn parse_stream_line(line: &str) -> Option<(String, bool)> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    let root: serde_json::Value = serde_json::from_str(trimmed).ok()?;
    let done = root.get("done").and_then(|d| d.as_bool()).unwrap_or(false);
    let delta = root
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();
    Some((delta, done))
}

#[cfg(test)]
mod tests {
    use super::parse_stream_line;

    #[test]
    fn stream_line_parses_delta_and_done() {
        let (delta, done) = parse_stream_line(r#"{"message":{"content":"Hel"},"done":false}"#).unwrap();
        assert_eq!(delta, "Hel");
        assert!(!done);
        let (_, done) = parse_stream_line(r#"{"message":{"content":""},"done":true}"#).unwrap();
        assert!(done);
        assert!(parse_stream_line("   ").is_none());
    }
}
