//! Writing a declaration into place.
//!
//! The counterpart to `load`: that reads the sources directory, this puts
//! something in it. Here rather than in the shell because the directories are
//! decided here (`sources_dir`, `scripts_dir`), and a second answer to "where
//! does a source live" is how the two drift.
//!
//! The contents arrive as text the caller fetched. This module never reaches
//! the network: what it is handed is already bytes, and whether they should
//! have been fetched at all was decided before now.

use std::fs;
use std::path::{Path, PathBuf};

use crate::load::{scripts_dir, sources_dir};

/// What a catalog entry turns into on disk.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Installing {
    /// `(file name, contents)`, written into the sources directory.
    pub declarations: Vec<(String, String)>,
    /// `(file name, contents)`, written into `~/.look/bin` and made
    /// executable. A `run` block pointing at one is useless without it.
    pub scripts: Vec<(String, String)>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Installed {
    pub written: Vec<PathBuf>,
    /// Paths that already held something. Not an error: this is also how an
    /// update lands. Reported so the caller can say what it overwrote, since
    /// one of them may be a file the user had edited.
    pub replaced: Vec<PathBuf>,
    pub problems: Vec<String>,
}

/// Writes `what` into this user's directories, creating them if needed.
///
/// Partial on purpose. A script that cannot be written does not take the
/// declaration down with it, because a half-installed source the user can see
/// and delete beats a silent nothing.
pub fn install(what: &Installing, home: &Path) -> Installed {
    install_into(what, &sources_dir(home), &scripts_dir(home))
}

/// The same, into directories the caller names.
///
/// Split out because `sources_dir` reads `LOOK_SOURCES_DIR`: a test that went
/// through `install` wrote into whatever that pointed at, which on a machine
/// with it set is somebody's real sources directory. Deciding where to write
/// and writing there are now separate, so a test can say where.
pub fn install_into(what: &Installing, sources: &Path, scripts: &Path) -> Installed {
    let mut done = Installed::default();
    write_all(&what.declarations, sources, false, &mut done);
    write_all(&what.scripts, scripts, true, &mut done);
    done
}

fn write_all(files: &[(String, String)], dir: &Path, executable: bool, done: &mut Installed) {
    if files.is_empty() {
        return;
    }
    if let Err(err) = fs::create_dir_all(dir) {
        done.problems.push(format!("{}: {err}", dir.display()));
        return;
    }
    for (name, contents) in files {
        // A catalog entry is remote text, and `../../.zshrc` is a file name
        // until something says otherwise. Nothing downstream re-checks this,
        // so it is checked here, where the path is actually built.
        if !plain_name(name) {
            done.problems
                .push(format!("{name:?} is not a plain file name"));
            continue;
        }
        let path = dir.join(name);
        let existed = path.exists();
        if let Err(err) = fs::write(&path, contents) {
            done.problems.push(format!("{}: {err}", path.display()));
            continue;
        }
        if executable && let Err(err) = make_executable(&path) {
            // The file is there and the user can chmod it themselves, so this
            // is said rather than undone.
            done.problems.push(format!("{}: {err}", path.display()));
        }
        if existed {
            done.replaced.push(path.clone());
        }
        done.written.push(path);
    }
}

/// One path component, nothing clever in it. Rejects a separator, `..`, a
/// leading dot, and anything a shell would have to quote.
fn plain_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && name.len() <= 255
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
}

#[cfg(unix)]
fn make_executable(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms)
}

/// Windows has no execute bit: `PATHEXT` and the extension decide, and both
/// came with the file name.
#[cfg(not(unix))]
fn make_executable(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "look-sources-install-{label}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("temp dir");
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn one(name: &str, contents: &str) -> (String, String) {
        (name.to_string(), contents.to_string())
    }

    /// Directories of this test's own, never the ones a `LOOK_SOURCES_DIR` in
    /// the environment would resolve to. What is being tested is that files
    /// land where they are told, and borrowing the real location to find that
    /// out would write into it.
    fn dirs(home: &Path) -> (PathBuf, PathBuf) {
        (home.join("sources"), home.join("bin"))
    }

    #[test]
    fn a_declaration_and_its_script_land_in_their_own_directories() {
        let home = TempDir::new("both");
        let (sources, scripts) = dirs(&home.0);
        let done = install_into(
            &Installing {
                declarations: vec![one("docker-containers.toml", "[docker]\ndo = [\"true\"]\n")],
                scripts: vec![one("docker-rows", "#!/bin/sh\necho hi\n")],
            },
            &sources,
            &scripts,
        );

        assert!(done.problems.is_empty(), "{:?}", done.problems);
        assert_eq!(done.written.len(), 2, "{:?}", done.written);
        assert!(sources.join("docker-containers.toml").is_file());
        assert!(scripts.join("docker-rows").is_file());
        assert!(done.replaced.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn a_script_arrives_executable_and_a_declaration_does_not() {
        use std::os::unix::fs::PermissionsExt;

        let home = TempDir::new("mode");
        let (sources, scripts) = dirs(&home.0);
        install_into(
            &Installing {
                declarations: vec![one("a.toml", "[a]\ndo = [\"true\"]\n")],
                scripts: vec![one("a-rows", "#!/bin/sh\n")],
            },
            &sources,
            &scripts,
        );

        let mode =
            |path: PathBuf| fs::metadata(path).expect("metadata").permissions().mode() & 0o111;
        assert_ne!(mode(scripts.join("a-rows")), 0);
        assert_eq!(mode(sources.join("a.toml")), 0);
    }

    #[test]
    fn installing_twice_reports_what_it_replaced() {
        let home = TempDir::new("replace");
        let what = Installing {
            declarations: vec![one("a.toml", "[a]\ndo = [\"true\"]\n")],
            ..Installing::default()
        };
        let (sources, scripts) = dirs(&home.0);
        install_into(&what, &sources, &scripts);
        let again = install_into(&what, &sources, &scripts);

        assert_eq!(again.replaced.len(), 1, "{:?}", again.replaced);
        assert_eq!(again.written.len(), 1);
    }

    #[test]
    fn a_file_name_that_is_a_path_is_refused() {
        let home = TempDir::new("escape");
        let (sources, scripts) = dirs(&home.0);
        let done = install_into(
            &Installing {
                declarations: vec![
                    one("../../escaped.toml", "[a]\ndo = [\"true\"]\n"),
                    one("/etc/passwd", "x"),
                    one(".zshrc", "x"),
                    one("fine.toml", "[a]\ndo = [\"true\"]\n"),
                ],
                ..Installing::default()
            },
            &sources,
            &scripts,
        );

        assert_eq!(done.problems.len(), 3, "{:?}", done.problems);
        assert_eq!(done.written.len(), 1, "{:?}", done.written);
        assert!(done.written[0].ends_with("fine.toml"));
        assert!(!home.0.join("../../escaped.toml").exists());
    }

    #[test]
    fn nothing_to_install_makes_no_directories() {
        let home = TempDir::new("empty");
        let (sources, scripts) = dirs(&home.0);
        let done = install_into(&Installing::default(), &sources, &scripts);

        assert!(done.written.is_empty());
        assert!(done.problems.is_empty());
        assert!(!sources.exists());
        assert!(!scripts.exists());
    }
}
