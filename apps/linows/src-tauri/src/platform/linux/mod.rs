pub mod autostart;
pub mod blur;
pub mod blur_wayland;
pub mod clipboard;
pub mod dbus;
pub mod fonts;
pub mod gnome_ext;
pub mod gpu;
pub mod icons;
pub mod kde_focus;
pub mod process;
pub mod sysinfo;
pub mod transparency;
pub mod version;
pub mod wayland_shortcut;
pub mod window_focus;
pub mod wlr_focus;
pub mod wm;

/// `Command` for a system binary with `LD_LIBRARY_PATH` scrubbed. The
/// AppImage runtime points that variable at the bundled Ubuntu libs; host
/// binaries resolving against them die with symbol lookup errors on distros
/// with newer system libraries. Use for every host-tool spawn.
pub fn host_command(program: &str) -> std::process::Command {
    let mut cmd = std::process::Command::new(program);
    cmd.env_remove("LD_LIBRARY_PATH");
    cmd
}

/// Owner, group and other execute bits.
const EXEC_BITS: u32 = 0o111;

/// Where `program` resolves on the host `PATH`, if anywhere. Use to pick
/// between interchangeable host tools before handing one to something else
/// (a compositor keybinding) to run later.
pub fn host_binary_path(program: &str) -> Option<std::path::PathBuf> {
    use std::os::unix::fs::PermissionsExt;

    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(program))
        .find(|candidate| {
            // Empty PATH entries mean the working directory, which resolves
            // to something else entirely from whatever runs the result later.
            candidate.is_absolute()
                && std::fs::metadata(candidate)
                    .map(|m| m.is_file() && m.permissions().mode() & EXEC_BITS != 0)
                    .unwrap_or(false)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_an_executable_on_path() {
        assert!(host_binary_path("sh").is_some_and(|p| p.ends_with("sh")));
    }

    #[test]
    fn missing_binary_resolves_to_none() {
        assert!(host_binary_path("look-nonexistent-binary").is_none());
    }
}
