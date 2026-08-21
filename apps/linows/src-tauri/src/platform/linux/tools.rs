//! Starting a tool the user named in their config, and revealing a path in
//! whatever file manager the desktop provides.
//!
//! Core composes what to run but refuses to guess *where a name lives*
//! (`look_tools::Launch::Application`): only native code can turn `zed` or
//! `nautilus` into something the machine can start. On Linux that is `$PATH`
//! first, since a GUI tool that ships a CLI takes the path as an argument, and
//! a `.desktop` entry second, for the ones that ship no CLI at all.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use super::{host_binary_path, host_command, user_session_command};

/// Every `.desktop` entry naming this tool would be under one of these.
const DESKTOP_SUBDIR: &str = "applications";
const DESKTOP_SUFFIX: &str = ".desktop";

/// The file manager interface every desktop file manager implements, and the
/// only way to open a folder with one file already selected.
const FILE_MANAGER_NAME: &str = "org.freedesktop.FileManager1";
const FILE_MANAGER_PATH: &str = "/org/freedesktop/FileManager1";
/// Startup id, which we have none of; every implementation accepts an empty one.
const NO_STARTUP_ID: &str = "";

/// Start `tool` on `path`.
///
/// Errors name the tool rather than the mechanism: "could not find nautilus" is
/// something a user can act on, where a failed `gio launch` is not.
pub fn launch(tool: &str, path: &str) -> Result<(), String> {
    if let Some(program) = program_for(tool) {
        return spawn(user_session_command(&program).arg(path));
    }

    if let Some(entry) = desktop_entry_for(tool) {
        let id = entry
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();
        // gtk-launch before gio launch, for the same reason app launching
        // prefers it: gio goes through D-Bus activation, which can report
        // success without ever making a window.
        if spawn(user_session_command("gtk-launch").arg(&id).arg(path)).is_ok() {
            return Ok(());
        }
        return spawn(
            user_session_command("gio")
                .arg("launch")
                .arg(&entry)
                .arg(path),
        );
    }

    Err(format!(
        "no binary on PATH and no desktop entry named {tool}"
    ))
}

/// Bring `tool` forward once it has had time to draw.
///
/// Only for a tool started with [`launch`]: a GUI editor that was already
/// running opens the file in its existing window and never asks to be raised,
/// which reads as the key having done nothing. A terminal spawned through a
/// composed shell command makes a *new* window, which every compositor focuses
/// on its own, so that path deliberately does not come through here.
pub fn activate(tool: &str) {
    let name = window_name(tool);
    std::thread::sleep(std::time::Duration::from_millis(
        crate::commands::HANDLER_FOCUS_DELAY_MS,
    ));
    crate::commands::try_focus_window(&name);
}

/// Show `path` in the desktop's own file manager with the file selected.
///
/// `ShowItems` is what makes this a reveal rather than an open: `xdg-open` can
/// only be handed the containing folder, leaving the user to find the file in
/// it again. Falls back to exactly that when nothing owns the interface.
pub fn reveal(path: &str) -> Result<(), String> {
    if show_items(path) {
        return Ok(());
    }

    let target = Path::new(path);
    let folder = if target.is_dir() {
        target
    } else {
        target.parent().unwrap_or(target)
    };
    spawn(host_command("xdg-open").arg(folder))
}

fn show_items(path: &str) -> bool {
    let Ok(uri) = url_from_path(path) else {
        return false;
    };
    let Some(connection) = super::dbus::session() else {
        return false;
    };

    super::dbus::runtime().block_on(async {
        connection
            .call_method(
                Some(FILE_MANAGER_NAME),
                FILE_MANAGER_PATH,
                Some(FILE_MANAGER_NAME),
                "ShowItems",
                &(vec![uri], NO_STARTUP_ID),
            )
            .await
            .is_ok()
    })
}

