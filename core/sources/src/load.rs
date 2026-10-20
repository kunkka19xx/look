//! Reading the sources directory.
//!
//! A `.toml` file is a set of blocks. An executable is a `run` block with
//! everything inferred, so a script that prints rows needs no declaration at
//! all. Anything else is ignored with a reason the user can see.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::def::{Block, inferred, parse_file};

/// Overrides where sources are read from, for dotfiles kept elsewhere. Mirrors
/// `LOOK_CONFIG_PATH` in the engine config.
pub const SOURCES_DIR_ENV: &str = "LOOK_SOURCES_DIR";
/// The same, for the directory apps install their manifests into.
pub const CONNECTORS_DIR_ENV: &str = "LOOK_CONNECTORS_DIR";

const SOURCES_DIR_NAME: &str = ".look/sources";
const CONNECTORS_DIR_NAME: &str = ".look/connectors";
const DECLARATION_EXTENSION: &str = "toml";
/// Between a manifest's name and a block's own id.
const NAMESPACE: char = '.';

/// A file the loader could not use, kept so the app can show it instead of
/// failing silently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub file: PathBuf,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Loaded {
    /// Enabled and disabled alike; the caller decides what to do with disabled
    /// ones, and a management screen needs to list them either way.
    pub blocks: Vec<Block>,
    pub problems: Vec<Problem>,
}

/// Where sources live: `$LOOK_SOURCES_DIR`, else `~/.look/sources`.
pub fn sources_dir(home: &Path) -> PathBuf {
    configured_dir(SOURCES_DIR_ENV, home, SOURCES_DIR_NAME)
}

/// Where connector manifests live: `$LOOK_CONNECTORS_DIR`, else
/// `~/.look/connectors`. A second directory rather than more files in the
/// first, because an app writes these and the user writes those: uninstalling
/// is then one file, and neither can take the other's blocks down with it.
pub fn connectors_dir(home: &Path) -> PathBuf {
    configured_dir(CONNECTORS_DIR_ENV, home, CONNECTORS_DIR_NAME)
}

