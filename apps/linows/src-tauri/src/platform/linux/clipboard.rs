//! Linux file-clipboard support (paste-into-file-manager).
//!
//! Encodes paths as `file://` URIs in the GNOME "copy" payload format and
//! pushes them to the clipboard via `wl-copy` (Wayland) or `xclip` (X11),
//! whichever is available. Neither is a hard runtime dependency.

use std::io::Write;
use std::process::{Command, Stdio};

pub(crate) fn copy_files(paths: &[String]) -> Result<(), String> {
    let uris: Vec<String> = paths
        .iter()
        .map(|p| {
            let encoded: String = p
                .bytes()
                .map(|b| match b {
                    b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                        (b as char).to_string()
                    }
                    _ => format!("%{b:02X}"),
                })
                .collect();
            format!("file://{encoded}")
        })
        .collect();
    let payload = format!("copy\n{}", uris.join("\n"));
    let mime = "x-special/gnome-copied-files";

    // Try wl-copy (Wayland) first, then xclip (X11) — no hard dependency on either.
    let wl_result = Command::new("wl-copy")
        .args(["-t", mime])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .and_then(|mut child| {
            if let Some(ref mut stdin) = child.stdin {
                stdin.write_all(payload.as_bytes())?;
            }
            child.wait()
        });

    if wl_result.is_ok() {
        return Ok(());
    }

    let xclip_result = Command::new("xclip")
        .args(["-selection", "clipboard", "-t", mime])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .and_then(|mut child| {
            if let Some(ref mut stdin) = child.stdin {
                stdin.write_all(payload.as_bytes())?;
            }
            child.wait()
        });

    xclip_result
        .map(|_| ())
        .map_err(|e| format!("Failed to copy files: {e}. Install xclip or wl-clipboard."))
}
