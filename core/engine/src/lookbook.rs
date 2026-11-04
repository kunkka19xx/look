//! Reading a lookbook entry before and while installing it.
//!
//! The shell fetches: the catalog lives on GitHub and `URLSession` is where
//! every other network call in this app already happens, so nothing here
//! reaches it. What arrives is text, and these are the two questions worth
//! asking about text somebody else wrote:
//!
//! - what would it do on this machine, before any of it is written down
//! - and then, write it down
//!
//! The first is the whole reason the browser can be better than a README. A
//! declaration is checked exactly as an installed one is, by the same code, so
//! the card and the Extend screen cannot disagree about whether `docker` is
//! here.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::sources::Needed;

/// A declaration read without installing it.
#[derive(Serialize)]
pub struct Preview {
    pub blocks: Vec<PreviewBlock>,
    /// Parts of the file that did not parse. The rest still previews.
    pub problems: Vec<String>,
}

/// One block the declaration would add, and what it would want changed.
#[derive(Serialize)]
pub struct PreviewBlock {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    /// `do`, `dir`, `file`, `run`, `connect` or `ask`.
    pub kind: &'static str,
    pub needs: Vec<Needed>,
}

/// What `contents` declares and what it would need here. `Err` only when the
/// text is not a file of blocks at all.
pub fn preview(contents: &str, home: &Path) -> Result<Preview, String> {
    let reviewed = look_sources::review(contents, home)?;

    let blocks = reviewed
        .blocks
        .into_iter()
        .map(|block| PreviewBlock {
            needs: reviewed
                .unready
                .iter()
                .filter(|unready| unready.block == block.id)
                .map(Needed::from)
                .collect(),
            kind: block.producer.key(),
            id: block.id,
            name: block.name,
            icon: block.icon,
        })
        .collect();

    Ok(Preview {
        blocks,
        problems: reviewed.problems,
    })
}

/// Programs from a catalog entry's `requires` that are not installed here.
///
/// A requirement arrives as a bare name with no block behind it, usually
/// because the thing that runs it is inside a helper script. Answered with the
/// same probe the block check uses, so one screen cannot claim `jq` is missing
/// while the other says it is here.
pub fn missing_programs(programs: &[String]) -> Vec<String> {
    programs
        .iter()
        .filter(|program| !look_sources::installed(program))
        .cloned()
        .collect()
}

/// Names to look for before writing anything.
#[derive(Deserialize)]
pub struct PresentRequest {
    #[serde(default)]
    pub declarations: Vec<String>,
    #[serde(default)]
    pub scripts: Vec<String>,
}

/// Which of them are already there.
#[derive(Serialize)]
pub struct Present {
    pub declarations: Vec<String>,
    pub scripts: Vec<String>,
}

/// What an install would replace, asked before it writes.
///
/// The shell could infer the declarations from the list of what is installed,
/// but not the scripts: those live in `~/.look/bin`, which no screen shows.
/// Asked here so one answer covers both directories and the shell never has
/// to rebuild where a source lives.
pub fn present(request: &PresentRequest, home: &Path) -> Present {
    let found = |dir: std::path::PathBuf, names: &[String]| -> Vec<String> {
        names
            .iter()
            .filter(|name| dir.join(name).exists())
            .cloned()
            .collect()
    };
    Present {
        declarations: found(look_sources::sources_dir(home), &request.declarations),
        scripts: found(look_sources::scripts_dir(home), &request.scripts),
    }
}

/// What the shell fetched, ready to write.
#[derive(Deserialize)]
pub struct InstallRequest {
    #[serde(default)]
    pub declarations: Vec<NamedFile>,
    /// Helper scripts. Without these a `run` block that points at one installs
    /// into a row that does nothing.
    #[serde(default)]
    pub scripts: Vec<NamedFile>,
}

#[derive(Deserialize)]
pub struct NamedFile {
    pub name: String,
    pub contents: String,
}

#[derive(Serialize)]
pub struct InstallOutcome {
    pub written: Vec<String>,
    /// Paths that already held something, so the shell can say what it
    /// overwrote. One of them may be a file the user had edited.
    pub replaced: Vec<String>,
    pub problems: Vec<String>,
}

