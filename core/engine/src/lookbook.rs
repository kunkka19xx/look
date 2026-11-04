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
    /// What the example asks the installer for, in the order declared.
    pub fields: Vec<Field>,
    /// Parts of the file that did not parse. The rest still previews.
    pub problems: Vec<String>,
}

/// One value the example asks for, as a form needs it.
///
/// Declared rather than inferred, which is what lets it reach a value the
/// check cannot see: `vault=look` is a working string that happens to be
/// somebody else's vault.
#[derive(Serialize)]
pub struct Field {
    pub label: String,
    /// `folder`, `file`, `text` or `choice`, deciding the control shown.
    pub kind: &'static str,
    pub note: Option<String>,
    pub options: Vec<String>,
    /// What the author wrote, which the control starts on.
    pub current: Option<String>,
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
    let (declared, mut problems) = look_sources::parse_setup(contents)?;
    problems.extend(reviewed.problems);

    let fields = declared
        .iter()
        .map(|field| Field {
            label: field.label.clone(),
            kind: field.kind.name(),
            note: field.note.clone(),
            options: field.options.clone(),
            current: look_sources::current(contents, field),
        })
        .collect();

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
        fields,
        problems,
    })
}

/// The same, for a declaration with its answers already filled in.
///
/// The readiness lines describe the file as written, and the whole point of
/// the form is to stop them being true. Previewing the filled text is how the
/// sheet can show what Install would actually land rather than what the
/// author shipped.
pub fn preview_filled(file: &NamedFile, home: &Path) -> Result<Preview, String> {
    let contents = answered(file).map_err(|problems| problems.join("\n"))?;
    preview(&contents, home)
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
    /// Answers to this file's `[[setup]]` fields, in the order declared. An
    /// entry left out, or null, keeps what the author wrote.
    #[serde(default)]
    pub answers: Vec<Option<String>>,
}

#[derive(Serialize)]
pub struct InstallOutcome {
    pub written: Vec<String>,
    /// Paths that already held something, so the shell can say what it
    /// overwrote. One of them may be a file the user had edited.
    pub replaced: Vec<String>,
    pub problems: Vec<String>,
}

