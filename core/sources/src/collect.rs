//! Turning a declared source into rows. Folder and list sources are answered
//! here; a command source needs a process, which is the shell's job, so this
//! module only reports that it cannot run one.

use std::fs;
use std::path::{Path, PathBuf};

use globset::{Glob, GlobSet, GlobSetBuilder};

use crate::def::{Block, Only, Producer, RowFormat};
use crate::rows::{SourceRow, parse_rows};
use crate::run::{RowContext, expand_path};

/// Hard ceiling on rows from one source. A source that hits this is a mistake
/// (a root pointed at the home directory, a runaway script), and the honest
/// answer is a capped list plus a visible truncation, not a frozen launcher.
pub const MAX_ROWS_PER_SOURCE: usize = 2_000;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Collected {
    pub rows: Vec<SourceRow>,
    /// Set when the cap dropped rows, so the caller can say so.
    pub truncated: bool,
    /// Roots that could not be read while others could. An external drive that
    /// is not mounted must not take the rest of the list down with it, but it
    /// must not vanish silently either.
    pub unreadable: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CollectError {
    /// The root, file, or an entry could not be read.
    Io(String),
    /// A `match` or `exclude` pattern is not a valid glob.
    Glob(String),
    /// Command sources are collected by the shell, which owns process
    /// execution. Never a user error.
    NeedsRunner,
    /// The rows were read but could not be parsed in the declared format.
    Malformed(String),
}

impl std::fmt::Display for CollectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(message) => write!(f, "{message}"),
            Self::Glob(message) => write!(f, "{message}"),
            Self::NeedsRunner => write!(f, "command sources are run by the shell"),
            Self::Malformed(message) => write!(f, "{message}"),
        }
    }
}

/// Rows for `def`, with `~` in any declared path resolved against `home`.
pub fn collect(block: &Block, home: &Path) -> Result<Collected, CollectError> {
    match &block.producer {
        Producer::Bundle { .. } => Ok(Collected {
            // A bundle is one row: itself. Its steps are what Enter performs,
            // not something to pick from.
            rows: vec![SourceRow::new(String::new(), block.name.clone())],
            truncated: false,
            unreadable: Vec::new(),
        }),
        Producer::Dir {
            roots,
            depth,
            only,
            include,
            exclude,
        } => {
            let roots: Vec<PathBuf> = roots.iter().map(|root| expand_home(root, home)).collect();
            collect_folders(&roots, *depth, *only, include, exclude)
        }
        Producer::File { path, format } => collect_list(&expand_home(path, home), *format),
        Producer::Run { .. } => Err(CollectError::NeedsRunner),
    }
}

/// Rows for `block` produced against the row a level was launched from. The
/// producer expands against that row, because its own rows do not exist yet. A
/// `run` block still comes back as `NeedsRunner` for the shell to run.
pub fn collect_for_row(
    block: &Block,
    home: &Path,
    row: &RowContext,
) -> Result<Collected, CollectError> {
    match &block.producer {
        Producer::Dir {
            roots,
            depth,
            only,
            include,
            exclude,
        } => {
            let roots: Vec<PathBuf> = roots
                .iter()
                .map(|root| expand_home(&expand_path(root, row), home))
                .collect();
            collect_folders(&roots, *depth, *only, include, exclude)
        }
        Producer::File { path, format } => {
            collect_list(&expand_home(&expand_path(path, row), home), *format)
        }
        Producer::Bundle { .. } | Producer::Run { .. } => collect(block, home),
    }
}

/// Resolves a leading `~` only. A `~` anywhere else is a legitimate character in
/// a path and is left alone.
pub fn expand_home(path: &str, home: &Path) -> PathBuf {
    match path.strip_prefix('~') {
        Some(rest) => home.join(rest.trim_start_matches(['/', '\\'])),
        None => PathBuf::from(path),
    }
}

