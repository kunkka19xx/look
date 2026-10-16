//! The parts of the shell that are bound to GTK, WebKitGTK or Tauri's window,
//! and so stay out of the backend crate.

#[cfg(target_os = "linux")]
pub mod blur;
#[cfg(target_os = "linux")]
pub mod clipboard;
#[cfg(target_os = "windows")]
pub mod effects;
#[cfg(target_os = "linux")]
pub mod gpu;
#[cfg(target_os = "linux")]
pub mod layer_shell;
