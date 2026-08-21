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
pub mod niri;
pub mod process;
pub mod sysinfo;
pub mod tools;
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

/// Build a Command that runs `prog` inside the user's systemd session, so
/// the child sees the user manager's environment (current XAUTHORITY for
/// XWayland, DBUS_SESSION_BUS_ADDRESS, etc.) rather than whatever Look
/// inherited. Falls back to a plain spawn when systemd-run isn't available.
///
/// Without this, a dev-mode Look launched from a long-lived `nix develop` /
/// terminal shell carries the X11 cookie path it picked up at shell start -
/// stale after the next mutter/XWayland restart - so spawned GUI children
/// (firefox, etc.) fail with "cannot open display: :0" while gtk-launch
/// itself still reports success.
///
/// `KillMode=process` keeps the actual GUI app alive after gtk-launch / gio
/// launch (the unit's main process) exits.
pub fn user_session_command(prog: &str) -> std::process::Command {
    use std::sync::OnceLock;
    static SYSTEMD_RUN: OnceLock<bool> = OnceLock::new();
    let available = *SYSTEMD_RUN.get_or_init(|| {
        // Exercise the actual `--user` path (a no-op transient unit) - checking
        // only `systemd-run --version` succeeds on systems that have the binary
        // but no usable per-user manager (containers, minimal installs), and
        // would route every launch through a wrapper that then fails.
        host_command("systemd-run")
            .args(["--user", "--quiet", "--wait", "--collect", "--", "true"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    });
    if available {
        let mut cmd = host_command("systemd-run");
        cmd.args([
            "--user",
            "--collect",
            "--quiet",
            "--property=KillMode=process",
            "--",
            prog,
        ]);
        cmd
    } else {
        host_command(prog)
    }
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