/// `file://` with every byte outside the unreserved set percent-encoded, which
/// is what the interface takes. Fails for a relative path, which has no URI.
fn url_from_path(path: &str) -> Result<String, ()> {
    let absolute = Path::new(path);
    if !absolute.is_absolute() {
        return Err(());
    }

    let mut encoded = String::from("file://");
    for byte in path.as_bytes() {
        match byte {
            b'/' | b'-' | b'_' | b'.' | b'~' => encoded.push(*byte as char),
            _ if byte.is_ascii_alphanumeric() => encoded.push(*byte as char),
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    Ok(encoded)
}

/// The program to run for `tool`: a path the user spelled out, or a name on
/// `$PATH`.
fn program_for(tool: &str) -> Option<String> {
    let tool = tool.trim();
    if tool.is_empty() {
        return None;
    }
    if tool.contains('/') {
        return Path::new(tool).is_file().then(|| tool.to_string());
    }
    host_binary_path(tool).map(|_| tool.to_string())
}

/// The `.desktop` file declaring `tool`, matched on its id rather than its
/// `Name=`: a user naming a tool means the thing they would type, and an id is
/// the closest thing a desktop entry has to that.
fn desktop_entry_for(tool: &str) -> Option<PathBuf> {
    let wanted = tool.trim().to_ascii_lowercase();
    if wanted.is_empty() {
        return None;
    }
    let wanted = wanted
        .strip_suffix(DESKTOP_SUFFIX)
        .unwrap_or(&wanted)
        .to_string();

    for dir in desktop_dirs() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(id) = path
                .file_name()
                .and_then(|name| name.to_str())
                .and_then(|name| name.strip_suffix(DESKTOP_SUFFIX))
            else {
                continue;
            };
            let id = id.to_ascii_lowercase();
            // A reverse-DNS id (org.gnome.Nautilus) is still "nautilus" to the
            // user, so its last segment counts as the name too.
            if id == wanted || id.rsplit('.').next() == Some(wanted.as_str()) {
                return Some(path);
            }
        }
    }
    None
}

/// The XDG directories holding desktop entries, most specific first, so a
/// user's own override wins over the system copy.
fn desktop_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let mut push = |dir: PathBuf| {
        if !dirs.contains(&dir) {
            dirs.push(dir);
        }
    };

    if let Some(home) = dirs::data_local_dir() {
        push(home.join(DESKTOP_SUBDIR));
    }
    if let Ok(data_dirs) = std::env::var("XDG_DATA_DIRS") {
        for dir in data_dirs.split(':').filter(|dir| !dir.trim().is_empty()) {
            push(PathBuf::from(dir.trim()).join(DESKTOP_SUBDIR));
        }
    }
    dirs
}

/// What a tool's window is called to the compositor. GTK apps set their class
/// from the desktop id, so a tool named by its path or its reverse-DNS id has
/// to be reduced to the same short name the WM sees.
fn window_name(tool: &str) -> String {
    let name = tool.trim().rsplit('/').next().unwrap_or(tool);
    name.strip_suffix(DESKTOP_SUFFIX)
        .unwrap_or(name)
        .to_string()
}

fn spawn(command: &mut std::process::Command) -> Result<(), String> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("{}: {e}", command.get_program().to_string_lossy()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A path with a space is the case that breaks a naive `file://` + path.
    #[test]
    fn a_uri_encodes_everything_outside_the_unreserved_set() {
        assert_eq!(
            url_from_path("/tmp/my project/a b.txt"),
            Ok("file:///tmp/my%20project/a%20b.txt".to_string())
        );
        assert_eq!(
            url_from_path("/tmp/a~b-c_d.txt").unwrap(),
            "file:///tmp/a~b-c_d.txt"
        );
        assert_eq!(url_from_path("relative.txt"), Err(()));
    }

    /// The name a compositor knows, from every form a user might declare.
    #[test]
    fn a_declared_tool_reduces_to_the_name_its_window_carries() {
        for (declared, expected) in [
            ("nautilus", "nautilus"),
            ("/usr/bin/nautilus", "nautilus"),
            ("  ghostty  ", "ghostty"),
            ("org.gnome.Nautilus.desktop", "org.gnome.Nautilus"),
        ] {
            assert_eq!(window_name(declared), expected, "{declared:?}");
        }
    }

    /// A tool spelled as a path is taken at its word: naming a specific build
    /// must not silently fall through to another one on `$PATH`.
    #[test]
    fn a_path_that_is_not_there_resolves_to_nothing() {
        assert_eq!(program_for("/nonexistent/bin/zed"), None);
        assert_eq!(program_for("   "), None);
    }
}
