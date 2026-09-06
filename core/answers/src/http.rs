//! Tiny blocking HTTP-GET-JSON helper built on the system `curl`, shared by all
//! sources. Using `curl` (rather than `reqwest`) keeps this crate free of an
//! async runtime and matches the existing `translate_api` transport, including
//! the Windows console-suppression flag.

use std::process::Command;

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

/// A completed GET. The status is carried alongside the body because a throttled
/// host answers 429 with an HTML apology page, which a caller must not mistake
/// for a malformed reply from a healthy service.
pub struct Response {
    pub status: u16,
    pub body: String,
}

impl Response {
    pub fn is_success(&self) -> bool {
        (HTTP_OK..HTTP_MULTIPLE_CHOICES).contains(&self.status)
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

    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }
    split_status(&String::from_utf8(output.stdout).ok()?)
}

/// Splits curl's stdout, which is the body followed by `WRITE_OUT_STATUS`, at
/// the last newline. The body keeps its own newlines, trailing one included.
fn split_status(stdout: &str) -> Option<Response> {
    let (body, status) = stdout.rsplit_once('\n')?;
    Some(Response {
        status: status.trim().parse().ok()?,
        body: body.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_body_from_written_out_status() {
        let ok = split_status("{\"a\":1}\n200").expect("parsed");
        assert_eq!(ok.body, "{\"a\":1}");
        assert!(ok.is_success());
        assert!(!ok.is_rate_limited());

        let multiline = split_status("line1\nline2\n\n429").expect("parsed");
        assert_eq!(multiline.body, "line1\nline2\n");
        assert!(!multiline.is_success());
        assert!(multiline.is_rate_limited());

        assert!(split_status("no status written out").is_none());
        assert!(split_status("body\nnot-a-status").is_none());
    }
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
