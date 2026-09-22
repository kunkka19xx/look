//! The paste chord, typed into whatever window has the keyboard.
//!
//! Three doors, one per session kind: XTEST on X11, `zwp_virtual_keyboard_v1`
//! on the wlroots family and KWin, and the Look shell extension on GNOME,
//! whose Mutter offers neither.

use x11rb::connection::{Connection, RequestConnection as _};
use x11rb::protocol::xproto::{ConnectionExt as _, KEY_PRESS_EVENT, KEY_RELEASE_EVENT, ModMask};
use x11rb::protocol::xtest::ConnectionExt as _;

use super::{gnome_ext, transparency, virtual_keyboard};

const KEYSYM_V: u32 = 0x0076;
const KEYSYM_CONTROL_L: u32 = 0xffe3;
const KEYSYM_SHIFT_L: u32 = 0xffe1;

/// XTEST's own device, as opposed to a real one it could impersonate.
const XTEST_DEVICE: u8 = 0;
const NOW: u32 = 0;

pub fn available() -> bool {
    if transparency::is_wayland() {
        return virtual_keyboard::available() || gnome_ext::focused_app_id().is_some();
    }
    x11_available()
}

/// `shift` for terminals, where Ctrl+V types a literal and Ctrl+Shift+V pastes.
pub fn send_paste(shift: bool) -> bool {
    if transparency::is_wayland() {
        return virtual_keyboard::send_paste(shift) || gnome_ext::send_paste(shift);
    }
    x11_send_paste(shift)
}

fn x11_available() -> bool {
    let Ok((conn, _)) = x11rb::connect(None) else {
        return false;
    };
    conn.extension_information(x11rb::protocol::xtest::X11_EXTENSION_NAME)
        .ok()
        .flatten()
        .is_some()
}

fn x11_send_paste(shift: bool) -> bool {
    let Ok((conn, screen_num)) = x11rb::connect(None) else {
        return false;
    };
    let Some(v) = keycode_for(&conn, KEYSYM_V) else {
        return false;
    };

    // Typing a modifier the fingers are still on would release it for them.
    let held = held_modifiers(&conn, screen_num);
    let mut mods = Vec::new();
    if held & u16::from(ModMask::CONTROL) == 0 {
        mods.extend(keycode_for(&conn, KEYSYM_CONTROL_L));
    }
    if shift && held & u16::from(ModMask::SHIFT) == 0 {
        mods.extend(keycode_for(&conn, KEYSYM_SHIFT_L));
    }

    for code in &mods {
        if !fake_key(&conn, *code, true) {
            return false;
        }
    }
    let typed = fake_key(&conn, v, true) && fake_key(&conn, v, false);
    for code in mods.iter().rev() {
        fake_key(&conn, *code, false);
    }
    conn.flush().is_ok() && typed
}

/// Sent and accepted: a fake key the server refuses comes back on the reply.
fn fake_key(conn: &impl Connection, keycode: u8, press: bool) -> bool {
    let event = if press {
        KEY_PRESS_EVENT
    } else {
        KEY_RELEASE_EVENT
    };
    conn.xtest_fake_input(event, keycode, NOW, x11rb::NONE, 0, 0, XTEST_DEVICE)
        .is_ok_and(|cookie| cookie.check().is_ok())
}

fn held_modifiers(conn: &impl Connection, screen_num: usize) -> u16 {
    let Some(screen) = conn.setup().roots.get(screen_num) else {
        return 0;
    };
    conn.query_pointer(screen.root)
        .ok()
        .and_then(|cookie| cookie.reply().ok())
        .map(|reply| u16::from(reply.mask))
        .unwrap_or(0)
}

/// Looked up rather than assumed at a position: the app on the other end
/// matches the character, which sits elsewhere on Dvorak and Colemak.
fn keycode_for(conn: &impl Connection, keysym: u32) -> Option<u8> {
    let setup = conn.setup();
    let min = setup.min_keycode;
    let count = setup.max_keycode.checked_sub(min)?.checked_add(1)?;
    let mapping = conn.get_keyboard_mapping(min, count).ok()?.reply().ok()?;
    let per_code = usize::from(mapping.keysyms_per_keycode);
    if per_code == 0 {
        return None;
    }
    let index = mapping
        .keysyms
        .chunks(per_code)
        .position(|level| level.contains(&keysym))?;
    u8::try_from(usize::from(min) + index).ok()
}
