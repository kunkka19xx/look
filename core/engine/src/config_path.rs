//! Where the config file lives, and the one-time move into `~/.look/`.
//!
//! Look reads AND writes this file (the app upserts the `ui_*` keys), so every
//! caller must agree on the answer. Three implementations resolve it today (this
//! one, the macOS `ThemeStore`, and the linows config module), and if they ever
//! disagree the app reads one file and saves to another, which looks like
//! settings silently not persisting. That is the failure mode this module
//! exists to prevent: resolve once, hand back the path, read and write there.

use std::fs;
use std::path::{Path, PathBuf};

/// Overrides everything, for tests and for people who keep dotfiles elsewhere.
pub const ENV_CONFIG_PATH: &str = "LOOK_CONFIG_PATH";

/// The home-relative locations, newest first. A dev build keeps its own pair so
/// running Look Dev never edits the settings of the installed copy.
const CONFIG_DIR: &str = ".look";
const CONFIG_NAME: &str = "config";
const DEV_CONFIG_NAME: &str = "config.dev";
const LEGACY_CONFIG_NAME: &str = ".look.config";

/// Prepended to the legacy file once its contents have been copied across, so
/// someone who edits it out of habit is told why nothing happened. Doubles as
/// the "already moved" flag: a deliberate delete of the new file must not be
/// silently undone by copying the old one back.
const MOVED_NOTICE_PREFIX: &str = "# Moved to ~/.look/";

/// What resolution found, so a caller can report it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedConfig {
    pub path: PathBuf,
    /// True when the legacy file was copied into `~/.look/` by this call.
    pub migrated: bool,
}

/// The config file to read and write. `$LOOK_CONFIG_PATH` overrides everything;
/// otherwise this is `resolve_home`.
pub fn resolve(home: &Path) -> ResolvedConfig {
    if let Ok(custom) = std::env::var(ENV_CONFIG_PATH) {
        let trimmed = custom.trim();
        if !trimmed.is_empty() {
            return ResolvedConfig {
                path: PathBuf::from(trimmed),
                migrated: false,
            };
        }
    }
    resolve_home(home)
}

/// Resolution against a home directory, ignoring the environment. Split out so
/// the move can be tested without an ambient `$LOOK_CONFIG_PATH` deciding the
/// answer for every case.
pub fn resolve_home(home: &Path) -> ResolvedConfig {
    resolve_home_variant(home, false)
}

/// `dev` selects the file a development build uses, so running it never edits
/// the installed copy's settings.
///
/// The dev file is never migrated: it belongs to whoever is working on Look and
/// is created by hand, so a released build has nothing to move and no business
/// touching it.
pub fn resolve_home_variant(home: &Path, dev: bool) -> ResolvedConfig {
    if dev {
        return ResolvedConfig {
            path: ensured(home.join(CONFIG_DIR).join(DEV_CONFIG_NAME)),
            migrated: false,
        };
    }

    let current = home.join(CONFIG_DIR).join(CONFIG_NAME);
    if current.exists() {
        return ResolvedConfig {
            path: current,
            migrated: false,
        };
    }

    let legacy = home.join(LEGACY_CONFIG_NAME);
    if !legacy.exists() {
        // A fresh install: nothing to migrate, and `~/.look/` may not exist
        // yet. Every caller here goes on to WRITE this path, so the directory
        // has to be there or the default config is silently never created and
        // settings never persist.
        return ResolvedConfig {
            path: ensured(current),
            migrated: false,
        };
    }

    // A symlinked config is someone's dotfile repo. Copying would turn it into
    // a plain file and their repo edits would stop applying, so it is read
    // where it lies and never moved.
    if fs::symlink_metadata(&legacy).is_ok_and(|meta| meta.file_type().is_symlink()) {
        return ResolvedConfig {
            path: legacy,
            migrated: false,
        };
    }

    match migrate(&legacy, &current, CONFIG_NAME) {
        Ok(()) => ResolvedConfig {
            path: current,
            migrated: true,
        },
        // A failed copy must not lose the user's settings: keep reading the
        // file that definitely has them.
        Err(_) => ResolvedConfig {
            path: legacy,
            migrated: false,
        },
    }
}

/// Makes sure the returned path is writable by creating its parent. Failure is
/// left to the caller's own write, which reports it in context.
fn ensured(path: PathBuf) -> PathBuf {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    path
}