fn configured_dir(env_key: &str, home: &Path, name: &str) -> PathBuf {
    if let Ok(custom) = env::var(env_key) {
        let trimmed = custom.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    home.join(name)
}

/// Loads every manifest in `dir`, each block's id namespaced by the file that
/// declared it: `[notes]` in `myapp.toml` is `myapp.notes`.
///
/// The namespace is what lets an app install blocks without knowing what the
/// user already called theirs, and the separator is a dot because a bare
/// `[myapp.notes]` is a nested table in TOML rather than an id: colliding with
/// a manifest takes deliberate quoting. It follows the file rather than a
/// declared key so uninstalling is still one file, and `then` is rewritten with
/// it so a manifest keeps naming its own blocks plainly.
pub fn load_connectors(dir: &Path) -> Loaded {
    load_from(dir, true)
}

/// Rewrites one manifest's blocks in place: every id gains the namespace, and
/// a `then` naming a sibling gains it too. Done per file, before duplicate ids
/// are dropped, so two manifests may both declare `[notes]`.
///
/// A target the manifest does not declare is dropped here rather than left to
/// resolve later. Kept, it would survive the merge as a bare name and reach
/// whatever the USER happens to call that: an installed app could run a block
/// someone wrote by guessing its id.
fn namespace_blocks(blocks: &mut [Block], manifest: &str, messages: &mut Vec<String>) {
    let own: BTreeMap<String, ()> = blocks.iter().map(|block| (block.id.clone(), ())).collect();
    for block in blocks {
        let id = block.id.clone();
        block.then.retain(|target| {
            if own.contains_key(target.as_str()) {
                return true;
            }
            messages.push(format!(
                "[{id}]: `then` target {target:?} is not declared in this manifest, and a \
                 manifest may only name its own blocks"
            ));
            false
        });
        for target in &mut block.then {
            *target = namespaced(manifest, target);
        }
        block.id = namespaced(manifest, &block.id);
    }
}

fn namespaced(manifest: &str, id: &str) -> String {
    format!("{manifest}{NAMESPACE}{id}")
}

/// What a manifest's name may be, which is what the row cache can hold as part
/// of a file name (`look_engine::index::run_cache`).
fn usable_namespace(stem: &str) -> bool {
    !stem.is_empty() && !stem.contains(['/', '\\']) && !stem.contains("..")
}

impl Loaded {
    /// Takes `other`'s blocks and problems. An id this one already holds is
    /// dropped and reported, so a manifest cannot shadow a block the user
    /// wrote themselves.
    pub fn absorb(&mut self, other: Loaded) {
        let taken: BTreeMap<&str, ()> = self
            .blocks
            .iter()
            .map(|block| (block.id.as_str(), ()))
            .collect();
        let (mut kept, shadowed): (Vec<Block>, Vec<Block>) = other
            .blocks
            .into_iter()
            .partition(|block| !taken.contains_key(block.id.as_str()));

        let mut dropped: BTreeMap<String, PathBuf> = BTreeMap::new();
        for block in shadowed {
            let file = PathBuf::from(block.source_file.unwrap_or_default());
            self.problems.push(Problem {
                file: file.clone(),
                message: format!(
                    "block [{}] is already declared elsewhere; this one is not loaded",
                    block.id
                ),
            });
            dropped.insert(block.id, file);
        }

        // A sibling's `then` still names the block that was dropped, and that
        // name now belongs to someone else's. Reaching it would run a block
        // the manifest never meant, so the reference goes with it.
        for block in &mut kept {
            block.then.retain(|target| {
                let Some(file) = dropped.get(target.as_str()) else {
                    return true;
                };
                self.problems.push(Problem {
                    file: file.clone(),
                    message: format!(
                        "[{}] drops its `then` target {target:?}: that block was not loaded",
                        block.id
                    ),
                });
                false
            });
        }

        self.blocks.extend(kept);
        self.problems.extend(other.problems);
    }
}

/// Loads every block in `dir`. A directory that does not exist is not a problem
/// to report: it is the ordinary state of a user who has not made one.
pub fn load_dir(dir: &Path) -> Loaded {
    load_from(dir, false)
}

fn load_from(dir: &Path, namespace_by_file: bool) -> Loaded {
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

    for path in declarations.values() {
        match fs::read_to_string(path) {
            Ok(contents) => match parse_file(&contents) {
                Ok(parsed) => {
                    let mut blocks = parsed.blocks;
                    let mut messages = parsed.problems;
                    if namespace_by_file {
                        let stem = path
                            .file_stem()
                            .map(|stem| stem.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        // A stem that cannot be half of a usable id takes its
                        // blocks with it: `my..app.notes` is refused by the row
                        // cache, so its rows would fail every refresh forever.
                        if usable_namespace(&stem) {
                            namespace_blocks(&mut blocks, &stem, &mut messages);
                        } else {
                            messages.push(format!(
                                "manifest name {stem:?} cannot namespace a block: no `..`, `/` or `\\`"
                            ));
                            blocks.clear();
                        }
                    }
                    loaded.blocks.extend(blocks.into_iter().map(|mut block| {
                        block.source_file = Some(path.to_string_lossy().into_owned());
                        block
                    }));
                    for message in messages {
                        loaded.problems.push(Problem {
                            file: path.clone(),
                            message,
                        });
                    }
                }
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
        if namespace_by_file {
            // An app ships a manifest. A bare executable has nothing to be
            // namespaced by except itself, which would read as `foo.foo`.
            loaded.problems.push(Problem {
                file: path.clone(),
                message:
                    "an executable here has no manifest to namespace it; declare it in a .toml"
                        .into(),
            });
            continue;
        }
        let mut block = inferred(stem, &executable_command(path));
        block.source_file = Some(path.to_string_lossy().into_owned());
        loaded.blocks.push(block);
    }

    // The id namespaces a block's row ids, its usage history, and its scoped
    // prune. Two blocks answering to one id would quietly share all three.
    let mut taken: BTreeMap<String, ()> = BTreeMap::new();
    loaded.blocks.retain(|block| {
        if taken.insert(block.id.clone(), ()).is_none() {
            return true;
        }
        loaded.problems.push(Problem {
            file: dir.join(&block.id),
            message: format!("duplicate block [{}]: only the first is loaded", block.id),
        });
        false
    });

    // `then` names are checked once every block is known, so a reference to a
    // block declared in another file is fine and only a genuine typo reports.
    let known: BTreeMap<&str, ()> = loaded
        .blocks
        .iter()
        .map(|block| (block.id.as_str(), ()))
        .collect();
    let unknown: Vec<Problem> = loaded
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .then
                .iter()
                .filter(|target| !known.contains_key(target.as_str()))
                .map(|target| Problem {
                    file: block
                        .source_file
                        .as_ref()
                        .map(PathBuf::from)
                        .unwrap_or_else(|| dir.join(&block.id)),
                    message: format!("[{}] then names unknown block \"{target}\"", block.id),
                })
        })
        .collect();
    loaded.problems.extend(unknown);

    for path in ignored {
        // Not an error, but silence here reads as "my block is broken" when the
        // real answer is that the file is neither executable nor a declaration.
        loaded.problems.push(Problem {
            file: path,
            message: "ignored: not executable and not a .toml declaration".into(),
        });
    }

    loaded.blocks.sort_by(|a, b| a.id.cmp(&b.id));
    loaded.problems.sort_by(|a, b| a.file.cmp(&b.file));
    loaded
}

/// The shell text that runs an executable source. Quoted, because the sources
/// directory sits under a home folder the user named, and plenty of them have a
/// space in the name.
#[cfg(not(windows))]
fn executable_command(path: &Path) -> String {
    look_tools::shell_quote(&path.to_string_lossy())
}

/// `cmd` runs what `PATHEXT` covers, and `.ps1` is not on that list: a script
/// there needs PowerShell named in front of it or the step reads as a missing
/// command.
#[cfg(windows)]
fn executable_command(path: &Path) -> String {
    let quoted = look_tools::cmd_quote(&path.to_string_lossy());
    let powershell = path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("ps1"));
    if powershell {
        format!("powershell -NoProfile -ExecutionPolicy Bypass -File {quoted}")
    } else {
        quoted
    }
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
    use crate::def::KEY_RUN;

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
        assert!(loaded.blocks.is_empty());
        assert!(loaded.problems.is_empty());
    }

    #[test]
    fn every_block_in_every_file_is_loaded() {
        let tmp = TempDir::new("blocks");
        tmp.write(
            "mine.toml",
            "[projects]\ndir = \"~/dev\"\n\n[work]\ndo = [\"open -a Slack\"]\n",
        );
        tmp.write("more.toml", "[notes]\ndir = \"~/notes\"\n");

        let loaded = load_dir(&tmp.0);
        let ids: Vec<&str> = loaded.blocks.iter().map(|b| b.id.as_str()).collect();
        assert_eq!(ids, ["notes", "projects", "work"]);
        assert!(loaded.problems.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn an_executable_needs_no_declaration() {
        let tmp = TempDir::new("executable");
        tmp.write_executable("hosts", "#!/bin/sh\necho web1\n");

        let loaded = load_dir(&tmp.0);
        assert_eq!(loaded.blocks.len(), 1);
        assert_eq!(loaded.blocks[0].id, "hosts");
        assert_eq!(loaded.blocks[0].producer.key(), KEY_RUN);
    }

    #[cfg(windows)]
    #[test]
    fn an_executable_needs_no_declaration() {
        let tmp = TempDir::new("executable");
        tmp.write_executable("hosts.cmd", "@echo web1\n");
        tmp.write_executable("repos.ps1", "Write-Output look\n");

        let loaded = load_dir(&tmp.0);
        let ids: Vec<&str> = loaded.blocks.iter().map(|b| b.id.as_str()).collect();
        assert_eq!(ids, ["hosts", "repos"]);
        assert!(loaded.blocks.iter().all(|b| b.producer.key() == KEY_RUN));

        // The path is quoted for `cmd`, and a `.ps1` names the interpreter that
        // can actually run it: `PATHEXT` does not cover one.
        let command = |id: &str| match &loaded.blocks.iter().find(|b| b.id == id).unwrap().producer
        {
            crate::def::Producer::Run { command, .. } => command.clone(),
            other => panic!("{other:?}"),
        };
        assert_eq!(
            command("hosts"),
            format!("\"{}\"", tmp.0.join("hosts.cmd").display())
        );
        assert!(
            command("repos").starts_with("powershell -NoProfile -ExecutionPolicy Bypass -File \""),
            "{}",
            command("repos")
        );
    }

    #[test]
    fn a_duplicate_block_id_loads_once_and_says_so() {
        let tmp = TempDir::new("collide");
        tmp.write("a.toml", "[projects]\ndir = \"~/dev\"\n");
        tmp.write("b.toml", "[projects]\ndir = \"~/other\"\n");

        let loaded = load_dir(&tmp.0);
        assert_eq!(loaded.blocks.len(), 1);
        assert_eq!(loaded.problems.len(), 1);
        assert!(loaded.problems[0].message.contains("duplicate block"));
    }

    #[test]
    fn a_broken_file_is_reported_and_the_others_still_load() {
        let tmp = TempDir::new("broken");
        tmp.write("good.toml", "[good]\ndir = \"~/dev\"\n");
        tmp.write("bad.toml", "[bad]\nname = \"no producer\"\n");

        let loaded = load_dir(&tmp.0);
        assert_eq!(loaded.blocks.len(), 1);
        assert_eq!(loaded.problems.len(), 1);
        assert!(loaded.problems[0].message.contains("[bad]"));
    }

    #[test]
    fn then_may_name_a_block_from_another_file() {
        let tmp = TempDir::new("thencross");
        tmp.write(
            "a.toml",
            "[projects]\ndir = \"~/dev\"\nthen = [\"deploy\"]\n",
        );
        tmp.write("b.toml", "[deploy]\ndo = [\"make -C {path} deploy\"]\n");

        let loaded = load_dir(&tmp.0);
        assert_eq!(loaded.blocks.len(), 2);
        assert!(loaded.problems.is_empty(), "{:?}", loaded.problems);
    }

    #[test]
    fn a_then_typo_is_reported_against_the_file_that_wrote_it() {
        let tmp = TempDir::new("thentypo");
        tmp.write("a.toml", "[projects]\ndir = \"~/dev\"\nthen = [\"edt\"]\n");

        let loaded = load_dir(&tmp.0);
        assert_eq!(loaded.blocks.len(), 1, "the block still loads");
        assert_eq!(loaded.problems.len(), 1);
        assert!(loaded.problems[0].message.contains("unknown block \"edt\""));
        assert!(loaded.problems[0].file.ends_with("a.toml"));
    }

    #[test]
    fn an_unusable_file_says_why_instead_of_vanishing() {
        let tmp = TempDir::new("ignored");
        tmp.write("notes.md", "not a block");

        let loaded = load_dir(&tmp.0);
        assert!(loaded.blocks.is_empty());
        assert!(loaded.problems[0].message.contains("ignored"));
    }

    #[test]
    fn a_manifests_blocks_are_namespaced_by_its_file() {
        let tmp = TempDir::new("manifest");
        tmp.write(
            "myapp.toml",
            "[notes]\ndir = \"~/notes\"\nthen = [\"archive\"]\n\n[archive]\ndo = [\"true\"]\n",
        );

        let loaded = load_connectors(&tmp.0);
        let ids: Vec<&str> = loaded.blocks.iter().map(|b| b.id.as_str()).collect();
        assert_eq!(ids, ["myapp.archive", "myapp.notes"]);

        // `then` keeps naming the block plainly in the file and still resolves.
        let notes = loaded
            .blocks
            .iter()
            .find(|b| b.id == "myapp.notes")
            .unwrap();
        assert_eq!(notes.then, ["myapp.archive"]);
    }

    #[test]
    fn two_manifests_may_each_declare_the_same_block() {
        // Deduping before namespacing dropped the second app's block, although
        // `alpha.notes` and `beta.notes` were never the same id.
        let tmp = TempDir::new("two-manifests");
        tmp.write("alpha.toml", "[notes]\ndo = [\"true\"]\n");
        tmp.write("beta.toml", "[notes]\ndo = [\"true\"]\n");

        let loaded = load_connectors(&tmp.0);
        let ids: Vec<&str> = loaded.blocks.iter().map(|b| b.id.as_str()).collect();
        assert_eq!(ids, ["alpha.notes", "beta.notes"]);
        assert!(loaded.problems.is_empty(), "{:?}", loaded.problems);
    }

    #[test]
    fn a_manifest_may_only_name_its_own_blocks() {
        // Left in place, `deploy` survives the merge as a bare name and reaches
        // whatever the user calls that.
        let tmp = TempDir::new("foreign-then");
        tmp.write(
            "myapp.toml",
            "[open]\ndo = [\"true\"]\nthen = [\"deploy\"]\n",
        );

        let loaded = load_connectors(&tmp.0);
        assert!(loaded.blocks[0].then.is_empty());
        assert!(
            loaded.problems[0]
                .message
                .contains("may only name its own blocks"),
            "{:?}",
            loaded.problems
        );
    }

    #[test]
    fn a_then_target_goes_with_the_block_it_named() {
        // The user's own block takes the id, so the manifest's is dropped. A
        // sibling pointing at it would otherwise reach the user's block.
        let user = TempDir::new("user-shadow");
        user.write("mine.toml", "[\"myapp.notes\"]\ndo = [\"true\"]\n");
        let apps = TempDir::new("app-shadow");
        apps.write(
            "myapp.toml",
            "[notes]\ndo = [\"true\"]\n\n[open]\ndo = [\"true\"]\nthen = [\"notes\"]\n",
        );

        let mut loaded = load_dir(&user.0);
        loaded.absorb(load_connectors(&apps.0));

        let open = loaded.blocks.iter().find(|b| b.id == "myapp.open").unwrap();
        assert!(open.then.is_empty(), "the dangling target is dropped");
        assert!(
            loaded
                .problems
                .iter()
                .any(|p| p.message.contains("drops its `then` target")),
            "{:?}",
            loaded.problems
        );
    }

    #[test]
    fn a_manifest_name_that_cannot_namespace_is_refused() {
        // `my..app.notes` would be rejected by the row cache, so its `run`
        // rows would fail every refresh and never show. Said once instead.
        let tmp = TempDir::new("bad-manifest");
        tmp.write("my..app.toml", "[notes]\ndo = [\"true\"]\n");

        let loaded = load_connectors(&tmp.0);
        assert!(loaded.blocks.is_empty());
        assert!(loaded.problems[0].message.contains("cannot namespace"));
    }

    #[test]
    fn a_manifest_cannot_shadow_a_block_the_user_wrote() {
        // Quoted, because a bare `[myapp.notes]` is a NESTED table in TOML, not
        // an id with a dot in it. That is most of why the dot is the namespace:
        // a user has to go out of their way to collide with one.
        let user = TempDir::new("user-blocks");
        user.write("mine.toml", "[\"myapp.notes\"]\ndo = [\"true\"]\n");
        let apps = TempDir::new("app-blocks");
        apps.write("myapp.toml", "[notes]\ndo = [\"false\"]\n");

        let mut loaded = load_dir(&user.0);
        loaded.absorb(load_connectors(&apps.0));

        assert_eq!(
            loaded.blocks.len(),
            1,
            "the user's own block is the one kept"
        );
        assert_eq!(
            loaded.blocks[0].source_file.as_deref().unwrap_or(""),
            user.0.join("mine.toml").to_string_lossy()
        );
        assert!(loaded.problems[0].message.contains("already declared"));
    }

    #[test]
    fn the_directory_falls_back_to_the_home_relative_default() {
        if env::var(SOURCES_DIR_ENV).is_ok() {
            return;
        }
        assert_eq!(
            sources_dir(Path::new("/home/u")),
            PathBuf::from("/home/u").join(SOURCES_DIR_NAME)
        );
    }
}
