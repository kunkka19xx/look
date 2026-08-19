//! Reading the sources directory.
//!
//! What a file is, is decided by the file itself: an executable is a command
//! source with everything inferred, a `.toml` is a declared source of any kind,
//! and anything else is ignored with a reason the user can see. A `.toml` and an
//! executable sharing a stem are one source: the declaration wins and the script
//! supplies its command, so a script gains actions without moving its logic into
//! a config value.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::def::{SourceDef, inferred, parse};

/// Overrides where sources are read from, for dotfiles kept elsewhere. Mirrors
/// `LOOK_CONFIG_PATH` in the engine config.
pub const SOURCES_DIR_ENV: &str = "LOOK_SOURCES_DIR";

const SOURCES_DIR_NAME: &str = ".look/sources";
const DECLARATION_EXTENSION: &str = "toml";

/// A file the loader could not use, kept so `:source` can show it instead of
/// failing silently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub file: PathBuf,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Loaded {
    /// Enabled and disabled alike; the caller decides what to do with disabled
    /// ones, and `:source` needs to list them either way.
    pub sources: Vec<SourceDef>,
    pub problems: Vec<Problem>,
}

/// Where sources live: `$LOOK_SOURCES_DIR`, else `~/.look/sources`.
pub fn sources_dir(home: &Path) -> PathBuf {
    if let Ok(custom) = env::var(SOURCES_DIR_ENV) {
        let trimmed = custom.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    home.join(SOURCES_DIR_NAME)
}

/// Loads every source in `dir`. A directory that does not exist is not a
/// problem to report: it is the ordinary state of a user who has not made one.
pub fn load_dir(dir: &Path) -> Loaded {
    let mut loaded = Loaded::default();
    let Ok(entries) = fs::read_dir(dir) else {
        return loaded;
    };

    let mut declarations: BTreeMap<String, PathBuf> = BTreeMap::new();
    let mut executables: BTreeMap<String, PathBuf> = BTreeMap::new();
    let mut ignored: Vec<PathBuf> = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            continue;
        }
        let Some(stem) = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
        else {
            continue;
        };

        if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case(DECLARATION_EXTENSION))
        {
            declarations.insert(stem, path);
        } else if is_executable(&path) {
            executables.insert(stem, path);
        } else {
            ignored.push(path);
        }
    }

    for (stem, path) in &declarations {
        let script = executables
            .get(stem)
            .map(|path| path.to_string_lossy().into_owned());
        match fs::read_to_string(path) {
            Ok(contents) => match parse(stem, &contents, script.as_deref()) {
                Ok(def) => loaded.sources.push(def),
                Err(message) => loaded.problems.push(Problem {
                    file: path.clone(),
                    message,
                }),
            },
            Err(err) => loaded.problems.push(Problem {
                file: path.clone(),
                message: err.to_string(),
            }),
        }
    }

    for (stem, path) in &executables {
        if declarations.contains_key(stem) {
            continue;
        }
        loaded.sources.push(inferred(stem, &path.to_string_lossy()));
    }

    for path in ignored {
        // Not an error, but silence here reads as "my source is broken" when the
        // real answer is that the file is neither executable nor a declaration.
        loaded.problems.push(Problem {
            file: path,
            message: "ignored: not executable and not a .toml declaration".into(),
        });
    }

    loaded.sources.sort_by(|a, b| a.id.cmp(&b.id));
    loaded.problems.sort_by(|a, b| a.file.cmp(&b.file));
    loaded
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    const ANY_EXECUTE_BIT: u32 = 0o111;
    fs::metadata(path).is_ok_and(|meta| meta.permissions().mode() & ANY_EXECUTE_BIT != 0)
}

