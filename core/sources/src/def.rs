//! What a user declares. Parsing only: nothing here reads a directory or runs a
//! command.
//!
//! A file is a set of blocks. Every block has a `name` you can type, and one
//! producer key that decides what it is:
//!
//! - `do`   a bundle. One row; Enter performs its steps.
//! - `dir`  a list of the children of one or more directories.
//! - `file` a list read from a text file.
//! - `run`  a list read from a command's stdout.
//!
//! The producer key is the only thing that distinguishes them, so there is no
//! `kind` to remember and no way for the two to disagree.

use std::collections::BTreeMap;
use std::time::Duration;

use look_tools::Action;
use serde::Deserialize;

use crate::run::{PLACEHOLDER_PARENT, PLACEHOLDERS};

/// Immediate children only. Deep walks belong to the file index, not to a
/// block the user declared to get one short list.
pub const DEFAULT_FOLDER_DEPTH: usize = 1;

const SECONDS_PER_MINUTE: u64 = 60;
const SECONDS_PER_HOUR: u64 = 60 * SECONDS_PER_MINUTE;
const SECONDS_PER_DAY: u64 = 24 * SECONDS_PER_HOUR;

/// Which entries a `dir` block keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Only {
    Dirs,
    Files,
    #[default]
    All,
}

/// How the rows a `file` or `run` block emits are encoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RowFormat {
    #[default]
    Lines,
    Json,
}

/// Parses a duration such as `30s`, `5m`, `1h`, `2d`.
pub fn parse_duration(value: &str) -> Result<Duration, String> {
    let trimmed = value.trim();
    let split = trimmed
        .find(|c: char| !c.is_ascii_digit())
        .ok_or_else(|| format!("\"{trimmed}\" has no unit, try \"5s\""))?;
    let (digits, unit) = trimmed.split_at(split);
    let amount: u64 = digits
        .parse()
        .map_err(|_| format!("\"{trimmed}\" is not a duration"))?;
    let seconds = match unit {
        "s" => amount,
        "m" => amount * SECONDS_PER_MINUTE,
        "h" => amount * SECONDS_PER_HOUR,
        "d" => amount * SECONDS_PER_DAY,
        other => return Err(format!("unit \"{other}\" is not one of s, m, h, d")),
    };
    if seconds == 0 {
        return Err("a duration must be greater than zero".into());
    }
    Ok(Duration::from_secs(seconds))
}

/// What a block produces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Producer {
    /// One row. Enter performs every step in order.
    Bundle { steps: Vec<String> },
    /// Rows from the children of one or more directories.
    Dir {
        roots: Vec<String>,
        depth: usize,
        only: Only,
        include: Vec<String>,
        exclude: Vec<String>,
    },
    /// Rows from a text file.
    File { path: String, format: RowFormat },
    /// Rows from a command's stdout.
    Run {
        command: String,
        cwd: Option<String>,
        timeout: Option<Duration>,
        format: RowFormat,
    },
}

impl Producer {
    pub fn key(&self) -> &'static str {
        match self {
            Self::Bundle { .. } => KEY_DO,
            Self::Dir { .. } => KEY_DIR,
            Self::File { .. } => KEY_FILE,
            Self::Run { .. } => KEY_RUN,
        }
    }
}

pub const KEY_DO: &str = "do";
pub const KEY_DIR: &str = "dir";
pub const KEY_FILE: &str = "file";
pub const KEY_RUN: &str = "run";

/// The standard things a user does to a row. Each has one key across the whole
/// app, so Cmd+E edits whatever the row is and wherever it came from. A block
/// only names the ones whose command differs from the global default.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Verbs {
    pub open: Option<String>,
    pub edit: Option<String>,
    pub terminal: Option<String>,
    pub reveal: Option<String>,
}

impl Verbs {
    pub const OPEN: &'static str = "open";
    pub const EDIT: &'static str = "edit";
    pub const TERMINAL: &'static str = "terminal";
    pub const REVEAL: &'static str = "reveal";

    pub fn is_empty(&self) -> bool {
        self.open.is_none()
            && self.edit.is_none()
            && self.terminal.is_none()
            && self.reveal.is_none()
    }

