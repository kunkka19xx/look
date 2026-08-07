//! Thin Tauri bridge to the shared `look-netspeed` core crate, the same
//! measurement macOS reaches through the FFI bridge.

use look_netspeed::SpeedReading;

/// Runs on Tauri's blocking pool: the measurement blocks for many seconds.
#[tauri::command(async)]
pub fn speed_test() -> Result<SpeedReading, String> {
    look_netspeed::run().map_err(|error| error.message().to_string())
}
