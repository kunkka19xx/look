//! The Windows half of `look_tools::Launch`: starting a named tool, and
//! revealing a path in Explorer.
//!
//! Only the cases that reach Windows are here. `Launch::Shell` never does:
//! composition emits POSIX shell text, so core hands Windows a `Launch::Argv`
//! instead of something `cmd` would mangle.

use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const EXPLORER: &str = "explorer.exe";
const SELECT: &str = "/select,";
/// What `PATHEXT` holds on a machine that has somehow lost it.
const DEFAULT_PATHEXT: &str = ".COM;.EXE;.BAT;.CMD";

/// Start `tool` on `path`.
pub fn launch(tool: &str, path: &str) -> Result<(), String> {
    Command::new(program_for(tool)?)
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(crate::consts::CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// The program to run for `tool`: a path the user spelled out, or a name on
/// `PATH`.
///
/// `CreateProcessW` searches `PATH` itself but only ever appends `.exe`, so a
/// shim is invisible to it: `code` on `PATH` is `code.cmd`. This is the
/// `PATHEXT` walk that makes a bare name work.
fn program_for(tool: &str) -> Result<PathBuf, String> {
    let tool = tool.trim();
    if tool.is_empty() {
        return Err("no tool declared".to_string());
    }

    // A path is taken at its word: naming a specific build must not fall
    // through to another one on PATH.
    if tool.contains(['\\', '/']) {
        return executable(Path::new(tool)).ok_or_else(|| format!("{tool} is not there"));
    }

    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path)
        .find_map(|dir| executable(&dir.join(tool)))
        .ok_or_else(|| format!("nothing named {tool} on PATH"))
}

/// `candidate` when it already spells an extension, else its first `PATHEXT`
/// spelling that exists.
///
/// An extensionless file is refused rather than taken: VS Code ships `bin\code`
/// (a POSIX shell script) next to `bin\code.cmd`, and handing that to
/// `CreateProcess` answers ERROR_BAD_EXE_FORMAT.
fn executable(candidate: &Path) -> Option<PathBuf> {
    if candidate.extension().is_some() && candidate.is_file() {
        return Some(candidate.to_path_buf());
    }

    let pathext = std::env::var("PATHEXT").unwrap_or_else(|_| DEFAULT_PATHEXT.to_string());
    pathext
        .split(';')
        .map(str::trim)
        .filter(|extension| !extension.is_empty())
        .find_map(|extension| {
            let mut spelled = candidate.as_os_str().to_os_string();
            spelled.push(extension);
            let path = PathBuf::from(spelled);
            path.is_file().then_some(path)
        })
}

/// Start `tool` with `args` in `cwd`, which is what opens a terminal that takes
/// no directory flag. No `CREATE_NO_WINDOW` unlike [`launch`]: the console
/// window is the point.
pub fn launch_argv(tool: &str, args: &[String], cwd: &str) -> Result<(), String> {
    Command::new(program_for(tool)?)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Raise the window the launched tool opened its file in. Detached: the caller
/// is a command the frontend awaits, and nothing there consumes the outcome.
pub fn activate(tool: &str) {
    let tool = tool.to_string();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(
            crate::consts::HANDLER_FOCUS_DELAY_MS,
        ));
        let _ = super::window_focus::try_focus_existing(&tool);
    });
}

/// Explorer with the file selected, which is what reveal means.
pub fn reveal(path: &str) -> Result<(), String> {
    Command::new(EXPLORER)
        .arg(SELECT)
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shim case: a tool whose only spelling on disk is `.cmd`.
    #[test]
    fn a_name_resolves_through_pathext() {
        let dir = std::env::temp_dir().join(format!("look-win-tools-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a scratch dir");
        let shim = dir.join("faketool.cmd");
        std::fs::write(&shim, "@echo off\n").expect("a shim");

        // Compared case-insensitively: the spelling comes from PATHEXT, which
        // is upper-case, and Windows does not care which one starts the file.
        let lowered = |path: Option<PathBuf>| path.map(|p| p.to_string_lossy().to_lowercase());
        let expected = lowered(Some(shim.clone()));

        assert_eq!(lowered(executable(&dir.join("faketool"))), expected);
        // A name that already spells its extension answers as itself.
        assert_eq!(executable(&shim), Some(shim));
        assert_eq!(executable(&dir.join("not-installed")), None);

        std::fs::remove_dir_all(&dir).expect("the scratch dir goes away");
    }

    /// The VS Code shape: `bin\code` is a POSIX shell script next to
    /// `bin\code.cmd`, and starting it answers ERROR_BAD_EXE_FORMAT.
    #[test]
    fn an_extensionless_sibling_never_wins_over_a_pathext_spelling() {
        let dir = std::env::temp_dir().join(format!("look-win-shim-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a scratch dir");
        std::fs::write(dir.join("faketool"), "#!/bin/sh\n").expect("a posix script");
        std::fs::write(dir.join("faketool.cmd"), "@echo off\n").expect("a shim");

        let resolved = executable(&dir.join("faketool")).expect("the shim resolves");
        assert!(
            resolved
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("cmd")),
            "took {resolved:?} rather than the .cmd"
        );

        // With only the extensionless file there, nothing is startable.
        std::fs::remove_file(dir.join("faketool.cmd")).expect("drop the shim");
        assert_eq!(executable(&dir.join("faketool")), None);

        std::fs::remove_dir_all(&dir).expect("the scratch dir goes away");
    }

    /// A tool spelled as a path is taken at its word: naming a specific build
    /// must not fall through to another one on PATH.
    #[test]
    fn a_path_that_is_not_there_is_not_guessed_at() {
        assert!(program_for("C:\\nowhere\\zed.exe").is_err());
        assert!(program_for("   ").is_err());
    }

    /// The whole point: a bare name, no extension, no directory.
    #[test]
    fn a_bare_name_resolves_off_path() {
        assert!(program_for("notepad").is_ok());
        assert!(program_for("definitely-not-a-real-tool").is_err());
    }
}
