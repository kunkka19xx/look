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

/// GETs `url` and parses the body as JSON, or `None` on any failure (spawn
/// error, non-zero exit, non-UTF-8, non-JSON). `timeout_secs` caps the request.
pub fn get_json(url: &str, timeout_secs: u32) -> Option<serde_json::Value> {
    let body = get(url, timeout_secs, USER_AGENT, &[])?;
    serde_json::from_str(&body).ok()
}

/// GETs `url` with a custom user agent and extra `-H` headers, returning the raw
/// body, or `None` on spawn error / non-2xx / non-UTF-8. Lets callers that need
/// a specific UA or `Accept-Language` (e.g. translation) share one curl path.
pub fn get(url: &str, timeout_secs: u32, user_agent: &str, headers: &[&str]) -> Option<String> {
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
    String::from_utf8(output.stdout).ok()
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
