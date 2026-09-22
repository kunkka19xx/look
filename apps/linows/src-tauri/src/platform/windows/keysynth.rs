//! The paste chord, typed into whatever window has the keyboard.
//!
//! Windows asks for no permission here: a foreground process may inject into
//! the queue, and Look is the foreground process right up to the hide. The one
//! thing to get right is the order.

use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput,
    VIRTUAL_KEY, VK_CONTROL, VK_V, VkKeyScanW,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

const KEY_DOWN_BIT: i16 = -0x8000;
/// The low byte of a `VkKeyScanW` result is the virtual key, the high byte the
/// shift state the chord supplies.
const VK_MASK: u16 = 0xff;
const NO_SUCH_KEY: i16 = -1;

/// `shift` is carried for the shape of the Linux call and never set: Windows
/// Terminal, the console host and every editor take plain Ctrl+V.
pub fn send_paste(_shift: bool) -> bool {
    let key = paste_key();
    // Typing a modifier the fingers are still on would release it for them.
    let hold_ctrl = !is_down(VK_CONTROL);
    let mut inputs = Vec::with_capacity(4);
    if hold_ctrl {
        inputs.push(key_input(VK_CONTROL, false));
    }
    inputs.push(key_input(key, false));
    inputs.push(key_input(key, true));
    if hold_ctrl {
        inputs.push(key_input(VK_CONTROL, true));
    }

    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    sent as usize == inputs.len()
}

pub fn self_is_foreground() -> bool {
    let hwnd = unsafe { GetForegroundWindow() };
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    pid == std::process::id()
}

/// The key that types "v" on the active layout, which is elsewhere on Dvorak
/// and Colemak.
fn paste_key() -> VIRTUAL_KEY {
    let scan = unsafe { VkKeyScanW('v' as u16) };
    if scan == NO_SUCH_KEY {
        return VK_V;
    }
    VIRTUAL_KEY(scan as u16 & VK_MASK)
}

fn is_down(key: VIRTUAL_KEY) -> bool {
    unsafe { GetAsyncKeyState(key.0 as i32) & KEY_DOWN_BIT != 0 }
}

fn key_input(key: VIRTUAL_KEY, release: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                dwFlags: if release {
                    KEYEVENTF_KEYUP
                } else {
                    Default::default()
                },
                ..Default::default()
            },
        },
    }
}