/// Writes an entry into this user's directories, with its fields filled in.
///
/// Filling happens here rather than in the shell so the shell never builds
/// TOML: it collects answers and the engine decides which bytes they replace.
pub fn install(request: &InstallRequest, home: &Path) -> InstallOutcome {
    let mut refused = Vec::new();
    let mut declarations = Vec::new();
    for file in &request.declarations {
        declarations.push((
            file.name.clone(),
            match answered(file) {
                Ok(contents) => contents,
                Err(problems) => {
                    refused.extend(problems);
                    continue;
                }
            },
        ));
    }
    // A field that could not be applied means the file would land saying
    // something other than what the form showed, so none of it is written.
    if !refused.is_empty() {
        return InstallOutcome {
            written: Vec::new(),
            replaced: Vec::new(),
            problems: refused,
        };
    }

    let named = |files: &[NamedFile]| {
        files
            .iter()
            .map(|file| (file.name.clone(), file.contents.clone()))
            .collect()
    };
    let done = look_sources::install(
        &look_sources::Installing {
            declarations,
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

/// One declaration with its answers applied, or why they could not be.
fn answered(file: &NamedFile) -> Result<String, Vec<String>> {
    if file.answers.iter().all(Option::is_none) {
        return Ok(file.contents.clone());
    }
    let (fields, problems) = look_sources::parse_setup(&file.contents).map_err(|err| vec![err])?;
    if !problems.is_empty() {
        return Err(problems);
    }
    look_sources::fill(&file.contents, &fields, &file.answers)
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

    const ASKING: &str = r#"
# A comment that has to survive being installed.
[projects]
name = "Projects"
dir  = "~/dev"
edit = "code {path}"

[[setup]]
label = "Your projects folder"
type  = "folder"
block = "projects"
key   = "dir"

[[setup]]
label = "Editor command"
block = "projects"
key   = "edit"
match = "code"
"#;

    #[test]
    fn a_preview_carries_the_fields_the_example_asks_for() {
        let home = TempDir::new("fields");
        let found = preview(ASKING, &home.0).expect("parses");

        assert_eq!(found.fields.len(), 2);
        assert_eq!(found.fields[0].label, "Your projects folder");
        assert_eq!(found.fields[0].kind, "folder");
        assert_eq!(found.fields[0].current.as_deref(), Some("~/dev"));
        // Defaulted, because the declaration left `type` out.
        assert_eq!(found.fields[1].kind, "text");
        assert_eq!(found.fields[1].current.as_deref(), Some("code"));
        // The fields are not blocks.
        assert_eq!(found.blocks.len(), 1);
    }

    #[test]
    fn answers_land_in_the_installed_file_and_the_comments_do_too() {
        let _guard = GUARD.lock().unwrap_or_else(|err| err.into_inner());
        let home = TempDir::new("answers");
        let _sources = SourcesDir::pointed_at(&home.0.join("sources"));
        let request = InstallRequest {
            declarations: vec![NamedFile {
                name: "projects.toml".to_string(),
                contents: ASKING.to_string(),
                answers: vec![
                    Some(home.0.to_string_lossy().into_owned()),
                    Some("hx".into()),
                ],
            }],
            scripts: Vec::new(),
        };
        let done = install(&request, &home.0);
        assert!(done.problems.is_empty(), "{:?}", done.problems);

        let written = std::fs::read_to_string(&done.written[0]).expect("reads back");
        assert!(
            written.contains(&format!("dir  = \"{}\"", home.0.display())),
            "{written}"
        );
        assert!(written.contains(r#"edit = "hx {path}""#), "{written}");
        assert!(written.contains("# A comment that has to survive being installed."));

        // And the point of all of it: answered, the block no longer reports
        // the thing the card warned about.
        let after = look_sources::load_dir(&look_sources::sources_dir(&home.0));
        let still: Vec<String> = after
            .blocks
            .iter()
            .flat_map(|block| look_sources::check(block, &home.0))
            .map(|unready| unready.message())
            .collect();
        assert!(
            !still.iter().any(|line| line.contains("~/dev")),
            "the folder was filled in: {still:?}"
        );
    }

    #[test]
    fn a_filled_preview_no_longer_reports_what_the_answer_fixed() {
        let home = TempDir::new("filled");
        let folder = home.0.join("projects");
        std::fs::create_dir_all(&folder).expect("folder");

        let file = NamedFile {
            name: "projects.toml".to_string(),
            contents: ASKING.to_string(),
            answers: vec![Some(folder.to_string_lossy().into_owned()), None],
        };

        let before = preview(ASKING, &home.0).expect("parses");
        assert!(
            before.blocks[0]
                .needs
                .iter()
                .any(|need| need.value == "~/dev"),
            "{:?}",
            before.blocks[0].needs
        );

        // The whole point of the form: the line the sheet showed is answered.
        let after = preview_filled(&file, &home.0).expect("parses");
        assert!(
            !after.blocks[0]
                .needs
                .iter()
                .any(|need| need.value == "~/dev"),
            "{:?}",
            after.blocks[0].needs
        );
    }

    #[test]
    fn nothing_is_written_when_an_answer_cannot_be_applied() {
        let _guard = GUARD.lock().unwrap_or_else(|err| err.into_inner());
        let home = TempDir::new("refused");
        let _sources = SourcesDir::pointed_at(&home.0.join("sources"));
        let request = InstallRequest {
            declarations: vec![NamedFile {
                name: "a.toml".to_string(),
                contents: "[a]\ndo = [\"cp x x\"]\n\n[[setup]]\nlabel = \"Which\"\nblock = \"a\"\nkey = \"do\"\nmatch = \"x\"\n".to_string(),
                answers: vec![Some("y".into())],
            }],
            scripts: Vec::new(),
        };
        let done = install(&request, &home.0);

        assert!(done.written.is_empty(), "{:?}", done.written);
        assert_eq!(done.problems.len(), 1, "{:?}", done.problems);
        assert!(
            done.problems[0].contains("exactly once"),
            "{:?}",
            done.problems
        );
    }

    #[test]
    fn a_refused_script_still_leaves_the_declaration_written() {
        let _guard = GUARD.lock().unwrap_or_else(|err| err.into_inner());
        let home = TempDir::new("partial");
        let _sources = SourcesDir::pointed_at(&home.0.join("sources"));

        let done = install(
            &InstallRequest {
                declarations: vec![NamedFile {
                    name: "a.toml".to_string(),
                    contents: "[a]\ndo = [\"true\"]\n".to_string(),
                    answers: Vec::new(),
                }],
                scripts: vec![NamedFile {
                    name: "../escape".to_string(),
                    contents: "x".to_string(),
                    answers: Vec::new(),
                }],
            },
            &home.0,
        );

        // Partial on purpose, and the shell has to know: something is on
        // disk, so the screens behind it are out of date even though this
        // install also failed.
        assert_eq!(done.written.len(), 1, "{:?}", done.written);
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
                answers: Vec::new(),
            }],
            scripts: vec![NamedFile {
                name: "a-rows".to_string(),
                contents: "#!/bin/sh\n".to_string(),
                answers: Vec::new(),
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
                answers: Vec::new(),
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
