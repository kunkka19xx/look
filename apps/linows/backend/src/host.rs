//! What the backend needs from the shell that embeds it.
//!
//! The backend never owns a window or an event loop. A shell (Tauri today, gpui
//! next) installs one [`Host`] at startup for the process-wide hooks, and hands
//! a [`LauncherWindow`] to the few calls that take the launcher off screen or
//! bring it back.

use std::sync::OnceLock;

use crate::health::HealthIssue;

/// Process-wide hooks into the shell. Installed once, before any backend work
/// that could fire them.
pub trait Host: Send + Sync {
    /// The index finished a refresh the UI asked for; the result list should
    /// re-run its query.
    fn index_ready(&self);
    /// The setup health list changed; the UI shows the new snapshot.
    fn health_changed(&self, issues: Vec<HealthIssue>);
    /// Become the clipboard owner for `forms`, answering each MIME type on
    /// demand. `false` when the shell cannot, so the caller shells out instead.
    #[cfg(target_os = "linux")]
    fn own_clipboard(&self, forms: Vec<ClipForm>) -> bool;
}

/// One form a copy is offered in: every MIME spelling that asks for it, and
/// the bytes whoever asks receives.
#[cfg(target_os = "linux")]
pub struct ClipForm {
    pub targets: &'static [&'static str],
    pub payload: Vec<u8>,
}

/// The launcher window as the backend sees it. Every method is safe from any
/// thread; the shell routes to its main thread where it has to.
pub trait LauncherWindow: Send + Sync {
    /// Dismiss the launcher, playing whatever exit the shell has first.
    fn hide(&self);
    /// Dismiss at once, no exit motion.
    fn hide_now(&self);
    fn show(&self);
    fn focus(&self);
    fn is_visible(&self) -> bool;
}

static HOST: OnceLock<Box<dyn Host>> = OnceLock::new();

/// Install the shell's hooks. A second call is ignored.
pub fn install(host: Box<dyn Host>) {
    let _ = HOST.set(host);
}

/// The installed shell, `None` before [`install`] runs.
pub fn host() -> Option<&'static dyn Host> {
    HOST.get().map(|h| h.as_ref())
}