    /// The command this block declares for `action`, if any. The one mapping
    /// from a tool action to a block key, so `Action::Edit` cannot come to mean
    /// `terminal` in one caller and `edit` in another.
    pub fn for_action(&self, action: Action) -> Option<&str> {
        match action {
            Action::Edit => self.edit.as_deref(),
            Action::TerminalHere => self.terminal.as_deref(),
            Action::Reveal => self.reveal.as_deref(),
        }
    }

    /// Declared verbs in a stable order, for the action menu.
    pub fn declared(&self) -> Vec<(&'static str, &str)> {
        [
            (Self::OPEN, self.open.as_deref()),
            (Self::EDIT, self.edit.as_deref()),
            (Self::TERMINAL, self.terminal.as_deref()),
            (Self::REVEAL, self.reveal.as_deref()),
        ]
        .into_iter()
        .filter_map(|(name, command)| command.map(|command| (name, command)))
        .collect()
    }
}

/// One declared block. `id` is the table header, which is what row ids and
/// scoped pruning key on, so it never comes from the block's own contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub id: String,
    /// What the user types to see this block's rows. Defaults to the id.
    pub name: String,
    pub producer: Producer,
    pub verbs: Verbs,
    /// Other blocks reachable from a row of this one. A target that performs
    /// steps (`do`) is an action; a target that produces rows is a drill-down.
    /// The target's own producer decides which, so `then` stays a plain list of
    /// names with no mode to declare.
    pub then: Vec<String>,
    pub aliases: Vec<String>,
    pub bias: i64,
    pub icon: Option<String>,
    pub enabled: bool,
    pub preview: Option<String>,
    /// A yes/no question asked before this block performs anything. Destructive
    /// steps get one, because a launcher makes Enter on the wrong row cheap.
    pub confirm: Option<String>,
    /// The file this block was declared in, so the app can show it and reveal
    /// it. Set by the loader; parsing a string alone has no file to name.
    pub source_file: Option<String>,
    /// Keys the parser did not recognize. Reported, never fatal, so a file
    /// written for a newer version still loads.
    pub unknown_keys: Vec<String>,
}

impl Block {
    pub fn is_bundle(&self) -> bool {
        matches!(self.producer, Producer::Bundle { .. })
    }

    /// True when this block only makes sense against a selected row, because
    /// what it produces refers to one. Such a block is reachable through another
    /// block's `then`, never as a top-level row: running `make -C {path} deploy`
    /// with nothing selected would substitute an empty path.
    pub fn needs_row(&self) -> bool {
        self.producer_text().iter().any(|text| {
            text.contains(PLACEHOLDER_PARENT)
                || PLACEHOLDERS
                    .iter()
                    .any(|placeholder| text.contains(placeholder))
        })
    }