fn collect_folders(
    roots: &[PathBuf],
    depth: usize,
    only: Only,
    include: &[String],
    exclude: &[String],
) -> Result<Collected, CollectError> {
    let include = build_globs(include)?;
    let exclude = build_globs(exclude)?;

    let mut collected = Collected::default();
    let mut first_error = None;

    for root in roots {
        match collect_folder(root, depth, only, include.as_ref(), exclude.as_ref()) {
            Ok(one) => {
                collected.rows.extend(one.rows);
                collected.truncated |= one.truncated;
            }
            Err(err) => {
                collected.unreadable.push(root.display().to_string());
                first_error.get_or_insert(err);
            }
        }
    }

    // Every root failing is a broken source. Some failing is a mounted-drive
    // problem, and the rows that did resolve are still worth showing.
    if collected.rows.is_empty()
        && let Some(err) = first_error
    {
        return Err(err);
    }

    sort_rows(&mut collected.rows);
    Ok(collected)
}

fn collect_folder(
    root: &Path,
    depth: usize,
    only: Only,
    include: Option<&GlobSet>,
    exclude: Option<&GlobSet>,
) -> Result<Collected, CollectError> {
    let mut rows = Vec::new();
    let mut truncated = false;
    let mut pending = vec![(root.to_path_buf(), 1usize)];

    while let Some((dir, level)) = pending.pop() {
        let entries = fs::read_dir(&dir)
            .map_err(|err| CollectError::Io(format!("{}: {err}", dir.display())))?;

        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if is_hidden(&name) {
                continue;
            }
            let is_dir = entry.file_type().is_ok_and(|kind| kind.is_dir());

            if is_dir && level < depth {
                pending.push((entry.path(), level + 1));
            }

            if !keeps(only, is_dir) {
                continue;
            }
            if let Some(include) = include
                && !include.is_match(&name)
            {
                continue;
            }
            if let Some(exclude) = exclude
                && exclude.is_match(&name)
            {
                continue;
            }

            if rows.len() == MAX_ROWS_PER_SOURCE {
                truncated = true;
                break;
            }

            let path = entry.path();
            let mut row = SourceRow::new(path.to_string_lossy().into_owned(), name);
            row.subtitle = dir.to_str().map(String::from);
            row.path = Some(path.to_string_lossy().into_owned());
            rows.push(row);
        }

        if truncated {
            break;
        }
    }

    Ok(Collected {
        rows,
        truncated,
        unreadable: Vec::new(),
    })
}

/// Directory order is whatever the filesystem hands back, and it changes as
/// entries are added and removed. Rows the user sees must not reshuffle, and
/// with several roots the merged list needs one order, not one per root.
fn sort_rows(rows: &mut [SourceRow]) {
    rows.sort_by(|a, b| {
        a.title
            .to_lowercase()
            .cmp(&b.title.to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });
}

fn collect_list(file: &Path, format: RowFormat) -> Result<Collected, CollectError> {
    let contents =
        fs::read(file).map_err(|err| CollectError::Io(format!("{}: {err}", file.display())))?;
    let text = String::from_utf8_lossy(&contents);

    let (rows, truncated) = parse_rows(&text, MAX_ROWS_PER_SOURCE, format)
        .map_err(|message| CollectError::Malformed(format!("{}: {message}", file.display())))?;

    Ok(Collected {
        rows,
        truncated,
        unreadable: Vec::new(),
    })
}

pub(crate) fn build_globs(patterns: &[String]) -> Result<Option<GlobSet>, CollectError> {
    if patterns.is_empty() {
        return Ok(None);
    }
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob = Glob::new(pattern)
            .map_err(|err| CollectError::Glob(format!("pattern \"{pattern}\": {err}")))?;
        builder.add(glob);
    }
    builder
        .build()
        .map(Some)
        .map_err(|err| CollectError::Glob(err.to_string()))
}

fn keeps(only: Only, is_dir: bool) -> bool {
    match only {
        Only::All => true,
        Only::Dirs => is_dir,
        Only::Files => !is_dir,
    }
}

