//! System appearance toggle (dark <-> light) for Windows. Action id: `"theme"`.
//! Windows peer of `theme.rs`. Writes both `Personalize` registry values and
//! broadcasts a settings-change so apps repaint, as the Settings app does.
//! `On` == dark; the registry values are inverted (`1` light, `0` dark).

use crate::qactions::{ActionIntent, ActionOutcome, ActionState, SystemControl};
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, REG_DWORD, REG_SAM_FLAGS, REG_VALUE_TYPE,
    RegCloseKey, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    HWND_BROADCAST, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_SETTINGCHANGE,
};
use windows::core::PCWSTR;

const SUBKEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";
const APPS_KEY: &str = "AppsUseLightTheme";
const SYSTEM_KEY: &str = "SystemUsesLightTheme";
const UNAVAILABLE: &str = "Could not change the Windows theme";

/// Toggles the Windows light/dark appearance. Action id: `"theme"`.
pub struct ThemeControl;

impl SystemControl for ThemeControl {
    fn state(&self) -> ActionState {
        // Missing value means the Windows default (light), so treat it as off.
        if is_dark().unwrap_or(false) {
            ActionState::On
        } else {
            ActionState::Off
        }
    }

    fn apply(&self, intent: ActionIntent) -> ActionOutcome {
        let dark = match intent {
            ActionIntent::Toggle => !is_dark().unwrap_or(false),
            ActionIntent::SetOn(on) => on,
            ActionIntent::Run => {
                return ActionOutcome::Failed {
                    message: "Theme has no run action".to_string(),
                };
            }
        };
        if write_theme(dark) {
            broadcast_change();
            ActionOutcome::Ok {
                banner: Some(if dark { "Dark" } else { "Light" }.to_string()),
            }
        } else {
            ActionOutcome::Failed {
                message: UNAVAILABLE.to_string(),
            }
        }
    }
}

/// Whether apps are set to dark. `None` when the value is absent or unreadable.
fn is_dark() -> Option<bool> {
    read_dword(APPS_KEY).map(|light| light == 0)
}

/// A NUL-terminated wide string for the Win32 `W` APIs.
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Open the Personalize key with `access`, or `None` if it can't be opened.
fn open_key(access: REG_SAM_FLAGS) -> Option<HKEY> {
    let subkey = wide(SUBKEY);
    let mut hkey = HKEY::default();
    let open = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            None,
            access,
            &mut hkey,
        )
    };
    (open.0 == 0).then_some(hkey)
}

fn read_dword(name: &str) -> Option<u32> {
    let hkey = open_key(KEY_READ)?;
    let name_w = wide(name);
    let mut data_type = REG_VALUE_TYPE(0);
    let mut value: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;
    let query = unsafe {
        RegQueryValueExW(
            hkey,
            PCWSTR(name_w.as_ptr()),
            None,
            Some(&mut data_type),
            Some(&mut value as *mut u32 as *mut u8),
            Some(&mut size),
        )
    };
    let _ = unsafe { RegCloseKey(hkey) };
    (query.0 == 0 && data_type == REG_DWORD).then_some(value)
}

/// Set both theme values to `dark` (registry `0`) or light (`1`); whether both
/// writes succeeded.
fn write_theme(dark: bool) -> bool {
    let Some(hkey) = open_key(KEY_SET_VALUE) else {
        return false;
    };
    let light: u32 = if dark { 0 } else { 1 };
    let ok = write_dword(hkey, APPS_KEY, light) && write_dword(hkey, SYSTEM_KEY, light);
    let _ = unsafe { RegCloseKey(hkey) };
    ok
}

fn write_dword(hkey: HKEY, name: &str, value: u32) -> bool {
    let name_w = wide(name);
    let bytes = value.to_ne_bytes();
    let set =
        unsafe { RegSetValueExW(hkey, PCWSTR(name_w.as_ptr()), None, REG_DWORD, Some(&bytes)) };
    set.0 == 0
}

/// Tell running apps the theme changed so they repaint without a sign-out.
/// Best-effort and time-bounded: a hung top-level window must not block.
fn broadcast_change() {
    let param = wide("ImmersiveColorSet");
    unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            WPARAM(0),
            LPARAM(param.as_ptr() as isize),
            SMTO_ABORTIFHUNG,
            1000,
            None,
        );
    }
}
