//! Linux window-manager / compositor detection.

use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// The compositor's IPC endpoint. A shell kept across a relogin (tmux) still
/// carries the old session's variables, so each is checked by a connect, and
/// a scan of the runtime dir stands in when none answers.
pub enum Ipc {
    Sway(PathBuf),
    Niri(PathBuf),
    /// The instance signature, the directory under `hypr/`.
    Hyprland(String),
}

const SWAY_SOCKET_PREFIX: &str = "sway-ipc.";
const SOCKET_SUFFIX: &str = ".sock";
const HYPR_DIR: &str = "hypr";
const HYPR_SOCKET: &str = ".socket.sock";

pub fn ipc() -> Option<&'static Ipc> {
    static IPC: OnceLock<Option<Ipc>> = OnceLock::new();
    IPC.get_or_init(find_ipc).as_ref()
}

fn alive(path: &Path) -> bool {
    UnixStream::connect(path).is_ok()
}

fn runtime_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from)
}

fn hypr_socket(runtime: &Path, signature: &str) -> PathBuf {
    runtime.join(HYPR_DIR).join(signature).join(HYPR_SOCKET)
}

fn find_ipc() -> Option<Ipc> {
    let runtime = runtime_dir();
    if let Some(path) = std::env::var_os("SWAYSOCK")
        .map(PathBuf::from)
        .filter(|p| alive(p))
    {
        return Some(Ipc::Sway(path));
    }
    if let Some(path) = std::env::var_os("NIRI_SOCKET")
        .map(PathBuf::from)
        .filter(|p| alive(p))
    {
        return Some(Ipc::Niri(path));
    }
    if let (Ok(signature), Some(runtime)) = (std::env::var("HYPRLAND_INSTANCE_SIGNATURE"), &runtime)
        && alive(&hypr_socket(runtime, &signature))
    {
        return Some(Ipc::Hyprland(signature));
    }
    scan_ipc(&runtime?)
}

fn scan_ipc(runtime: &Path) -> Option<Ipc> {
    let display = std::env::var("WAYLAND_DISPLAY").unwrap_or_default();
    let names = |dir: &Path| {
        std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| entry.file_name().into_string().ok())
    };
    for name in names(runtime) {
        let path = runtime.join(&name);
        if is_sway_socket(&name) && alive(&path) {
            return Some(Ipc::Sway(path));
        }
        if is_niri_socket(&name, &display) && alive(&path) {
            return Some(Ipc::Niri(path));
        }
    }
    names(&runtime.join(HYPR_DIR))
        .find(|signature| alive(&hypr_socket(runtime, signature)))
        .map(Ipc::Hyprland)
}

/// `sway-ipc.<uid>.<pid>.sock`
fn is_sway_socket(name: &str) -> bool {
    name.starts_with(SWAY_SOCKET_PREFIX) && name.ends_with(SOCKET_SUFFIX)
}

/// `niri.<display>.<pid>.sock`, on this display.
fn is_niri_socket(name: &str, display: &str) -> bool {
    name.strip_prefix("niri.")
        .and_then(|rest| rest.strip_prefix(display))
        .and_then(|rest| rest.strip_prefix('.'))
        .and_then(|rest| rest.strip_suffix(SOCKET_SUFFIX))
        .is_some_and(|pid| !pid.is_empty() && pid.bytes().all(|b| b.is_ascii_digit()))
}

pub fn is_sway() -> bool {
    matches!(ipc(), Some(Ipc::Sway(_)))
}

pub fn is_niri() -> bool {
    matches!(ipc(), Some(Ipc::Niri(_)))
}

pub fn is_hyprland() -> bool {
    matches!(ipc(), Some(Ipc::Hyprland(_)))
}

pub fn niri_socket() -> Option<&'static Path> {
    match ipc()? {
        Ipc::Niri(path) => Some(path),
        _ => None,
    }
}

/// `swaymsg` on the live socket, which may not be the one the environment names.
pub fn swaymsg() -> Command {
    let mut cmd = super::host_command("swaymsg");
    if let Some(Ipc::Sway(path)) = ipc() {
        cmd.arg("-s").arg(path);
    }
    cmd
}

