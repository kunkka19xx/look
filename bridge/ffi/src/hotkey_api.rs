use crate::state::store_json_allocation;
use look_engine::config::RuntimeConfig;
use look_engine::hotkey::HotkeyCheck;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

const NULL_JSON: &str = "null";

/// The configured launcher hotkey as a JSON `LauncherHotkey`, already resolved
/// to the platform default when the config value is missing or invalid.
pub(crate) fn look_launcher_hotkey_json_impl() -> *mut c_char {
    allocate(serde_json::to_string(
        &RuntimeConfig::load_cached().launcher_hotkey,
    ))
}

/// A hotkey spec checked against core's grammar, as a JSON `HotkeyCheck`
/// (`{spec, display, error}`), so a settings screen can validate before saving.
pub(crate) fn look_hotkey_check_json_impl(spec: *const c_char) -> *mut c_char {
    if spec.is_null() {
        return allocate(Ok(NULL_JSON.to_string()));
    }
    let Ok(spec) = (unsafe { CStr::from_ptr(spec) }).to_str() else {
        return allocate(Ok(NULL_JSON.to_string()));
    };
    allocate(serde_json::to_string(&HotkeyCheck::new(spec)))
}

fn allocate(json: serde_json::Result<String>) -> *mut c_char {
    let json = json.unwrap_or_else(|_| NULL_JSON.to_string());
    let cstring =
        CString::new(json).unwrap_or_else(|_| CString::new(NULL_JSON).expect("valid static json"));
    store_json_allocation(cstring)
}