    /// Everything the producer needs before it can make a row. `cwd` counts:
    /// it is where the command runs, so a command naming no row from a
    /// directory that does is still a block about one selected row.
    fn producer_text(&self) -> Vec<&str> {
        match &self.producer {
            Producer::Bundle { steps } => steps.iter().map(String::as_str).collect(),
            Producer::Dir { roots, .. } => roots.iter().map(String::as_str).collect(),
            Producer::File { path, .. } => vec![path.as_str()],
            Producer::Run { command, cwd, .. } => {
                let mut text = vec![command.as_str()];
                text.extend(cwd.as_deref());
                text
            }
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct RawBlock {
    name: Option<String>,
    aliases: Option<Vec<String>>,
    bias: Option<i64>,
    icon: Option<String>,
    enabled: Option<bool>,
    preview: Option<String>,
    confirm: Option<String>,

    #[serde(rename = "do")]
    steps: Option<Vec<String>>,
    then: Option<Vec<String>>,

    dir: Option<String>,
    dirs: Option<Vec<String>>,
    depth: Option<usize>,
    only: Option<Only>,
    #[serde(rename = "match")]
    include: Option<Vec<String>>,
    exclude: Option<Vec<String>>,

    file: Option<String>,

    run: Option<String>,
    cwd: Option<String>,
    timeout: Option<String>,
    format: Option<RowFormat>,

    open: Option<String>,
    edit: Option<String>,
    terminal: Option<String>,
    reveal: Option<String>,

    #[serde(flatten)]
    extra: BTreeMap<String, toml::Value>,
}

/// Everything one file declared. A block that does not validate is reported
/// without taking its neighbours down.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedFile {
    pub blocks: Vec<Block>,
    pub problems: Vec<String>,
}

/// Parse a file of blocks. Every top-level table is one block, keyed by its
/// header.
pub fn parse_file(contents: &str) -> Result<ParsedFile, String> {
    let value: toml::Value = toml::from_str(contents).map_err(|err| err.to_string())?;
    let table = value
        .as_table()
        .ok_or("a source file must be a table of blocks")?;

    let mut parsed = ParsedFile::default();
    for (id, block) in table {
        let Some(block) = block.as_table() else {
            parsed.problems.push(format!(
                "[{id}] must be a table, like [{id}]\\nname = \"…\""
            ));
            continue;
        };
        let raw: RawBlock = match toml::Value::Table(block.clone()).try_into() {
            Ok(raw) => raw,
            Err(err) => {
                parsed.problems.push(format!("[{id}]: {err}"));
                continue;
            }
        };
        match build(id, raw) {
            Ok(block) => parsed.blocks.push(block),
            Err(message) => parsed.problems.push(format!("[{id}]: {message}")),
        }
    }
    Ok(parsed)
}

/// A block with no declaration at all: an executable dropped in the directory,
/// where the file itself is the command and everything else is a default.
pub fn inferred(id: &str, command: &str) -> Block {
    build(
        id,
        RawBlock {
            run: Some(command.to_string()),
            ..Default::default()
        },
    )
    .expect("an inferred run block has every required field")
}

fn build(id: &str, raw: RawBlock) -> Result<Block, String> {
    // Row ids are `src:<block>:<row>` and split on the first colon, so a block
    // id carrying one would resolve to a different block than it names. TOML
    // allows it through a quoted header, so it has to be refused here.
    if id.contains(':') {
        return Err("a block id cannot contain \":\"".into());
    }
    let producer = producer_from(&raw)?;

    let verbs = Verbs {
        open: raw.open,
        edit: raw.edit,
        terminal: raw.terminal,
        reveal: raw.reveal,
    };
    // A bundle IS the action, so a verb on it would be a second, hidden meaning
    // for Enter.
    if matches!(producer, Producer::Bundle { .. }) && !verbs.is_empty() {
        return Err(format!(
            "a `{KEY_DO}` block performs its own steps, so it cannot also declare open/edit/terminal/reveal"
        ));
    }

    Ok(Block {
        id: id.to_string(),
        name: raw.name.unwrap_or_else(|| id.to_string()),
        producer,
        verbs,
        then: raw
            .then
            .unwrap_or_default()
            .into_iter()
            .filter(|name| !name.trim().is_empty())
            .collect(),
        aliases: raw.aliases.unwrap_or_default(),
        bias: raw.bias.unwrap_or_default(),
        icon: raw.icon,
        enabled: raw.enabled.unwrap_or(true),
        preview: raw.preview,
        confirm: raw.confirm.filter(|question| !question.trim().is_empty()),
        source_file: None,
        unknown_keys: raw.extra.keys().cloned().collect(),
    })
}

fn producer_from(raw: &RawBlock) -> Result<Producer, String> {
    let declared: Vec<&str> = [
        (KEY_DO, raw.steps.is_some()),
        (KEY_DIR, raw.dir.is_some() || raw.dirs.is_some()),
        (KEY_FILE, raw.file.is_some()),
        (KEY_RUN, raw.run.is_some()),
    ]
    .into_iter()
    .filter_map(|(key, present)| present.then_some(key))
    .collect();

    match declared.as_slice() {
        [] => Err(format!(
            "needs one of {KEY_DO}, {KEY_DIR}, {KEY_FILE}, or {KEY_RUN}"
        )),
        [KEY_DO] => {
            let steps: Vec<String> = raw
                .steps
                .clone()
                .unwrap_or_default()
                .into_iter()
                .filter(|step| !step.trim().is_empty())
                .collect();
            if steps.is_empty() {
                return Err(format!("`{KEY_DO}` needs at least one step"));
            }
            Ok(Producer::Bundle { steps })
        }
        [KEY_DIR] => Ok(Producer::Dir {
            roots: dir_roots(raw)?,
            depth: raw.depth.unwrap_or(DEFAULT_FOLDER_DEPTH),
            only: raw.only.unwrap_or_default(),
            include: raw.include.clone().unwrap_or_default(),
            exclude: raw.exclude.clone().unwrap_or_default(),
        }),
        [KEY_FILE] => Ok(Producer::File {
            path: raw.file.clone().unwrap_or_default(),
            format: raw.format.unwrap_or_default(),
        }),
        [KEY_RUN] => Ok(Producer::Run {
            command: raw.run.clone().unwrap_or_default(),
            cwd: raw.cwd.clone(),
            timeout: raw
                .timeout
                .as_deref()
                .map(|value| parse_duration(value).map_err(|err| format!("timeout {err}")))
                .transpose()?,
            format: raw.format.unwrap_or_default(),
        }),
        many => Err(format!(
            "declares {}, but a block is one thing: pick one",
            many.join(" and ")
        )),
    }
}

/// `dir` and `dirs` are the same key in two shapes, so a block that starts with
/// one place and later gathers another never has to be restructured.
fn dir_roots(raw: &RawBlock) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    out.extend(raw.dir.clone());
    out.extend(raw.dirs.clone().unwrap_or_default());
    out.retain(|value| !value.trim().is_empty());
    if out.is_empty() {
        return Err(format!("`{KEY_DIR}` cannot be empty"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(contents: &str) -> Block {
        let parsed = parse_file(contents).expect("valid file");
        assert!(parsed.problems.is_empty(), "{:?}", parsed.problems);
        parsed.blocks.into_iter().next().expect("one block")
    }

    #[test]
    fn a_bundle_is_a_name_and_a_list_of_steps() {
        let block = one(r#"
[work]
name = "Work setup"
do = [
  "open -a Slack",
  "open -a Safari https://github.com",
]
"#);
        assert_eq!(block.id, "work");
        assert_eq!(block.name, "Work setup");
        assert!(block.is_bundle());
        match block.producer {
            Producer::Bundle { steps } => assert_eq!(steps.len(), 2),
            other => panic!("expected a bundle, got {other:?}"),
        }
    }

    #[test]
    fn the_producer_key_is_the_only_thing_that_says_what_a_block_is() {
        assert_eq!(one("[a]\ndir = \"~/dev\"\n").producer.key(), KEY_DIR);
        assert_eq!(one("[b]\nfile = \"~/hosts\"\n").producer.key(), KEY_FILE);
        assert_eq!(one("[c]\nrun = \"ls\"\n").producer.key(), KEY_RUN);
        assert_eq!(one("[d]\ndo = [\"ls\"]\n").producer.key(), KEY_DO);
    }

    #[test]
    fn a_block_that_is_two_things_at_once_is_refused() {
        let parsed = parse_file("[a]\ndir = \"~/dev\"\nrun = \"ls\"\n").unwrap();
        assert!(parsed.blocks.is_empty());
        assert!(
            parsed.problems[0].contains("pick one"),
            "{:?}",
            parsed.problems
        );
    }

    #[test]
    fn a_block_with_no_producer_says_what_it_needs() {
        let parsed = parse_file("[a]\nname = \"Nothing\"\n").unwrap();
        assert!(
            parsed.problems[0].contains(KEY_DIR),
            "{:?}",
            parsed.problems
        );
    }

    #[test]
    fn verbs_are_the_commands_a_row_can_run() {
        let block = one(r#"
[projects]
name = "Projects"
dir  = "~/dev"
only = "dirs"
open = "open {path}"
edit = "nvim {path}"
"#);
        assert_eq!(
            block.verbs.declared(),
            [("open", "open {path}"), ("edit", "nvim {path}")]
        );
    }

    #[test]
    fn a_bundle_cannot_also_declare_a_verb() {
        // Enter already means "perform the steps", so a verb would be a second
        // hidden meaning for the same key.
        let parsed = parse_file("[a]\ndo = [\"ls\"]\nedit = \"nvim\"\n").unwrap();
        assert!(
            parsed.problems[0].contains("cannot also"),
            "{:?}",
            parsed.problems
        );
    }

    #[test]
    fn name_falls_back_to_the_block_header() {
        assert_eq!(
            one("[downloads]\ndir = \"~/Downloads\"\n").name,
            "downloads"
        );
    }

    #[test]
    fn one_file_holds_as_many_blocks_as_you_like() {
        let parsed = parse_file(
            r#"
[projects]
dir = "~/dev"

[work]
do = ["open -a Slack"]

[notes]
dir = "~/notes"
"#,
        )
        .unwrap();
        let ids: Vec<&str> = parsed.blocks.iter().map(|b| b.id.as_str()).collect();
        assert_eq!(ids, ["notes", "projects", "work"]);
        assert!(parsed.problems.is_empty());
    }

    #[test]
    fn a_broken_block_is_reported_and_its_neighbours_still_load() {
        let parsed = parse_file("[good]\ndir = \"~/dev\"\n\n[bad]\nname = \"x\"\n").unwrap();
        let ids: Vec<&str> = parsed.blocks.iter().map(|b| b.id.as_str()).collect();
        assert_eq!(ids, ["good"]);
        assert_eq!(parsed.problems.len(), 1);
        assert!(parsed.problems[0].contains("[bad]"));
    }

    #[test]
    fn dir_and_dirs_are_the_same_key_in_two_shapes() {
        let block = one("[a]\ndir = \"~/dev\"\ndirs = [\"~/work\"]\n");
        match block.producer {
            Producer::Dir { roots, .. } => assert_eq!(roots, ["~/dev", "~/work"]),
            other => panic!("expected a dir block, got {other:?}"),
        }
    }

    #[test]
    fn unknown_keys_are_reported_but_never_fatal() {
        let block = one("[a]\ndir = \"~/dev\"\nfuture_key = 1\n");
        assert_eq!(block.unknown_keys, ["future_key"]);
    }

    #[test]
    fn an_executable_alone_is_a_whole_block() {
        let block = inferred("hosts", "/home/u/.look/sources/hosts");
        assert_eq!(block.name, "hosts");
        assert_eq!(block.producer.key(), KEY_RUN);
    }

    #[test]
    fn a_destructive_block_can_ask_before_it_acts() {
        let block = one("[drop]\nconfirm = \"Delete {id}?\"\ndo = [\"git branch -D {id}\"]\n");
        assert_eq!(block.confirm.as_deref(), Some("Delete {id}?"));
        assert!(block.needs_row());
    }

    #[test]
    fn a_block_id_with_a_colon_is_refused() {
        // `["team:prod"]` is valid TOML, and its rows would be `src:team:prod:x`,
        // which reads back as block `team` and row `prod:x`.
        let parsed = parse_file("[\"team:prod\"]\ndir = \"~/dev\"\n").unwrap();
        assert!(parsed.blocks.is_empty());
        assert!(parsed.problems[0].contains(':'), "{:?}", parsed.problems);
    }

    #[test]
    fn a_blank_confirm_is_no_confirm() {
        // An empty string would otherwise mean "ask", with nothing to read.
        assert!(
            one("[a]\nconfirm = \"  \"\ndo = [\"true\"]\n")
                .confirm
                .is_none()
        );
    }

    #[test]
    fn a_producer_emitting_json_is_not_mistaken_for_one_that_needs_a_row() {
        // Every brace used to count as a placeholder, so a `run` command that
        // printed JSON was read as "only meaningful against a selected row" and
        // vanished from the index entirely.
        let json = one(r#"[branches]
format = "json"
run = "git for-each-ref --format='{\"id\":\"%(refname:short)\"}' refs/heads"
"#);
        assert!(!json.needs_row());

        let real = one("[deploy]\nrun = \"make -C {path} targets\"\n");
        assert!(real.needs_row());
    }

    #[test]
    fn the_shipped_example_parses_with_nothing_left_unexplained() {
        // example.toml is what users copy, so a key renamed here without
        // updating it would hand everyone a file full of ignored settings.
        let parsed = parse_file(include_str!("../example.toml")).expect("valid example");
        assert!(parsed.problems.is_empty(), "{:?}", parsed.problems);

        let ids: Vec<&str> = parsed.blocks.iter().map(|b| b.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "branches",
                "drop-branch",
                "ghostty",
                "hosts",
                "projects",
                "repos",
                "work"
            ]
        );

        // Anything the example shows as destructive must ask first, or it
        // teaches the wrong habit to everyone who copies it.
        let destructive = parsed
            .blocks
            .iter()
            .find(|block| block.id == "drop-branch")
            .expect("the delete example");
        assert!(destructive.confirm.is_some());

        // Every `then` target the example names must exist, or the file it
        // hands new users would load with a reported error.
        for block in &parsed.blocks {
            for target in &block.then {
                assert!(
                    parsed.blocks.iter().any(|other| &other.id == target),
                    "[{}] then names unknown block \"{target}\"",
                    block.id
                );
            }
        }

        for block in &parsed.blocks {
            assert!(
                block.unknown_keys.is_empty(),
                "[{}] has unknown keys: {:?}",
                block.id,
                block.unknown_keys
            );
        }
    }

    #[test]
    fn then_names_the_blocks_a_row_can_reach() {
        let block = one("[projects]\ndir = \"~/dev\"\nthen = [\"deploy\", \"branches\"]\n");
        assert_eq!(block.then, ["deploy", "branches"]);
    }

    #[test]
    fn the_target_producer_decides_action_or_drill_down() {
        // `then` carries no mode: a target that performs steps is an action, a
        // target that produces rows is a level to descend into.
        let parsed = parse_file(
            r#"
[projects]
dir  = "~/dev"
then = ["deploy", "branches"]

[deploy]
do = ["make -C {path} deploy"]

[branches]
run = "git -C {path} branch"
"#,
        )
        .unwrap();
        assert!(parsed.problems.is_empty());
        let deploy = parsed.blocks.iter().find(|b| b.id == "deploy").unwrap();
        let branches = parsed.blocks.iter().find(|b| b.id == "branches").unwrap();
        assert!(deploy.is_bundle(), "performs its steps");
        assert!(!branches.is_bundle(), "produces rows");
    }

    #[test]
    fn a_block_that_refers_to_a_row_cannot_stand_alone() {
        // Running `make -C {path} deploy` with nothing selected would substitute
        // an empty path, so it is reachable only through `then`.
        let deploy = one("[deploy]\ndo = [\"make -C {path} deploy\"]\n");
        assert!(deploy.needs_row());

        let work = one("[work]\ndo = [\"open -a Slack\"]\n");
        assert!(!work.needs_row(), "names no row, so it is a top-level row");
    }

    #[test]
    fn a_command_that_only_names_a_row_in_its_cwd_still_needs_one() {
        // `cd {path} && npm run` and `run = "npm run", cwd = "{path}"` are the
        // same block written two ways. Reading only the command indexed the
        // second as a top-level row, then ran it against a literal `{path}`.
        let by_cwd = one("[scripts]\nrun = \"npm run\"\ncwd = \"{path}\"\n");
        assert!(by_cwd.needs_row());

        let by_command = one("[scripts]\nrun = \"npm --prefix {path} run\"\n");
        assert!(by_command.needs_row());

        // A fixed directory names no row, so the block stands on its own.
        let fixed = one("[scripts]\nrun = \"npm run\"\ncwd = \"~/dev/look\"\n");
        assert!(!fixed.needs_row());
    }

    #[test]
    fn timeout_takes_a_duration_and_nothing_else() {
        assert_eq!(parse_duration("90s").unwrap(), Duration::from_secs(90));
        assert_eq!(
            parse_duration("2h").unwrap(),
            Duration::from_secs(2 * SECONDS_PER_HOUR)
        );
        assert!(parse_duration("1w").is_err());
        assert!(parse_duration("0m").is_err());
        assert!(parse_duration("soon").is_err());
    }

    #[test]
    fn refresh_is_no_longer_a_key() {
        // `run` blocks refresh on reload, one gesture for everything. A leftover
        // `refresh = ...` is reported as unknown rather than silently ignored.
        let block = one("[a]\nrun = \"ls\"\nrefresh = \"open\"\n");
        assert_eq!(block.unknown_keys, ["refresh"]);
    }
}