#[cfg(windows)]
fn is_executable(path: &Path) -> bool {
    const EXECUTABLE_EXTENSIONS: [&str; 5] = ["exe", "cmd", "bat", "ps1", "com"];

    path.extension().is_some_and(|extension| {
        let extension = extension.to_string_lossy().to_lowercase();
        EXECUTABLE_EXTENSIONS.contains(&extension.as_str())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::def::SourceSpec;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "look-sources-load-{label}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("temp dir");
            Self(path)
        }

        fn write(&self, name: &str, contents: &str) -> PathBuf {
            let path = self.0.join(name);
            fs::write(&path, contents).expect("write");
            path
        }

        fn write_executable(&self, name: &str, contents: &str) -> PathBuf {
            let path = self.write(name, contents);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&path).expect("metadata").permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&path, perms).expect("chmod");
            }
            path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_missing_directory_is_not_an_error() {
        let loaded = load_dir(Path::new("/definitely/not/here"));
        assert!(loaded.sources.is_empty());
        assert!(loaded.problems.is_empty());
    }

    #[test]
    fn a_declaration_loads_under_its_file_stem() {
        let tmp = TempDir::new("declared");
        tmp.write("projects.toml", "root = \"~/dev\"\nname = \"Projects\"\n");

        let loaded = load_dir(&tmp.0);
        assert_eq!(loaded.sources.len(), 1);
        assert_eq!(loaded.sources[0].id, "projects");
        assert_eq!(loaded.sources[0].name, "Projects");
        assert!(loaded.problems.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn an_executable_alone_needs_no_declaration() {
        let tmp = TempDir::new("executable");
        tmp.write_executable("hosts", "#!/bin/sh\necho web1\n");

        let loaded = load_dir(&tmp.0);
        assert_eq!(loaded.sources.len(), 1);
        assert_eq!(loaded.sources[0].id, "hosts");
        assert_eq!(loaded.sources[0].spec.kind_key(), "command");
    }

    #[cfg(unix)]
    #[test]
    fn a_declaration_beside_a_script_is_one_source_that_runs_the_script() {
        let tmp = TempDir::new("paired");
        let script = tmp.write_executable("hosts", "#!/bin/sh\necho web1\n");
        tmp.write(
            "hosts.toml",
            "name = \"SSH hosts\"\n\n[actions.default]\nrun = \"ssh {id}\"\n",
        );

        let loaded = load_dir(&tmp.0);
        assert_eq!(loaded.sources.len(), 1, "the pair is one source, not two");
        let def = &loaded.sources[0];
        assert_eq!(def.name, "SSH hosts");
        match &def.spec {
            SourceSpec::Command { command, .. } => {
                assert_eq!(command, script.to_str().unwrap());
            }
            other => panic!("expected a command spec, got {other:?}"),
        }
        assert_eq!(def.default_action().unwrap().run, "ssh {id}");
    }

    #[test]
    fn a_broken_declaration_is_reported_and_the_others_still_load() {
        let tmp = TempDir::new("broken");
        tmp.write("good.toml", "root = \"~/dev\"\n");
        tmp.write("bad.toml", "kind = \"folder\"\n");

        let loaded = load_dir(&tmp.0);
        assert_eq!(loaded.sources.len(), 1);
        assert_eq!(loaded.sources[0].id, "good");
        assert_eq!(loaded.problems.len(), 1);
        assert!(loaded.problems[0].message.contains("root"));
    }

    #[test]
    fn an_unusable_file_says_why_instead_of_vanishing() {
        let tmp = TempDir::new("ignored");
        tmp.write("notes.md", "not a source");

        let loaded = load_dir(&tmp.0);
        assert!(loaded.sources.is_empty());
        assert_eq!(loaded.problems.len(), 1);
        assert!(loaded.problems[0].message.contains("ignored"));
    }

    #[test]
    fn the_directory_falls_back_to_the_home_relative_default() {
        // The env override is process-wide, so this asserts the default only.
        if env::var(SOURCES_DIR_ENV).is_ok() {
            return;
        }
        assert_eq!(
            sources_dir(Path::new("/home/u")),
            PathBuf::from("/home/u").join(SOURCES_DIR_NAME)
        );
    }
}
