//! Linux host: a centred layer shell surface on Wayland, a borderless floating
//! dialog on X11 (as the Tauri shell's window), commands over a Unix socket.

use std::io::Write;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use gpui::{
    App, Bounds, Pixels, Size, Window, WindowBackgroundAppearance, WindowKind,
    layer_shell::{Anchor, KeyboardInteractivity, Layer, LayerShellOptions},
    point, px,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

use linows_backend::platform::linux::{outputs, wm};

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
    // A launcher already answers: hand it the summon and let the caller exit.
    if let Ok(mut running) = UnixStream::connect(&path) {
        let _ = running.write_all(b"toggle");
        return Err(std::io::ErrorKind::AddrInUse.into());
    }
    let _ = std::fs::remove_file(&path);
    UnixListener::bind(&path)
}

/// gpui's own pick between its two Linux backends (`guess_compositor`).
fn is_x11() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_none_or(|d| d.is_empty())
}

static OPAQUE: AtomicBool = AtomicBool::new(false);

/// Bare X11 has nothing to blend a transparent pixel with, so the gaps
/// between the cards come out black. Asked again on every summon, since a
/// compositor may have started since.
pub fn probe_opaque() {
    OPAQUE.store(is_x11() && !x11_composited(), Ordering::Relaxed);
}

/// Whether the root must paint a solid backdrop, as of the last summon.
pub fn opaque() -> bool {
    OPAQUE.load(Ordering::Relaxed)
}

/// A compositing manager owns `_NET_WM_CM_S<screen>` (EWMH).
fn x11_composited() -> bool {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::ConnectionExt;

    let Ok((conn, screen)) = x11rb::connect(None) else {
        return false;
    };
    let name = format!("_NET_WM_CM_S{screen}");
    let owner = conn
        .intern_atom(false, name.as_bytes())
        .ok()
        .and_then(|c| c.reply().ok())
        .and_then(|atom| conn.get_selection_owner(atom.atom).ok())
        .and_then(|c| c.reply().ok());
    let _ = conn.flush();
    owner.is_some_and(|o| o.owner != x11rb::NONE)
}

/// Logical height of the screen the launcher opens on. gpui has no primary
/// display on Wayland and rounds fractional scales, so xdg-output comes first.
pub fn screen_height(cx: &App) -> Option<f32> {
    outputs::logical_height().map(|h| h as f32).or_else(|| {
        cx.primary_display()
            .or_else(|| cx.displays().into_iter().next())
            .map(|d| f32::from(d.bounds().size.height))
    })
}

/// Wayland: no anchor, so the compositor centres it, as the GTK port does.
/// X11: centred, top aligned with the split panel's; placed again by `decorate`.
pub fn bounds(size: Size<Pixels>, cx: &App) -> Bounds<Pixels> {
    let origin = match cx.primary_display().filter(|_| is_x11()) {
        Some(display) => {
            let screen = display.bounds();
            let split_h = px(crate::theme::window_h());
            point(
                (screen.origin.x + (screen.size.width - size.width) / 2.0).round(),
                (screen.origin.y + (screen.size.height - split_h) / 2.0).round(),
            )
        }
        None => point(px(0.0), px(0.0)),
    };
    Bounds { origin, size }
}

/// The size last asked of X, so each frame does not ask again.
static FITTED: Mutex<Option<Size<Pixels>>> = Mutex::new(None);

pub fn fitted(size: Size<Pixels>) {
    *FITTED.lock().unwrap_or_else(|p| p.into_inner()) = Some(size);
}

/// Bare X11: resize and recentre the window on a layout switch.
pub fn fit(window: &mut Window, frame: Size<Pixels>, cx: &App) {
    use x11rb::protocol::xproto::{ConfigureWindowAux, ConnectionExt};

    if !opaque() {
        return;
    }
    {
        let mut last = FITTED.lock().unwrap_or_else(|p| p.into_inner());
        if *last == Some(frame) {
            return;
        }
        *last = Some(frame);
    }
    let (Some(wid), Ok((conn, _))) = (x11_window(window), x11rb::connect(None)) else {
        window.resize(frame);
        return;
    };
    let target = bounds(frame, cx);
    let scale = window.scale_factor();
    let device = |v: Pixels| (f32::from(v) * scale).round();
    let aux = ConfigureWindowAux::new()
        .x(device(target.origin.x) as i32)
        .y(device(target.origin.y) as i32)
        .width(device(frame.width) as u32)
        .height(device(frame.height) as u32);
    let _ = conn.configure_window(wid, &aux);
    x11_sync(&conn);
}

