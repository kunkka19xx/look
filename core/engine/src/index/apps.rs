use crate::config::RuntimeConfig;
use std::sync::mpsc;

pub fn discover_installed_apps(
    config: &RuntimeConfig,
    tx: mpsc::SyncSender<look_indexing::Candidate>,
) {
    #[cfg(target_os = "windows")]
    {
        crate::platform::discover_windows_installed_apps(config, tx);
    }

    #[cfg(not(target_os = "windows"))]
    {
        crate::platform::discover_macos_installed_apps(config, tx);
    }
}
