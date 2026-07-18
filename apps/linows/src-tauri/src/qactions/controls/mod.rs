//! Per-control adapters. One file per control, all of its OS-specific code
//! quarantined inside (see docs/writing-controls.md). `bluetooth` is the
//! reference implementation to copy.

#[cfg(target_os = "linux")]
pub mod bluetooth;

#[cfg(target_os = "windows")]
pub mod bluetooth_windows;