/// `hyprctl` on the live instance; it finds its socket by this variable.
pub fn hyprctl() -> Command {
    let mut cmd = super::host_command("hyprctl");
    if let Some(Ipc::Hyprland(signature)) = ipc() {
        cmd.env("HYPRLAND_INSTANCE_SIGNATURE", signature);
    }
    cmd
}

/// swayfx, which frosts layer surfaces through its own `layer_effects`
/// rather than a blur protocol. Its version reply carries the sway it is
/// built on, which plain sway's does not. Asked once.
pub fn is_swayfx() -> bool {
    static SWAYFX: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *SWAYFX.get_or_init(|| {
        is_sway()
            && swaymsg()
                .args(["-t", "get_version"])
                .output()
                .ok()
                .filter(|out| out.status.success())
                .is_some_and(|out| {
                    String::from_utf8_lossy(&out.stdout).contains("sway_original_version")
                })
    })
}

/// Whether the compositor offers `zwlr_layer_shell_v1`; Mutter does not.
/// `false` off Wayland. Asked once.
pub fn has_layer_shell() -> bool {
    static LAYER_SHELL: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *LAYER_SHELL.get_or_init(|| has_wayland_global("zwlr_layer_shell_v1"))
}

fn has_wayland_global(interface: &str) -> bool {
    use wayland_client::{
        Connection, Dispatch, QueueHandle,
        globals::{GlobalListContents, registry_queue_init},
        protocol::wl_registry::{self, WlRegistry},
    };

    struct Globals;
    impl Dispatch<WlRegistry, GlobalListContents> for Globals {
        fn event(
            _: &mut Self,
            _: &WlRegistry,
            _: wl_registry::Event,
            _: &GlobalListContents,
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
        }
    }

    let Ok(conn) = Connection::connect_to_env() else {
        return false;
    };
    registry_queue_init::<Globals>(&conn).is_ok_and(|(globals, _)| {
        globals
            .contents()
            .with_list(|list| list.iter().any(|g| g.interface == interface))
    })
}

/// Returns true when the session is KDE Plasma (any XDG_CURRENT_DESKTOP
/// segment is "KDE").
pub fn is_kde() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .split(':')
        .any(|s| s.trim().eq_ignore_ascii_case("KDE"))
}

pub fn detect_compositor() -> Option<String> {
    if std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok() {
        return Some("hyprland".into());
    }
    if is_sway() {
        return Some("sway".into());
    }
    if std::env::var("I3SOCK").is_ok() {
        return Some("i3".into());
    }
    if is_niri() {
        return Some("niri".into());
    }
    // XDG_CURRENT_DESKTOP can be colon-separated ("ubuntu:GNOME", "pop:GNOME").
    // Prefer a recognised desktop name over distro prefixes.
    const KNOWN: &[&str] = &[
        "gnome", "kde", "cinnamon", "xfce", "lxqt", "mate", "budgie", "deepin", "pantheon",
        "cosmic",
    ];
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    for seg in desktop.split(':') {
        let s = seg.trim().to_ascii_lowercase();
        if KNOWN.iter().any(|&k| k == s) {
            return Some(s);
        }
    }
    // Fallback: first non-empty segment.
    desktop.split(':').find_map(|s| {
        let t = s.trim();
        (!t.is_empty()).then(|| t.to_ascii_lowercase())
    })
}

/// Returns true for tiling WMs (i3, sway, Hyprland, niri) where `set_position`
/// on a hidden/unmapped window is ignored - the WM applies its own placement
/// on map.
pub fn is_tiling_wm() -> bool {
    std::env::var("I3SOCK").is_ok()
        || is_sway()
        || std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok()
        || is_niri()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socket_names() {
        assert!(is_sway_socket("sway-ipc.1000.2892.sock"));
        assert!(!is_sway_socket("sway-ipc.1000.2892.sock.lock"));
        assert!(is_niri_socket("niri.wayland-1.69433.sock", "wayland-1"));
        assert!(!is_niri_socket("niri.wayland-1.69433.sock", "wayland-2"));
        assert!(!is_niri_socket("niri.wayland-1.sock", "wayland-1"));
    }
}
