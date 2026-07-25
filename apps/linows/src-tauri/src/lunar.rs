//! Thin Tauri bridge to the shared `look-lunar` core crate. The frontend owns
//! "now" in local time, so it passes its calendar date and UTC offset (hours)
//! and core does the timezone-sensitive conversion - the same function macOS
//! reaches through the FFI bridge.

/// Convert a Gregorian date to its lunar date at the given UTC offset.
#[tauri::command]
pub fn lunar_date(year: i64, month: i64, day: i64, tz: f64) -> look_lunar::LunarDate {
    look_lunar::solar_to_lunar(year, month, day, tz)
}
