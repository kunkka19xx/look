//! What every shell does before it opens a window.

use crate::{autostart, cli_path, config};

/// Set dev-mode config and database paths so dev doesn't pollute production.
/// SAFETY: Must be called at startup before any threads are spawned.
#[cfg(debug_assertions)]
pub fn setup_dev_env() {
    use crate::state;

    // The engine's answer, not a second one: Windows can have $HOME and
    // $USERPROFILE pointing at different directories.
    let home = look_engine::config_path::home().unwrap_or_else(|| ".".to_string());

    if std::env::var(config::ENV_CONFIG_PATH)
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        // `~/.look/config.dev`, never migrated: a dev file is written by hand.
        let dev =
            look_engine::config_path::resolve_home_variant(std::path::Path::new(&home), true).path;
        unsafe {
            std::env::set_var(config::ENV_CONFIG_PATH, dev);
        }
    }
    if std::env::var(state::ENV_DB_PATH)
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        #[cfg(target_os = "windows")]
        let db_dir = std::env::var("LOCALAPPDATA")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::PathBuf::from(&home)
                    .join("AppData")
                    .join("Local")
            })
            .join("look");

        #[cfg(not(target_os = "windows"))]
        let db_dir = std::env::var("XDG_DATA_HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::path::PathBuf::from(&home).join(".local").join("share"))
            .join("look");

        let _ = std::fs::create_dir_all(&db_dir);
        unsafe {
            std::env::set_var(state::ENV_DB_PATH, db_dir.join("look.dev.db"));
        }
    }
    eprintln!(
        "[dev] config={} db={}",
        std::env::var(config::ENV_CONFIG_PATH).unwrap_or_default(),
        std::env::var(state::ENV_DB_PATH).unwrap_or_default(),
    );
}

/// Sync OS integrations (autostart, PATH) with config on every launch, so the
/// registered exe path stays valid after updates or reinstalls.
///
/// Debug builds live under target/debug and may load the frontend from a dev
/// server. Registering them would launch or shadow the installed binary with
/// one that fails without it, so skip.
pub fn sync_integrations() {
    if cfg!(debug_assertions) {
        return;
    }

    let content = std::fs::read_to_string(config::config_file_path()).unwrap_or_default();

    const KEY: &str = "launch_at_login";
    let enabled = config_flag(&content, KEY).unwrap_or_else(|| {
        // First launch: enable by default and persist.
        let _ = config::set_config(vec![config::ConfigUpdate {
            key: KEY.into(),
            value: "true".into(),
        }]);
        true
    });
    let _ = autostart::set_autostart(enabled);

    // No default for PATH: an absent key leaves it alone, since the install
    // script may have added the entry already. Off the main thread because the
    // environment broadcast can block for up to a second.
    if let Some(enabled) = config_flag(&content, "add_to_path") {
        std::thread::spawn(move || {
            let _ = cli_path::set_cli_path(enabled);
        });
    }
}

/// Reads a `key=true|false` line straight off the config text. Cheaper than a
/// full parse, and runs before the window opens.
fn config_flag(content: &str, key: &str) -> Option<bool> {
    content.lines().find_map(|line| {
        let line = line.trim();
        if line.starts_with('#') {
            return None;
        }
        line.split_once('=')
            .filter(|(k, _)| k.trim() == key)
            .map(|(_, v)| v.trim() == "true")
    })
}
