pub mod autostart;
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
