//! Tiny blocking HTTP-GET-JSON helper built on the system `curl`, shared by all
//! sources. Using `curl` (rather than `reqwest`) keeps this crate free of an
//! async runtime and matches the existing `translate_api` transport, including
//! the Windows console-suppression flag.

use std::io::Read;
use std::process::{Command, Stdio};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
// Suppress the console window when curl spawns from a GUI shell.
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

const USER_AGENT: &str = "Look-Launcher";
/// Appended after the body so one stdout carries both; split on the last newline.
const WRITE_OUT_STATUS: &str = "\n%{http_code}";
const HTTP_OK: u16 = 200;
const HTTP_MULTIPLE_CHOICES: u16 = 300;
const HTTP_TOO_MANY_REQUESTS: u16 = 429;
const MAX_BODY_BYTES: u64 = 20 * 1024 * 1024;
/// What `WRITE_OUT_STATUS` adds after the body: a newline and three digits.
const STATUS_SUFFIX_BYTES: u64 = 4;

/// A completed GET. The status is carried alongside the body because a throttled
/// host answers 429 with an HTML apology page, which a caller must not mistake
/// for a malformed reply from a healthy service.
pub struct Response {
    pub status: u16,
    pub body: String,
}

impl Response {
    pub fn is_success(&self) -> bool {
        is_success(self.status)
    }

    pub fn is_rate_limited(&self) -> bool {
        self.status == HTTP_TOO_MANY_REQUESTS
    }
}

/// GETs `url` and parses the body as JSON, or `None` on any failure (spawn
/// error, non-zero exit, non-2xx, non-UTF-8, non-JSON). `timeout_secs` caps the
/// request.
pub fn get_json(url: &str, timeout_secs: u32) -> Option<serde_json::Value> {
    let response = get(url, timeout_secs, USER_AGENT, &[])?;
    if !response.is_success() {
        return None;
    }
    serde_json::from_str(&response.body).ok()
}

/// GETs `url` with a custom user agent and extra `-H` headers, returning the
/// status and raw body, or `None` on spawn error / non-zero exit / non-UTF-8.
/// A non-2xx response IS returned, so callers can tell "throttled" from
/// "unreadable". Lets callers that need a specific UA or `Accept-Language`
/// (e.g. translation) share one curl path.
pub fn get(url: &str, timeout_secs: u32, user_agent: &str, headers: &[&str]) -> Option<Response> {
    let stdout = curl(url, timeout_secs, user_agent, headers)?;
    let (body, status) = split_status(&stdout)?;
    Some(Response {
        status,
        body: String::from_utf8(body.to_vec()).ok()?,
    })
}

/// GETs `url` and returns the body of a 2xx reply as is, for binary payloads
/// such as an answer's thumbnail.
pub fn get_bytes(url: &str, timeout_secs: u32) -> Option<Vec<u8>> {
    let stdout = curl(url, timeout_secs, USER_AGENT, &[])?;
    let (body, status) = split_status(&stdout)?;
    is_success(status).then(|| body.to_vec())
}

fn is_success(status: u16) -> bool {
    (HTTP_OK..HTTP_MULTIPLE_CHOICES).contains(&status)
}

/// curl's stdout: the body followed by `WRITE_OUT_STATUS`.
fn curl(url: &str, timeout_secs: u32, user_agent: &str, headers: &[&str]) -> Option<Vec<u8>> {
    let mut command = Command::new("curl");
    // The AppImage points LD_LIBRARY_PATH at bundled Ubuntu libs; the system
    // curl resolves libcurl's deps against them and dies with a symbol
    // lookup error on distros with newer libs.
    #[cfg(target_os = "linux")]
    command.env_remove("LD_LIBRARY_PATH");
    command.args([
        "-s",
        "-m",
        &timeout_secs.to_string(),
        "--user-agent",
        user_agent,
        "--tlsv1.2",
        "-w",
        WRITE_OUT_STATUS,
    ]);
    for header in headers {
        command.args(["-H", header]);
    }
    command.arg(url);
    #[cfg(target_os = "windows")]
    command.creation_flags(CREATE_NO_WINDOW);

    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdout = Vec::new();
    let read = (child.stdout.take()?)
        .take(MAX_BODY_BYTES + STATUS_SUFFIX_BYTES + 1)
        .read_to_end(&mut stdout);
    if read.is_err() || stdout.len() as u64 > MAX_BODY_BYTES + STATUS_SUFFIX_BYTES {
        let _ = child.kill();
        let _ = child.wait();
        return None;
    }
    child.wait().ok()?.success().then_some(stdout)
}

/// Splits curl's stdout at the last newline into the body and the status.
/// The body keeps its own newlines, trailing one included.
fn split_status(stdout: &[u8]) -> Option<(&[u8], u16)> {
    let at = stdout.iter().rposition(|&b| b == b'\n')?;
    let status = std::str::from_utf8(&stdout[at + 1..])
        .ok()?
        .trim()
        .parse()
        .ok()?;
    Some((&stdout[..at], status))
}

/// Percent-encodes `value` for use in a URL query component (RFC 3986
/// unreserved set passes through; everything else is `%XX`).
pub fn encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for &b in value.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_body_from_written_out_status() {
        assert_eq!(
            split_status(b"{\"a\":1}\n200"),
            Some((&b"{\"a\":1}"[..], 200))
        );
        assert!(is_success(200));

        let (body, status) = split_status(b"line1\nline2\n\n429").expect("parsed");
        assert_eq!(body, b"line1\nline2\n");
        assert!(!is_success(status));
        assert_eq!(status, HTTP_TOO_MANY_REQUESTS);

        assert!(split_status(b"no status written out").is_none());
        assert!(split_status(b"body\nnot-a-status").is_none());
    }
}
