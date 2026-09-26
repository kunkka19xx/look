//! Typing into somebody else's window through `zwp_virtual_keyboard_v1`.
//!
//! Wayland gives a client no way to reach another client's surface, which is
//! the point of it. The one door the compositor leaves open is a virtual
//! keyboard: a device it treats as real, so whatever it types lands wherever
//! the focus is. sway, niri, Hyprland, river and KWin all open it. Mutter does
//! not, and GNOME goes through the shell extension instead.

use std::io::{Seek, SeekFrom, Write};
use std::os::fd::AsFd;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use wayland_client::protocol::{wl_keyboard, wl_registry, wl_seat};
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
    zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
    zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
};

/// The keymap below puts its one key at xkb keycode 9; the protocol counts in
/// evdev codes, which are the xkb ones less 8.
const V_KEYCODE: u32 = 1;
const KEYMAP_FORMAT_XKB_V1: u32 = 1;
const SHIFT_MASK: u32 = 1;
const CTRL_MASK: u32 = 4;
/// Long enough for the compositor to deliver both events before the connection
/// drops and takes the device with it.
const HANDOVER: Duration = Duration::from_millis(60);

/// A full layout would buy nothing: the app on the other end reads the keysym
/// this maps to, and the modifier state is sent outright rather than typed.
const KEYMAP: &str = r#"xkb_keymap {
xkb_keycodes "look" {
    minimum = 8;
    maximum = 255;
    <V> = 9;
};
xkb_types "look" { include "basic" };
xkb_compatibility "look" { include "basic" };
xkb_symbols "look" {
    key <V> { [ v, V ] };
};
};
"#;

pub fn available() -> bool {
    globals().is_some()
}

pub fn send_paste(shift: bool) -> bool {
    // Held, not used: dropping the connection destroys the device with it.
    let Some((_conn, mut queue, keyboard)) = bind() else {
        return false;
    };
    let Some((keymap, size)) = keymap_file() else {
        return false;
    };

    keyboard.keymap(KEYMAP_FORMAT_XKB_V1, keymap.as_fd(), size);
    let mods = CTRL_MASK | if shift { SHIFT_MASK } else { 0 };
    keyboard.modifiers(mods, 0, 0, 0);
    let time = now_ms();
    keyboard.key(time, V_KEYCODE, wl_keyboard::KeyState::Pressed as u32);
    keyboard.key(time + 1, V_KEYCODE, wl_keyboard::KeyState::Released as u32);
    // Left held, the compositor would keep Ctrl down for the next real key.
    keyboard.modifiers(0, 0, 0, 0);

    let mut state = State::default();
    if queue.roundtrip(&mut state).is_err() {
        return false;
    }
    std::thread::sleep(HANDOVER);
    true
}

/// The size counts the terminating NUL: what the compositor mmaps it parses as
/// a C string. Unlinked at once, since nothing else needs to find it.
fn keymap_file() -> Option<(std::fs::File, u32)> {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let path = dir.join(format!("look-keymap-{}", std::process::id()));
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .read(true)
        .write(true)
        .open(&path)
        .ok()?;
    let _ = std::fs::remove_file(&path);
    file.write_all(KEYMAP.as_bytes()).ok()?;
    file.write_all(&[0]).ok()?;
    file.seek(SeekFrom::Start(0)).ok()?;
    let size = u32::try_from(KEYMAP.len() + 1).ok()?;
    Some((file, size))
}

fn now_ms() -> u32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u32)
        .unwrap_or(0)
}

/// Nothing is created here: asking whether a paste is possible must not leave
/// a device behind.
fn globals() -> Option<(Connection, wayland_client::EventQueue<State>, State)> {
    let conn = Connection::connect_to_env().ok()?;
    let mut queue = conn.new_event_queue::<State>();
    let qh = queue.handle();
    let _registry = conn.display().get_registry(&qh, ());

    let mut state = State::default();
    queue.roundtrip(&mut state).ok()?;
    state.seat.as_ref()?;
    state.manager.as_ref()?;
    Some((conn, queue, state))
}

fn bind() -> Option<(
    Connection,
    wayland_client::EventQueue<State>,
    ZwpVirtualKeyboardV1,
)> {
    let (conn, queue, mut state) = globals()?;
    let manager = state.manager.take()?;
    let seat = state.seat.take()?;
    let keyboard = manager.create_virtual_keyboard(&seat, &queue.handle(), ());
    Some((conn, queue, keyboard))
}

#[derive(Default)]
struct State {
    seat: Option<wl_seat::WlSeat>,
    manager: Option<ZwpVirtualKeyboardManagerV1>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        else {
            return;
        };
        match interface.as_str() {
            "wl_seat" => state.seat = Some(registry.bind(name, version.min(7), qh, ())),
            "zwp_virtual_keyboard_manager_v1" => {
                state.manager = Some(registry.bind(name, version.min(1), qh, ()))
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for State {
    fn event(
        _: &mut Self,
        _: &wl_seat::WlSeat,
        _: wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpVirtualKeyboardManagerV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ZwpVirtualKeyboardManagerV1,
        _: <ZwpVirtualKeyboardManagerV1 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpVirtualKeyboardV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ZwpVirtualKeyboardV1,
        _: <ZwpVirtualKeyboardV1 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
