//! Per-control adapters. One file per control, all of its OS-specific code
//! quarantined inside (see docs/writing-controls.md). `bluetooth` is the
//! reference implementation to copy.

#[cfg(target_os = "linux")]
pub mod battery;
#[cfg(target_os = "linux")]
pub mod bluetooth;
#[cfg(target_os = "linux")]
pub mod keepawake;
#[cfg(target_os = "linux")]
pub mod mic;
#[cfg(target_os = "linux")]
pub mod power;
#[cfg(target_os = "linux")]
pub mod screensaver;
#[cfg(target_os = "linux")]
pub mod theme;
#[cfg(target_os = "linux")]
pub mod wifi;

#[cfg(target_os = "windows")]
pub mod battery_windows;
#[cfg(target_os = "windows")]
pub mod bluetooth_windows;
#[cfg(target_os = "windows")]
pub mod keepawake_windows;
#[cfg(target_os = "windows")]
pub mod mic_windows;
#[cfg(target_os = "windows")]
pub mod power_windows;
#[cfg(target_os = "windows")]
pub mod screensaver_windows;
#[cfg(target_os = "windows")]
pub mod theme_windows;
#[cfg(target_os = "windows")]
pub mod wifi_windows;