/// Round trip so the server applies requests before the connection drops.
fn x11_sync(conn: &impl x11rb::protocol::xproto::ConnectionExt) {
    let _ = conn.get_input_focus().map(|c| c.reply());
}

/// X11, after gpui has asked for the map: drop the frame gpui requested and
/// move the window where `bounds` says, since a tiling WM (i3) places a window
/// itself on map and only takes a position from a mapped one. Then ask for
/// focus, as the Tauri shell's toggle does.
///
/// gpui writes its own `_MOTIF_WM_HINTS` (a frame) on its connection, which
/// may not be flushed yet, so the fix-up waits off thread for the map.
pub fn decorate(window: &Window, bounds: Bounds<Pixels>) {
    if !is_x11() {
        return;
    }
    if let Some(wid) = x11_window(window) {
        let scale = window.scale_factor();
        let x = (f32::from(bounds.origin.x) * scale).round() as i32;
        let y = (f32::from(bounds.origin.y) * scale).round() as i32;
        std::thread::spawn(move || x11_undecorate_and_place(wid, x, y));
    }
    window.activate_window();
}

fn x11_window(window: &Window) -> Option<u32> {
    match HasWindowHandle::window_handle(window).ok()?.as_raw() {
        RawWindowHandle::Xcb(handle) => Some(handle.window.get()),
        RawWindowHandle::Xlib(handle) => u32::try_from(handle.window).ok(),
        _ => None,
    }
}

/// Once the window is viewable: `_MOTIF_WM_HINTS` with only the decorations
/// flag set and no decorations, which i3 reads as `border none`; then a
/// ConfigureWindow for the origin. On i3 the border is also set over its IPC,
/// which does not depend on when it reads the hints.
fn x11_undecorate_and_place(wid: u32, x: i32, y: i32) {
    use std::time::{Duration, Instant};
    use x11rb::protocol::xproto::{ConfigureWindowAux, ConnectionExt, MapState, PropMode};
    use x11rb::wrapper::ConnectionExt as _;

    const MAP_WAIT: Duration = Duration::from_millis(500);
    const MAP_POLL: Duration = Duration::from_millis(5);

    let Ok((conn, _)) = x11rb::connect(None) else {
        return;
    };
    let started = Instant::now();
    loop {
        let viewable = conn
            .get_window_attributes(wid)
            .ok()
            .and_then(|c| c.reply().ok())
            .map(|a| a.map_state == MapState::VIEWABLE);
        match viewable {
            Some(true) => break,
            // Gone already: hidden before the WM got to it.
            None => return,
            Some(false) if started.elapsed() > MAP_WAIT => break,
            Some(false) => std::thread::sleep(MAP_POLL),
        }
    }
    let motif = conn
        .intern_atom(false, b"_MOTIF_WM_HINTS")
        .ok()
        .and_then(|c| c.reply().ok());
    if let Some(motif) = motif {
        let _ = conn.change_property32(
            PropMode::REPLACE,
            wid,
            motif.atom,
            motif.atom,
            &[1 << 1, 0, 0, 0, 0],
        );
    }
    let _ = conn.configure_window(wid, &ConfigureWindowAux::new().x(x).y(y));
    x11_sync(&conn);
    if std::env::var_os("I3SOCK").is_some() {
        let _ = std::process::Command::new("i3-msg")
            .arg(format!("[id={wid}] border none"))
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
}

/// On X11 a layer shell kind falls back to a normal toplevel, which a tiling
/// WM such as i3 tiles. A dialog stays managed (focus, stacking) but floats:
/// the role min == max size hints play for the Tauri window, which gpui does
/// not let a caller set.
pub fn kind() -> WindowKind {
    if is_x11() {
        return WindowKind::Dialog;
    }
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
