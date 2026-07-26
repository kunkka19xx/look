// Shared constants used across multiple modules.

pub const MAIN_WINDOW: &str = "main";
pub const EVENT_INDEX_READY: &str = "index-ready";
/// Emitted right before the launcher window hides, so the frontend can pin the
/// launchpad to its entrance-start pose while the webview can still paint. Keeps
/// the next summon from flashing the fully-visible strip then rewinding it (see
/// superactions.armEntrance). Paired with the show-side `window-shown`.
pub const EVENT_WINDOW_HIDDEN: &str = "window-hidden";

/// Windows process creation flag to suppress console windows.
#[cfg(target_os = "windows")]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;
