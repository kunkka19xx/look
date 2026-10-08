//! The app's version is the one `tauri.conf.json` carries, the file the
//! release tag is checked against, until the Tauri shell retires and it
//! moves to this crate's manifest.

use std::fs;
use std::path::Path;

const TAURI_CONF: &str = "../src-tauri/tauri.conf.json";

fn main() {
    println!("cargo:rerun-if-changed={TAURI_CONF}");
    let version = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(TAURI_CONF))
        .ok()
        .and_then(|text| conf_version(&text))
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());
    println!("cargo:rustc-env=LOOK_VERSION={version}");
}

/// The `"version": "x.y.z"` line, read without a JSON crate in the build.
fn conf_version(text: &str) -> Option<String> {
    let at = text.find("\"version\"")?;
    let rest = &text[at + "\"version\"".len()..];
    let open = rest.find('"')?;
    let rest = &rest[open + 1..];
    let close = rest.find('"')?;
    Some(rest[..close].to_string())
}
