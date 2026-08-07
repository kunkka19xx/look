use crate::state::store_json_allocation;
use std::ffi::CString;
use std::os::raw::c_char;

/// Returned when the reading cannot be serialized at all, so the shell still
/// gets the shape it decodes rather than a null pointer.
const FAILURE_JSON: &str = r#"{"ok":false,"error":"Speed test failed"}"#;

pub(crate) fn look_netspeed_run_json_impl() -> *mut c_char {
    let json = look_netspeed::run_json();
    let cstring = CString::new(json)
        .unwrap_or_else(|_| CString::new(FAILURE_JSON).expect("valid static json"));
    store_json_allocation(cstring)
}
