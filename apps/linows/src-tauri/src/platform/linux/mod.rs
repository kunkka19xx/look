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
pub mod layer_shell;
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

/// How long the one-time `systemd-run --user` probe may run before the wrapper
/// counts as unavailable. A healthy per-user manager answers in milliseconds; a
/// wedged one blocks on its own D-Bus call for tens of seconds, which no launch
/// should wait out.
const SESSION_PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);
/// How often to look in on the probe while waiting for it.
const SESSION_PROBE_POLL: std::time::Duration = std::time::Duration::from_millis(20);

static SYSTEMD_RUN: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

/// Exercise the actual `--user` path (a no-op transient unit) - checking only
/// `systemd-run --version` succeeds on systems that have the binary but no
/// usable per-user manager (containers, minimal installs), and would route every
/// launch through a wrapper that then fails.
fn probe_systemd_run() -> bool {
    let Ok(mut child) = host_command("systemd-run")
        .args(["--user", "--quiet", "--wait", "--collect", "--", "true"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    else {
        return false;
    };

    let deadline = std::time::Instant::now() + SESSION_PROBE_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Err(_) => return false,
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return false;
                }
                std::thread::sleep(SESSION_PROBE_POLL);
            }
        }
    }
}

/// Run the probe off the request path, so the first launch of a session pays
/// nothing for it. Called once at startup.
pub fn prime_user_session() {
    std::thread::spawn(|| {
        SYSTEMD_RUN.get_or_init(probe_systemd_run);
    });
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
///
/// This form does not wait: the caller spawns and walks away, and a waiting
/// wrapper would sit there for as long as the app it started.
pub fn user_session_command(prog: &str) -> std::process::Command {
    session_command(prog, false)
}

/// [`user_session_command`] for a caller that reads the result. `systemd-run`
/// otherwise returns the moment the transient unit starts, so its status
/// describes the submission rather than `prog`, and every fallback behind that
/// check becomes unreachable. `--wait` returns as soon as the unit's main
/// process exits, which `KillMode=process` keeps separate from the app's own
/// lifetime.
pub fn user_session_command_for_status(prog: &str) -> std::process::Command {
    session_command(prog, true)
}

fn session_command(prog: &str, for_status: bool) -> std::process::Command {
    if !*SYSTEMD_RUN.get_or_init(probe_systemd_run) {
        return host_command(prog);
    }

    let mut cmd = host_command("systemd-run");
    cmd.args([
        "--user",
        "--collect",
        "--quiet",
        "--property=KillMode=process",
    ]);
    if for_status {
        cmd.arg("--wait");
    }
    cmd.args(["--", prog]);
    cmd
}

/// `file://` with every byte outside the RFC 3986 unreserved set percent-encoded.
/// `/` stays literal: it separates path segments rather than belonging to one.
///
/// A relative path is resolved against the working directory first, since
/// `file://` reads the first segment of one as the authority. Rows from a
/// `.look` source keep whatever path the file declared, so that case does reach
/// here, and the working directory is what produced the row in the first place.
///
/// Wanted by anything handing a path to a desktop service - the clipboard's
/// URI payloads and the file manager's `ShowItems` - which is why it does not
/// live in either.
pub fn file_uri(path: &str) -> String {
    let resolved = std::path::absolute(path).unwrap_or_else(|_| std::path::PathBuf::from(path));

    let mut encoded = String::from("file://");
    for byte in resolved.as_os_str().as_encoded_bytes() {
        match byte {
            b'/' | b'-' | b'.' | b'_' | b'~' => encoded.push(*byte as char),
            _ if byte.is_ascii_alphanumeric() => encoded.push(*byte as char),
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
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

    /// A path with a space is what breaks a naive `file://` + path.
    #[test]
    fn a_uri_encodes_everything_outside_the_unreserved_set() {
        assert_eq!(
            file_uri("/tmp/my project/a b.txt"),
            "file:///tmp/my%20project/a%20b.txt"
        );
        assert_eq!(file_uri("/tmp/a~b-c_d.txt"), "file:///tmp/a~b-c_d.txt");
    }

    /// `file://notes/a.txt` names a host called `notes`, not a local file.
    #[test]
    fn a_relative_path_is_resolved_before_it_is_encoded() {
        let cwd = std::env::current_dir().expect("a working directory");
        let expected = file_uri(&cwd.join("notes/a.txt").to_string_lossy());

        let uri = file_uri("notes/a.txt");
        assert_eq!(uri, expected);
        assert!(uri.starts_with("file:///"), "{uri} has an authority");
    }

    #[test]
    fn finds_an_executable_on_path() {
        assert!(host_binary_path("sh").is_some_and(|p| p.ends_with("sh")));
    }

    #[test]
    fn missing_binary_resolves_to_none() {
        assert!(host_binary_path("look-nonexistent-binary").is_none());
    }
}
