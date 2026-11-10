//! The two directories declarations live in, watched so a manifest an app
//! installs while Look runs is read without a manual reload. The index
//! watcher never looks here: a change means a config reload, which restarts
//! connectors and runs the `run` blocks again, not an index pass.
//!
//! Each directory is watched flat. One that is missing is caught appearing
//! through its parent, `~/.look`, which the config already keeps there.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::state::WATCHER_DEBOUNCE_SECS;

/// An editor's write and rename, or an installer's several files, settle
/// into one reload.
const DEBOUNCE: Duration = Duration::from_secs(WATCHER_DEBOUNCE_SECS);
const POLL: Duration = Duration::from_millis(500);

/// Watches until the process ends and tells the host on every settled
/// change. Without a home there is nothing to watch.
pub fn start() {
    let Some(home) = crate::sources::home() else {
        return;
    };
    let dirs = [
        look_sources::sources_dir(&home),
        look_sources::connectors_dir(&home),
    ];
    // A failed spawn leaves reloads manual, as before.
    let _ = std::thread::Builder::new()
        .name("look-sources-watch".into())
        .spawn(move || {
            run(&dirs, DEBOUNCE, &|| {
                if let Some(host) = crate::host::host() {
                    host.sources_changed();
                }
            })
        });
}

/// Returns only when the watcher is gone, so a thread of its own.
fn run(dirs: &[PathBuf], debounce: Duration, on_change: &dyn Fn()) {
    let (tx, rx) = mpsc::channel::<notify::Result<Event>>();
    let mut watcher = match RecommendedWatcher::new(
        move |result| {
            let _ = tx.send(result);
        },
        notify::Config::default(),
    ) {
        Ok(watcher) => watcher,
        Err(err) => {
            eprintln!("[sources] no watcher: {err}");
            return;
        }
    };
    for dir in dirs {
        if let Some(parent) = dir.parent() {
            let _ = watcher.watch(parent, RecursiveMode::NonRecursive);
        }
        arm(&mut watcher, dir);
    }

    let mut dirty_at: Option<Instant> = None;
    loop {
        match rx.recv_timeout(POLL) {
            Ok(Ok(event)) if counts(&event.kind) => {
                for path in &event.paths {
                    if dirs.iter().any(|dir| dir == path) {
                        // The directory itself came or went.
                        arm(&mut watcher, path);
                        dirty_at = Some(Instant::now());
                    } else if concerns(path, dirs) {
                        dirty_at = Some(Instant::now());
                    }
                }
            }
            Ok(_) | Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        if dirty_at.is_some_and(|at| at.elapsed() >= debounce) {
            dirty_at = None;
            on_change();
        }
    }
}

/// Watched once it exists; a watch on a missing path fails, and the parent
/// watch brings it back here when it appears.
fn arm(watcher: &mut RecommendedWatcher, dir: &Path) {
    if dir.is_dir() {
        match watcher.watch(dir, RecursiveMode::NonRecursive) {
            Ok(()) => eprintln!("[sources] watching {}", dir.display()),
            Err(err) => eprintln!("[sources] failed to watch {}: {err}", dir.display()),
        }
    }
}

fn counts(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    )
}

/// A declaration directly in one of the directories. The parent watch also
/// reports the config and `bin/` next to them, which are not ours.
fn concerns(path: &Path, dirs: &[PathBuf]) -> bool {
    dirs.iter().any(|dir| path.parent() == Some(dir.as_path()))
        && path
            .extension()
            .is_some_and(|ext| ext == look_sources::DECLARATION_EXTENSION)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::RecvTimeoutError;

    /// Long enough for inotify to deliver, short enough for a test.
    const TEST_DEBOUNCE: Duration = Duration::from_millis(100);
    const SETTLE: Duration = Duration::from_secs(3);

    /// A directory of this test's own, gone when the test is.
    struct Root(PathBuf);

    impl Root {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir()
                .join(format!("look-sources-watch-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("test root");
            Self(path)
        }
    }

    impl Drop for Root {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_directory_that_appears_later_is_watched_and_its_files_reload() {
        let root = Root::new("appears");
        let sources = root.0.join("sources");
        let connectors = root.0.join("connectors");
        std::fs::create_dir(&connectors).expect("connectors dir");
        let dirs = vec![sources.clone(), connectors.clone()];
        let (tx, rx) = mpsc::channel();
        // The thread outlives the test and ends with the process.
        std::thread::spawn(move || {
            run(&dirs, TEST_DEBOUNCE, &|| {
                let _ = tx.send(());
            })
        });
        // The watches are in place once the thread is up; give it a moment.
        std::thread::sleep(Duration::from_millis(200));

        std::fs::create_dir(&sources).expect("sources dir");
        std::fs::write(sources.join("notes.toml"), "[notes]\n").expect("write");
        assert!(
            rx.recv_timeout(SETTLE).is_ok(),
            "no reload for a new source"
        );
        // One burst, one reload.
        assert_eq!(
            rx.recv_timeout(Duration::from_millis(300)),
            Err(RecvTimeoutError::Timeout)
        );

        std::fs::write(connectors.join("myapp.toml"), "[myapp]\n").expect("write");
        assert!(rx.recv_timeout(SETTLE).is_ok(), "no reload for a manifest");

        std::fs::write(sources.join(".notes.toml.swp"), "").expect("write");
        std::fs::write(root.0.join("config"), "").expect("write");
        assert_eq!(
            rx.recv_timeout(Duration::from_millis(500)),
            Err(RecvTimeoutError::Timeout),
            "a swap file or the config next door reloaded"
        );
    }

    fn dirs() -> Vec<PathBuf> {
        vec![
            PathBuf::from("/home/me/.look/sources"),
            PathBuf::from("/home/me/.look/connectors"),
        ]
    }

    #[test]
    fn a_declaration_in_either_directory_counts() {
        assert!(concerns(
            Path::new("/home/me/.look/sources/notes.toml"),
            &dirs()
        ));
        assert!(concerns(
            Path::new("/home/me/.look/connectors/myapp.toml"),
            &dirs()
        ));
    }

    #[test]
    fn neighbours_swap_files_and_subdirectories_do_not() {
        assert!(!concerns(Path::new("/home/me/.look/config"), &dirs()));
        assert!(!concerns(Path::new("/home/me/.look/bin/notes.sh"), &dirs()));
        assert!(!concerns(
            Path::new("/home/me/.look/sources/.notes.toml.swp"),
            &dirs()
        ));
        assert!(!concerns(
            Path::new("/home/me/.look/sources/deeper/notes.toml"),
            &dirs()
        ));
    }
}