/// Writes an entry into this user's directories.
pub fn install(request: &InstallRequest, home: &Path) -> InstallOutcome {
    let named = |files: &[NamedFile]| {
        files
            .iter()
            .map(|file| (file.name.clone(), file.contents.clone()))
            .collect()
    };
    let done = look_sources::install(
        &look_sources::Installing {
            declarations: named(&request.declarations),
            scripts: named(&request.scripts),
        },
        home,
    );

    let shown = |paths: Vec<std::path::PathBuf>| {
        paths
            .into_iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect()
    };
    InstallOutcome {
        written: shown(done.written),
        replaced: shown(done.replaced),
        problems: done.problems,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A temp home is not enough on its own: `sources_dir` consults
    /// `LOOK_SOURCES_DIR`, so a test that writes has to say where, or it
    /// writes into whatever that points at. Process-wide, so one at a time.
    static GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Set for the length of a test and removed after.
    struct SourcesDir;

    impl SourcesDir {
        fn pointed_at(dir: &Path) -> Self {
            unsafe { std::env::set_var(look_sources::SOURCES_DIR_ENV, dir) };
            Self
        }
    }

    impl Drop for SourcesDir {
        fn drop(&mut self) {
            unsafe { std::env::remove_var(look_sources::SOURCES_DIR_ENV) };
        }
    }

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "look-engine-lookbook-{label}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("temp dir");
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_preview_says_what_each_block_would_need_here() {
        let home = TempDir::new("preview");
        let found = preview(
            "[projects]\nname = \"Projects\"\ndir = \"~/dev\"\nedit = \"look-no-editor {path}\"\n",
            &home.0,
        )
        .expect("parses");

        assert!(found.problems.is_empty(), "{:?}", found.problems);
        assert_eq!(found.blocks.len(), 1);
        let block = &found.blocks[0];
        assert_eq!(block.id, "projects");
        assert_eq!(block.name, "Projects");
        assert_eq!(block.kind, "dir");
        assert_eq!(block.needs.len(), 2, "{:?}", block.needs);
    }

    #[test]
    fn every_need_is_attributed_to_the_block_that_declared_it() {
        let home = TempDir::new("attribute");
        let found = preview(
            "[a]\ndir = \"~/look-not-here\"\n\n[b]\ndo = [\"true\"]\n",
            &home.0,
        )
        .expect("parses");

        let needs = |id: &str| {
            found
                .blocks
                .iter()
                .find(|block| block.id == id)
                .map(|block| block.needs.len())
                .expect("the block")
        };
        assert_eq!(needs("a"), 1);
        assert_eq!(needs("b"), 0);
    }

    #[test]
    fn a_requirement_with_no_block_behind_it_is_still_answered() {
        assert!(missing_programs(&["sh".to_string()]).is_empty());
        assert_eq!(
            missing_programs(&["sh".to_string(), "look-no-such-program".to_string()]),
            ["look-no-such-program"]
        );
    }

    #[test]
    fn an_install_reports_what_it_wrote_and_refuses_a_path_for_a_name() {
        let _guard = GUARD.lock().unwrap_or_else(|err| err.into_inner());
        let home = TempDir::new("install");
        let _sources = SourcesDir::pointed_at(&home.0.join("sources"));
        let request: InstallRequest = serde_json::from_str(
            r#"{
                "declarations": [{"name": "a.toml", "contents": "[a]\ndo = [\"true\"]\n"}],
                "scripts": [{"name": "../escape", "contents": "x"}]
            }"#,
        )
        .expect("parses");

        let done = install(&request, &home.0);
        assert_eq!(done.written.len(), 1, "{:?}", done.written);
        assert!(done.written[0].ends_with("a.toml"));
        assert_eq!(done.problems.len(), 1, "{:?}", done.problems);
    }

    #[test]
    fn what_an_install_would_replace_covers_both_directories() {
        let _guard = GUARD.lock().unwrap_or_else(|err| err.into_inner());
        let home = TempDir::new("present");
        let _sources = SourcesDir::pointed_at(&home.0.join("sources"));

        let request = InstallRequest {
            declarations: vec![NamedFile {
                name: "a.toml".to_string(),
                contents: "[a]\ndo = [\"true\"]\n".to_string(),
            }],
            scripts: vec![NamedFile {
                name: "a-rows".to_string(),
                contents: "#!/bin/sh\n".to_string(),
            }],
        };
        let asking = PresentRequest {
            declarations: vec!["a.toml".to_string(), "never.toml".to_string()],
            scripts: vec!["a-rows".to_string(), "never-rows".to_string()],
        };

        let before = present(&asking, &home.0);
        assert!(before.declarations.is_empty(), "{:?}", before.declarations);
        assert!(before.scripts.is_empty(), "{:?}", before.scripts);

        assert!(install(&request, &home.0).problems.is_empty());

        let after = present(&asking, &home.0);
        assert_eq!(after.declarations, ["a.toml"]);
        // The half the shell cannot see for itself: no screen lists ~/.look/bin.
        assert_eq!(after.scripts, ["a-rows"]);
    }

    #[test]
    fn what_was_previewed_is_what_the_extend_screen_then_says() {
        let _guard = GUARD.lock().unwrap_or_else(|err| err.into_inner());
        let home = TempDir::new("agree");
        let _sources = SourcesDir::pointed_at(&home.0.join("sources"));
        let toml = "[projects]\ndir = \"~/look-not-here\"\n";

        let before = preview(toml, &home.0).expect("parses");
        let request = InstallRequest {
            declarations: vec![NamedFile {
                name: "projects.toml".to_string(),
                contents: toml.to_string(),
            }],
            scripts: Vec::new(),
        };
        let done = install(&request, &home.0);
        assert!(done.problems.is_empty(), "{:?}", done.problems);

        // The same declaration, now read off disk by the loader rather than
        // from the text. The browser promised this and the screen has to keep
        // it, so the two are compared rather than assumed.
        let after = look_sources::load_dir(&look_sources::sources_dir(&home.0));
        let checked: Vec<String> = after
            .blocks
            .iter()
            .flat_map(|block| look_sources::check(block, &home.0))
            .map(|unready| unready.message())
            .collect();
        let promised: Vec<String> = before.blocks[0]
            .needs
            .iter()
            .map(|need| need.message.clone())
            .collect();
        assert_eq!(promised, checked);
        assert!(!promised.is_empty());
    }
}
