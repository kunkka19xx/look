//! Wayland host: a centred layer shell surface, commands over a Unix socket.

use std::os::unix::net::UnixListener;
use std::path::PathBuf;

use gpui::{
    App, Bounds, Pixels, Size, Window, WindowBackgroundAppearance, WindowKind,
    layer_shell::{Anchor, KeyboardInteractivity, Layer, LayerShellOptions},
    point, px,
};

use linows_backend::platform::linux::wm;

/// Blur behind comes from the compositor over its own protocol, see blur/wayland.rs.
pub const BACKGROUND: WindowBackgroundAppearance = WindowBackgroundAppearance::Transparent;

const SOCKET_NAME: &str = "look-gpui.sock";
/// A second instance beside the real one (tools/pace.sh) answers on its own
/// socket so the two do not trade commands.
const SOCKET_ENV: &str = "LOOK_CONTROL_SOCKET";

/// `printf toggle | socat - UNIX-CONNECT:$XDG_RUNTIME_DIR/look-gpui.sock`.
pub fn bind() -> std::io::Result<UnixListener> {
    let path = std::env::var_os(SOCKET_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("XDG_RUNTIME_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir)
                .join(SOCKET_NAME)
        });
    let _ = std::fs::remove_file(&path);
    UnixListener::bind(&path)
}

/// No anchor, so the compositor centres it, as the GTK port does.
pub fn bounds(size: Size<Pixels>, _: &App) -> Bounds<Pixels> {
    Bounds {
        origin: point(px(0.0), px(0.0)),
        size,
    }
}

pub fn decorate(_: &Window) {}

pub fn kind() -> WindowKind {
    WindowKind::LayerShell(LayerShellOptions {
        namespace: "lookapp".into(),
        layer: Layer::Overlay,
        anchor: Anchor::empty(),
        // sway hands an on-demand surface the keyboard on map and takes it
        // back in the same batch, so the window never counts as active, runs
        // at the inactive frame rate and gets no keys. niri keeps on-demand
        // focus, and it is the mode that lets other surfaces take the keyboard.
        keyboard_interactivity: if wm::is_sway() {
            KeyboardInteractivity::Exclusive
        } else {
            KeyboardInteractivity::OnDemand
        },
        ..Default::default()
    })
}
