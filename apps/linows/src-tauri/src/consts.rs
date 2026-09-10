// Shared constants used across multiple modules.

pub const MAIN_WINDOW: &str = "main";
pub const EVENT_INDEX_READY: &str = "index-ready";
/// Emitted every time the launcher is shown. Carries show-time decisions the
/// frontend should apply before replaying focus/animations.
pub const EVENT_WINDOW_SHOWN: &str = "window-shown";
/// Emitted right before the launcher window hides, so the frontend can pin the
/// launchpad to its entrance-start pose while the webview can still paint. Keeps
/// the next summon from flashing the fully-visible strip then rewinding it (see
/// superactions.armEntrance). Paired with the show-side `window-shown`.
pub const EVENT_WINDOW_HIDDEN: &str = "window-hidden";

/// Windows process creation flag to suppress console windows.
#[cfg(target_os = "windows")]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Delay between starting something and trying to focus the window it opened -
/// long enough for the app to have received the input and drawn. Used by the
/// file/URL open path and by the preferred-tools launcher on both platforms.
pub const HANDLER_FOCUS_DELAY_MS: u64 = 150;

/// Rendering workarounds (Linux only). Both shipped as `arch_*` before the
/// ghosting turned out to be a WebKitGTK trait rather than an Arch one; the
/// old names are still read so an existing config keeps its setting.
#[cfg(target_os = "linux")]
pub const KEY_DISABLE_GPU: &str = "disable_gpu_compositing";
#[cfg(target_os = "linux")]
pub const KEY_DISABLE_GPU_LEGACY: &str = "arch_disable_gpu";
