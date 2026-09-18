use crate::state::store_json_allocation;
use look_engine::config::RuntimeConfig;
use look_engine::hotkey::HotkeyCheck;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

const NULL_JSON: &str = "null";

pub(crate) fn look_launcher_hotkey_json_impl() -> *mut c_char {
    allocate(serde_json::to_string(&RuntimeConfig::load_cached().launcher_hotkey).ok())
}

pub(crate) fn look_hotkey_check_json_impl(spec: *const c_char) -> *mut c_char {
    if spec.is_null() {
        return allocate(None);
    }
    let spec = unsafe { CStr::from_ptr(spec) }.to_str().ok();
    allocate(spec.and_then(|spec| serde_json::to_string(&HotkeyCheck::new(spec)).ok()))
}

fn allocate(json: Option<String>) -> *mut c_char {
    let json = json.unwrap_or_else(|| NULL_JSON.to_string());
    store_json_allocation(CString::new(json).unwrap_or_else(|_| c"null".to_owned()))
}