/// Dotfiles are skipped, matching what `ls` shows and what the user pictured
/// when they pointed a source at a directory.
fn is_hidden(name: &str) -> bool {
    name.starts_with('.')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::def::parse_file;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "look-sources-{label}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("temp dir");
            Self(path)
        }

        fn dir(&self, name: &str) -> PathBuf {
            let path = self.0.join(name);
            fs::create_dir_all(&path).expect("dir");
            path
        }

        fn file(&self, name: &str, contents: &str) -> PathBuf {
            let path = self.0.join(name);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("parent");
            }
            fs::write(&path, contents).expect("file");
            path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn folder_def(body: &str) -> Block {
        let parsed = parse_file(&format!("[projects]\n{body}")).expect("valid file");
        assert!(parsed.problems.is_empty(), "{:?}", parsed.problems);
        parsed.blocks.into_iter().next().expect("one block")
    }

    #[test]
    fn a_folder_source_lists_its_children_in_a_stable_order() {
        let tmp = TempDir::new("folder");
        tmp.dir("zeta");
        tmp.dir("alpha");
        tmp.file("notes.md", "x");

        let def = folder_def(&format!("dir = {:?}\n", tmp.0.to_str().unwrap()));
        let collected = collect(&def, Path::new("/nonexistent")).unwrap();

        let titles: Vec<&str> = collected.rows.iter().map(|r| r.title.as_str()).collect();
        assert_eq!(titles, ["alpha", "notes.md", "zeta"]);
        assert!(!collected.truncated);
        assert!(collected.rows[0].path.is_some());
    }

    #[test]
    fn only_dirs_drops_the_files() {
        let tmp = TempDir::new("only");
        tmp.dir("look");
        tmp.file("readme.md", "x");

        let def = folder_def(&format!(
            "dir = {:?}\nonly = \"dirs\"\n",
            tmp.0.to_str().unwrap()
        ));
        let collected = collect(&def, Path::new("/nonexistent")).unwrap();
        let titles: Vec<&str> = collected.rows.iter().map(|r| r.title.as_str()).collect();
        assert_eq!(titles, ["look"]);
    }

    #[test]
    fn depth_one_stops_at_the_immediate_children() {
        let tmp = TempDir::new("depth");
        tmp.dir("look");
        tmp.file("look/nested.txt", "x");

        let shallow = folder_def(&format!("dir = {:?}\n", tmp.0.to_str().unwrap()));
        let titles: Vec<String> = collect(&shallow, Path::new("/nonexistent"))
            .unwrap()
            .rows
            .into_iter()
            .map(|r| r.title)
            .collect();
        assert_eq!(titles, ["look"]);

        let deep = folder_def(&format!("dir = {:?}\ndepth = 2\n", tmp.0.to_str().unwrap()));
        let titles: Vec<String> = collect(&deep, Path::new("/nonexistent"))
            .unwrap()
            .rows
            .into_iter()
            .map(|r| r.title)
            .collect();
        assert_eq!(titles, ["look", "nested.txt"]);
    }

    #[test]
    fn match_and_exclude_filter_by_name() {
        let tmp = TempDir::new("globs");
        tmp.file("keep.md", "x");
        tmp.file("drop.txt", "x");
        tmp.file("skip.md", "x");

        let def = folder_def(&format!(
            "dir = {:?}\nmatch = [\"*.md\"]\nexclude = [\"skip.*\"]\n",
            tmp.0.to_str().unwrap()
        ));
        let titles: Vec<String> = collect(&def, Path::new("/nonexistent"))
            .unwrap()
            .rows
            .into_iter()
            .map(|r| r.title)
            .collect();
        assert_eq!(titles, ["keep.md"]);
    }

    #[test]
    fn dotfiles_stay_out_of_the_list() {
        let tmp = TempDir::new("hidden");
        tmp.dir(".git");
        tmp.dir("visible");

        let def = folder_def(&format!("dir = {:?}\n", tmp.0.to_str().unwrap()));
        let titles: Vec<String> = collect(&def, Path::new("/nonexistent"))
            .unwrap()
            .rows
            .into_iter()
            .map(|r| r.title)
            .collect();
        assert_eq!(titles, ["visible"]);
    }

    #[test]
    fn several_roots_merge_into_one_sorted_list() {
        let tmp = TempDir::new("roots");
        let dev = tmp.dir("dev");
        let work = tmp.dir("work");
        fs::create_dir_all(dev.join("look")).expect("dir");
        fs::create_dir_all(work.join("atlas")).expect("dir");

        let def = folder_def(&format!(
            "dirs = [{:?}, {:?}]\nonly = \"dirs\"\n",
            dev.to_str().unwrap(),
            work.to_str().unwrap()
        ));
        let collected = collect(&def, Path::new("/nonexistent")).unwrap();

        let titles: Vec<&str> = collected.rows.iter().map(|r| r.title.as_str()).collect();
        assert_eq!(titles, ["atlas", "look"], "one list, one order");
        assert!(collected.unreadable.is_empty());
    }

    #[test]
    fn root_and_roots_are_the_same_key_in_two_shapes() {
        let tmp = TempDir::new("bothroots");
        let dev = tmp.dir("dev");
        let work = tmp.dir("work");
        fs::create_dir_all(dev.join("look")).expect("dir");
        fs::create_dir_all(work.join("atlas")).expect("dir");

        let def = folder_def(&format!(
            "dir = {:?}\ndirs = [{:?}]\nonly = \"dirs\"\n",
            dev.to_str().unwrap(),
            work.to_str().unwrap()
        ));
        let titles: Vec<String> = collect(&def, Path::new("/nonexistent"))
            .unwrap()
            .rows
            .into_iter()
            .map(|r| r.title)
            .collect();
        assert_eq!(titles, ["atlas", "look"]);
    }

    #[test]
    fn one_unreadable_root_does_not_take_the_others_down() {
        // An unmounted drive is a normal Tuesday. The rows that resolved are
        // still worth showing, and the root that did not is still worth saying.
        let tmp = TempDir::new("partial");
        let dev = tmp.dir("dev");
        fs::create_dir_all(dev.join("look")).expect("dir");

        let def = folder_def(&format!(
            "dirs = [{:?}, \"/definitely/not/here\"]\nonly = \"dirs\"\n",
            dev.to_str().unwrap()
        ));
        let collected = collect(&def, Path::new("/nonexistent")).unwrap();

        assert_eq!(collected.rows.len(), 1);
        assert_eq!(collected.unreadable, ["/definitely/not/here"]);
    }

    #[test]
    fn a_missing_root_is_an_error_the_user_can_read() {
        let def = folder_def("dir = \"/definitely/not/here\"\n");
        match collect(&def, Path::new("/nonexistent")) {
            Err(CollectError::Io(message)) => assert!(message.contains("/definitely/not/here")),
            other => panic!("expected an io error, got {other:?}"),
        }
    }

    #[test]
    fn a_list_source_reads_its_rows_from_the_file() {
        let tmp = TempDir::new("list");
        let file = tmp.file("hosts.txt", "web1\tProduction web\tServers\ndb1\n\n");

        let def = folder_def(&format!("file = {:?}\n", file.to_str().unwrap()));
        let collected = collect(&def, Path::new("/nonexistent")).unwrap();

        assert_eq!(collected.rows.len(), 2);
        assert_eq!(collected.rows[0].id, "web1");
        assert_eq!(collected.rows[0].title, "Production web");
        assert_eq!(collected.rows[0].subtitle.as_deref(), Some("Servers"));
        assert_eq!(collected.rows[1].title, "db1");
    }

    #[test]
    fn list_rows_keep_the_order_the_file_gave_them() {
        let tmp = TempDir::new("order");
        let file = tmp.file("ordered.txt", "zeta\nalpha\nmiddle\n");

        let def = folder_def(&format!("file = {:?}\n", file.to_str().unwrap()));
        let titles: Vec<String> = collect(&def, Path::new("/nonexistent"))
            .unwrap()
            .rows
            .into_iter()
            .map(|r| r.title)
            .collect();
        assert_eq!(titles, ["zeta", "alpha", "middle"]);
    }

    #[test]
    fn a_json_list_source_reads_the_fields_the_line_format_cannot_carry() {
        let tmp = TempDir::new("json-list");
        let file = tmp.file(
            "repos.json",
            r#"[{"id":"look","title":"Look","subtitle":"3 uncommitted","path":"/dev/look"}]"#,
        );

        let def = folder_def(&format!(
            "file = {:?}\nformat = \"json\"\n",
            file.to_str().unwrap()
        ));
        let collected = collect(&def, Path::new("/nonexistent")).unwrap();

        assert_eq!(collected.rows.len(), 1);
        assert_eq!(collected.rows[0].subtitle.as_deref(), Some("3 uncommitted"));
        assert_eq!(collected.rows[0].path.as_deref(), Some("/dev/look"));
    }

    #[test]
    fn a_json_list_that_does_not_parse_names_the_file() {
        let tmp = TempDir::new("json-broken");
        let file = tmp.file("broken.json", "{\"id\": }");

        let def = folder_def(&format!(
            "file = {:?}\nformat = \"json\"\n",
            file.to_str().unwrap()
        ));
        match collect(&def, Path::new("/nonexistent")) {
            Err(CollectError::Malformed(message)) => assert!(message.contains("broken.json")),
            other => panic!("expected a malformed error, got {other:?}"),
        }
    }

    #[test]
    fn a_producer_expands_against_the_row_a_level_was_launched_from() {
        let tmp = TempDir::new("level");
        std::fs::create_dir_all(tmp.0.join("animate/src")).expect("child dir");
        std::fs::create_dir_all(tmp.0.join("look/src")).expect("child dir");

        let def = folder_def("dir = \"{path}/src\"\n");
        let row = RowContext {
            path: tmp.0.join("animate").to_string_lossy().into_owned(),
            ..Default::default()
        };

        let collected = collect_for_row(&def, Path::new("/nonexistent"), &row).unwrap();
        assert!(
            collected
                .rows
                .iter()
                .all(|r| r.path.as_deref().unwrap().contains("animate")),
            "the level lists the selected row's folder, not another's"
        );
    }

    #[test]
    fn a_path_placeholder_is_not_shell_quoted_on_its_way_into_the_filesystem() {
        // `expand` quotes, which is right for a command and fatal for a path:
        // the quotes would become part of the name and nothing would be found.
        let tmp = TempDir::new("quoting");
        let parent = tmp.0.join("my project");
        std::fs::create_dir_all(parent.join("inside")).expect("child dir");

        let def = folder_def("dir = \"{path}\"\n");
        let row = RowContext {
            path: parent.to_string_lossy().into_owned(),
            ..Default::default()
        };

        let collected = collect_for_row(&def, Path::new("/nonexistent"), &row).unwrap();
        assert_eq!(collected.rows.len(), 1);
        assert_eq!(collected.rows[0].title, "inside");
    }

    #[test]
    fn a_command_source_is_left_to_the_shell() {
        let def = folder_def("run = \"ls\"\n");
        assert_eq!(
            collect(&def, Path::new("/nonexistent")),
            Err(CollectError::NeedsRunner)
        );
    }

    #[test]
    fn tilde_resolves_against_the_given_home_and_nothing_else() {
        let home = Path::new("/home/u");
        assert_eq!(expand_home("~/dev", home), PathBuf::from("/home/u/dev"));
        assert_eq!(expand_home("~", home), PathBuf::from("/home/u"));
        assert_eq!(expand_home("/etc", home), PathBuf::from("/etc"));
        assert_eq!(
            expand_home("/tmp/a~b", home),
            PathBuf::from("/tmp/a~b"),
            "a tilde inside a path is a normal character"
        );
    }
}