/// Copies the legacy file across and marks it, leaving the original in place so
/// nothing is destroyed and a downgrade still finds its settings.
fn migrate(legacy: &Path, current: &Path, name: &str) -> std::io::Result<()> {
    let contents = fs::read_to_string(legacy)?;
    if contents.starts_with(MOVED_NOTICE_PREFIX) {
        // Already moved once, and the new file has since been deleted. Honour
        // that rather than copying the stale contents back.
        return Err(std::io::Error::other("already migrated"));
    }

    if let Some(parent) = current.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(current, &contents)?;
    let notice = format!("{MOVED_NOTICE_PREFIX}{name} - Look no longer reads this file.");
    fs::write(legacy, format!("{notice}\n\n{contents}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempHome(PathBuf);

    impl TempHome {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "look-config-path-{label}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("temp home");
            Self(path)
        }

        fn legacy(&self) -> PathBuf {
            self.0.join(LEGACY_CONFIG_NAME)
        }

        fn current(&self) -> PathBuf {
            self.0.join(CONFIG_DIR).join(CONFIG_NAME)
        }
    }

    impl Drop for TempHome {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_new_install_gets_a_folder_path_it_can_actually_write() {
        // Every caller writes the resolved path. Without the directory the
        // default config is never created, and the user's settings silently
        // fail to persist on every launch.
        let home = TempHome::new("fresh");
        let resolved = resolve_home(&home.0);
        assert_eq!(resolved.path, home.current());
        assert!(!resolved.migrated);
        assert!(resolved.path.parent().is_some_and(Path::exists));
        fs::write(&resolved.path, "ui_theme=x\n").expect("the path must be writable");
    }

    #[test]
    fn a_dev_path_is_writable_too() {
        let home = TempHome::new("devwrite");
        let resolved = resolve_home_variant(&home.0, true);
        fs::write(&resolved.path, "ui_theme=x\n").expect("the path must be writable");
    }

    #[test]
    fn an_existing_user_is_moved_once_and_keeps_their_settings() {
        let home = TempHome::new("migrate");
        fs::write(home.legacy(), "ui_theme=dracula\n").expect("legacy");

        let resolved = resolve_home(&home.0);
        assert_eq!(resolved.path, home.current());
        assert!(resolved.migrated);
        assert_eq!(
            fs::read_to_string(home.current()).unwrap(),
            "ui_theme=dracula\n"
        );

        // The original survives, so a downgrade still finds its settings, and
        // it says why editing it no longer does anything.
        let legacy = fs::read_to_string(home.legacy()).unwrap();
        assert!(legacy.starts_with(MOVED_NOTICE_PREFIX));
        assert!(legacy.contains("ui_theme=dracula"));
    }

    #[test]
    fn the_folder_file_wins_once_it_exists() {
        let home = TempHome::new("prefer");
        fs::create_dir_all(home.current().parent().unwrap()).unwrap();
        fs::write(home.current(), "ui_theme=new\n").unwrap();
        fs::write(home.legacy(), "ui_theme=old\n").unwrap();

        let resolved = resolve_home(&home.0);
        assert_eq!(resolved.path, home.current());
        assert!(!resolved.migrated);
        // The legacy file is left exactly as it was: nothing to migrate.
        assert_eq!(fs::read_to_string(home.legacy()).unwrap(), "ui_theme=old\n");
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_config_is_read_where_it_lies() {
        // Copying would turn a dotfile-repo symlink into a plain file and
        // silently stop the user's repo edits from applying.
        let home = TempHome::new("symlink");
        let real = home.0.join("dotfiles-look.config");
        fs::write(&real, "ui_theme=linked\n").unwrap();
        std::os::unix::fs::symlink(&real, home.legacy()).unwrap();

        let resolved = resolve_home(&home.0);
        assert_eq!(resolved.path, home.legacy());
        assert!(!resolved.migrated);
        assert!(!home.current().exists(), "nothing was copied");
    }

    #[test]
    fn a_dev_build_has_its_own_file_and_never_migrates() {
        // The dev file is a developer's own copy, placed by hand. A released
        // build has nothing to move and must not touch the installed settings.
        let home = TempHome::new("dev");
        fs::write(home.legacy(), "ui_theme=installed\n").unwrap();

        let resolved = resolve_home_variant(&home.0, true);
        assert_eq!(resolved.path, home.0.join(CONFIG_DIR).join(DEV_CONFIG_NAME));
        assert!(!resolved.migrated);
        assert_eq!(
            fs::read_to_string(home.legacy()).unwrap(),
            "ui_theme=installed\n",
            "the installed config is left alone"
        );
    }

    #[test]
    fn a_deleted_new_file_is_not_silently_recreated_from_the_old_one() {
        let home = TempHome::new("deleted");
        fs::write(home.legacy(), "ui_theme=dracula\n").unwrap();

        assert!(resolve_home(&home.0).migrated);
        fs::remove_file(home.current()).unwrap();

        // The marker says the move already happened, so the stale contents do
        // not come back.
        let again = resolve_home(&home.0);
        assert_eq!(again.path, home.legacy());
        assert!(!again.migrated);
    }
}
