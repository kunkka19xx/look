use crate::state::store_json_allocation;
use look_lunar::solar_to_lunar;
use std::ffi::CString;
use std::os::raw::c_char;

/// The JSON returned when the conversion cannot be serialized. The shell treats
/// a `null` (or a null pointer) as "no lunar date", so it degrades gracefully.
const NULL_JSON: &str = "null";

/// Converts a Gregorian date to its lunar date at UTC offset `tz` (hours: 7 for
/// the Vietnamese calendar, 8 for the Chinese one) and returns it as a JSON
/// `LunarDate` object (`{day, month, year, leap}`). Free with `look_free_cstring`.
pub(crate) fn look_lunar_date_json_impl(year: i64, month: i64, day: i64, tz: f64) -> *mut c_char {
    let lunar = solar_to_lunar(year, month, day, tz);
    let json = serde_json::to_string(&lunar).unwrap_or_else(|_| NULL_JSON.to_string());
    let cstring =
        CString::new(json).unwrap_or_else(|_| CString::new(NULL_JSON).expect("valid static json"));
    store_json_allocation(cstring)
}
