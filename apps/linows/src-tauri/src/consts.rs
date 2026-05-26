// Shared constants used across multiple modules.

pub const MAIN_WINDOW: &str = "main";
pub const EVENT_INDEX_READY: &str = "index-ready";

/// Windows process creation flag to suppress console windows.
#[cfg(target_os = "windows")]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;
