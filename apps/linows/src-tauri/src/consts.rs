// What the Tauri shell and its webview agree on.

pub const MAIN_WINDOW: &str = "main";
pub const EVENT_INDEX_READY: &str = "index-ready";
pub const EVENT_HEALTH_CHANGED: &str = "health-changed";
/// Emitted every time the launcher is shown. Carries show-time decisions the
/// frontend should apply before replaying focus/animations.
pub const EVENT_WINDOW_SHOWN: &str = "window-shown";
/// Emitted right before the launcher window hides, so the frontend can pin the
/// launchpad to its entrance-start pose while the webview can still paint. Keeps
/// the next summon from flashing the fully-visible strip then rewinding it (see
/// superactions.armEntrance). Paired with the show-side `window-shown`.
pub const EVENT_WINDOW_HIDDEN: &str = "window-hidden";
/// `lookapp reload-config` reaching the running instance. The frontend runs the
/// same reload as Ctrl+Shift+; so theme and fonts apply too, not just the engine.
pub const EVENT_CONFIG_RELOAD_REQUESTED: &str = "config-reload-requested";

/// Rendering workarounds (Linux only). Both shipped as `arch_*` before the
/// ghosting turned out to be a WebKitGTK trait rather than an Arch one; the
/// old names are still read so an existing config keeps its setting.
#[cfg(target_os = "linux")]
pub const KEY_DISABLE_GPU: &str = "disable_gpu_compositing";
#[cfg(target_os = "linux")]
pub const KEY_DISABLE_GPU_LEGACY: &str = "arch_disable_gpu";
