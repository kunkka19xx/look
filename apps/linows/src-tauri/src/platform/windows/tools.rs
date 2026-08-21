//! The Windows half of `look_tools::Launch`: starting a named tool, and
//! revealing a path in Explorer.
//!
//! Only the cases that reach Windows are here. `Launch::Shell` never does:
//! composition emits POSIX shell text, so core refuses it off unix with
//! `Unavailable::UnsupportedPlatform` rather than hand `cmd` something it
//! would mangle.

use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};

const EXPLORER: &str = "explorer.exe";
const SELECT: &str = "/select,";

/// Start `tool` on `path`. `CreateProcessW` already searches `PATH`, so a name
/// the user could type is a name that resolves here.
pub fn launch(tool: &str, path: &str) -> Result<(), String> {
    Command::new(tool)
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(crate::consts::CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
        .map_err(|_| tool.to_string())
}

/// Raise the window the launched tool opened its file in.
pub fn activate(tool: &str) {
    std::thread::sleep(std::time::Duration::from_millis(HANDLER_FOCUS_DELAY_MS));
    let _ = super::window_focus::try_focus_existing(tool);
}

/// Long enough for the tool to have drawn the window we mean to raise.
const HANDLER_FOCUS_DELAY_MS: u64 = 150;

/// Explorer with the file selected, which is what reveal means.
pub fn reveal(path: &str) -> Result<(), String> {
    Command::new(EXPLORER)
        .arg(SELECT)
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}
