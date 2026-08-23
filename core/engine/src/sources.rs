//! What a shell asks about a user-declared block: what a row's block is, what
//! Enter will run, what a `then` target does, and the rows one produces.
//!
//! Here rather than in each shell because none of it is native work. It is
//! `look_sources` calls plus the id encoding `look_indexing` owns, and the two
//! shells that need it would otherwise reimplement 350 lines apiece and drift
//! on what `confirm` expands to. The C ABI over this is transport; the Tauri
//! commands serialize these structs directly.
//!
//! A row carries only its candidate id, so every entry point starts by
//! resolving `src:<block>:<row>` back to the block that declared it. Reading the
//! directory again on demand (rather than caching it here) means a file the user
//! just edited takes effect without a reindex.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use look_indexing::CandidateIdKind;
use look_sources::{Block, Producer, RowContext, load_dir, sources_dir};
use serde::Serialize;

/// Fallback limits for a captured command, when the block names none. A source
/// refresh happens while the user waits, so the ceiling is low on purpose.
const DEFAULT_CAPTURE_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_CAPTURE_BYTES: usize = 256 * 1024;

/// The longest a single block may declare. `timeout` is the one limit the
/// format hands to the author, and it only ever raises the default - a block
/// asking for "24h" parses fine and would hold the refresh for a day.
const MAX_BLOCK_TIMEOUT: Duration = Duration::from_secs(30);

/// The longest a whole refresh may spend running commands. Blocks run one after
/// another, so without this the wait is the SUM of every block's timeout, and
/// each new source a user declares makes every reload slower.
const REFRESH_TIME_BUDGET: Duration = Duration::from_secs(60);

/// Below this a block is skipped rather than handed a sliver, which would come
/// back as a timeout it did not earn.
const MIN_BLOCK_SLICE: Duration = Duration::from_secs(1);

/// How deep a stack of levels may go. A launcher six levels deep has stopped
/// being a launcher.
pub const MAX_LEVEL_DEPTH: usize = 5;

/// Recorded like an open, so a routine run every morning ranks like one.
pub const USAGE_ACTION: &str = look_indexing::UsageAction::EXECUTE;

/// Somewhere a row can go from here. `performs` is what the target's own
/// producer decides: steps to run, or rows to descend into.
#[derive(Serialize)]
pub struct ThenTarget {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub performs: bool,
    /// The question to ask before running it, already expanded against the row,
    /// or null when it needs no confirmation.
    pub confirm: Option<String>,
}

/// What the panel shows for a block row: its name, the exact steps Enter will
/// perform, and where a row can go next.
#[derive(Serialize)]
pub struct BlockDetail {
    pub id: String,
    pub name: String,
    /// Expanded against the row, like `confirm` beside it: a panel that
    /// answers "what is about to run" has to show the command, not the
    /// template it was written as. Quoting included, since that is what runs.
    pub steps: Vec<String>,
    /// The file this block was declared in, so the panel can show it and
    /// reveal has something to point at.
    pub file: Option<String>,
    /// The block's OWN question, expanded against the row, or null when it
    /// declares none. Enter has to ask it: `confirm` exists because a launcher
    /// makes Enter on the wrong row cheap, and a block reached by Enter rather
    /// than through another block's `then` is the same row and the same risk.
    pub confirm: Option<String>,
    pub then: Vec<ThenTarget>,
    /// Whether a `preview` command will run for this row. Answered with the
    /// cheap details rather than by waiting for the command, so the panel can
    /// lay itself out before knowing what the output says - or that there will
    /// be none at all.
    #[serde(rename = "hasPreview")]
    pub has_preview: bool,
}

/// One row of the block index the shell caches: enough to render a row without
/// re-reading the directory for every one of them.
#[derive(Serialize)]
pub struct BlockSummary {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
}

#[derive(Serialize, Default)]
pub struct PerformOutcome {
    pub performed: usize,
    pub errors: Vec<String>,
    /// The target makes rows to pick from rather than steps to run, so the
    /// caller should descend into it. An explicit signal, because "nothing was
    /// performed" is also what a failure looks like.
    pub produces_rows: bool,
    /// The block declares no `open` and the row names a path, so the row IS the
    /// thing: the shell opens it the way it opens any file. One rule for Enter,
    /// decided here rather than from the row's kind, which says which producer
    /// made it and nothing the user chose.
    pub opens_path: bool,
}

/// One row of a level: its candidate id carries the ancestors it came through.
#[derive(Serialize)]
pub struct LevelRow {
    #[serde(rename = "candidateId")]
    pub candidate_id: String,
    pub id: String,
    pub title: String,
    /// Resolved against the block name by the same rule the index uses.
    pub subtitle: String,
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

#[derive(Serialize, Default)]
pub struct Level {
    /// The parent's OWN id, decoded here so the shell needs no second decoder
    /// for the id format.
    #[serde(rename = "parentRowId")]
    pub parent_row_id: String,
    pub rows: Vec<LevelRow>,
    /// The row cap dropped rows, so the caller can say so rather than implying
    /// the level is all there is.
    pub truncated: bool,
    /// Set when the level could not be produced, and the caller must not
    /// descend: an empty level and a broken command look identical on screen.
    pub error: Option<String>,
}

/// A block's declared `preview` output, or the reason it could not run. A
/// failure is carried rather than swallowed: a preview that silently does
/// nothing reads as the feature being broken.
#[derive(Serialize)]
pub struct PreviewOutcome {
    pub text: String,
    pub error: Option<String>,
}

#[derive(Serialize, Default)]
pub struct RefreshOutcome {
    pub refreshed: usize,
    pub errors: Vec<String>,
    /// Any write to the run cache, not just rows gained: turning a block off
    /// clears its rows, and those have to leave the index too. The shells act
    /// on it, because the dirty flag they gate refreshes on is their own.
    #[serde(skip)]
    pub changed: bool,
}

/// `{id, name, steps, file, then}` for the block a candidate id belongs to, or
/// `None` when it is not a block row or no longer exists.
pub fn block_detail(candidate_id: &str, row: &RowContext) -> Option<BlockDetail> {
    let block_id = CandidateIdKind::source_id_of(candidate_id)?;
    let blocks = load_dir(&sources_dir(&home_dir()?)).blocks;
    let block = blocks.iter().find(|block| block.id == block_id)?;

    let then = block
        .then
        .iter()
        .filter_map(|target| blocks.iter().find(|block| &block.id == target))
        .map(|target| ThenTarget {
            id: target.id.clone(),
            name: target.name.clone(),
            icon: target.icon.clone(),
            performs: target.is_bundle(),
            // Expanded here so the question names the row the user is looking
            // at ("Delete main?"), not the template.
            confirm: target
                .confirm
                .as_deref()
                .map(|question| look_sources::expand(question, row)),
        })
        .collect();

    Some(BlockDetail {
        id: block.id.clone(),
        name: block.name.clone(),
        steps: steps_of(block, row),
        file: block.source_file.clone(),
        confirm: block
            .confirm
            .as_deref()
            .map(|question| look_sources::expand(question, row)),
        then,
        has_preview: block.preview.is_some(),
    })
}

/// Every declared block as `{id, name, icon}`. The shell caches this once per
/// launcher open so a row can show its declared icon without a disk read each
/// time it renders.
pub fn block_summaries() -> Vec<BlockSummary> {
    let Some(home) = home_dir() else {
        return Vec::new();
    };
    load_dir(&sources_dir(&home))
        .blocks
        .into_iter()
        .map(|block| BlockSummary {
            id: block.id,
            name: block.name,
            icon: block.icon,
        })
        .collect()
}

/// Performs `block_id` against the selected row, which is what its placeholders
/// expand to.
///
/// `as_target` is the caller's intent, and only the caller knows it. Enter on a
/// row means "do what this block's `open` says", so a row-producing block runs
/// its verb. A `then` target means "go to this block", so a row-producing one is
/// a level to descend into and running its `open` would act on the WRONG row -
/// the one currently selected, not one of the rows the target would list.
pub fn perform_block(block_id: &str, row: &RowContext, as_target: bool) -> PerformOutcome {
    let Some(block) = find_block(block_id) else {
        return failed(vec!["that block no longer exists".into()]);
    };

    // A bundle IS its steps. Any other producer makes rows: reached as a target
    // that means descend, reached by Enter it means run the block's `open`.
    let steps: Vec<String> = match &block.producer {
        Producer::Bundle { steps } => steps.clone(),
        _ if as_target => {
            return PerformOutcome {
                produces_rows: true,
                ..Default::default()
            };
        }
        _ => match block.verbs.open.as_deref() {
            Some(command) => vec![command.to_string()],
            // A row that names a path needs no verb to be openable, which is
            // what makes `dir` blocks work without declaring one.
            None if !row.path.is_empty() => {
                return PerformOutcome {
                    opens_path: true,
                    ..Default::default()
                };
            }
            None => {
                return failed(vec![format!(
                    "[{}] declares no `open` for its rows",
                    block.id
                )]);
            }
        },
    };

    let outcomes = look_sources::perform(&steps, Some(row));
    let errors: Vec<String> = outcomes
        .iter()
        .filter_map(|step| {
            step.error
                .as_ref()
                .map(|error| format!("{}: {error}", step.step))
        })
        .collect();

    PerformOutcome {
        performed: outcomes.len() - errors.len(),
        errors,
        ..Default::default()
    }
}

/// The rows of `block_id` produced against the selected row, for descending.
///
/// Live on every call, never cached: `~/.look/cache/rows/<block>` is keyed by
/// block alone, and a level's rows depend on the row it opened from.
pub fn level(
    block_id: &str,
    parent_candidate_id: &str,
    parent_title: &str,
    parent_path: &str,
    query: &str,
    parents: Vec<look_sources::ParentRow>,
) -> Level {
    let row = RowContext {
        id: CandidateIdKind::source_row_id_of(parent_candidate_id).to_string(),
        title: parent_title.to_string(),
        path: parent_path.to_string(),
        query: query.to_string(),
        // The parent's own ancestors: inside this call the parent IS the row, so
        // a producer saying `{parent.path}` means the grandparent.
        parents,
    };

    let Some(home) = home_dir() else {
        return level_error("no home directory");
    };
    let Some(block) = find_block(block_id) else {
        return level_error("that block no longer exists");
    };
    if !block.enabled {
        return level_error(format!("[{}] is turned off", block.id));
    }

    // The chain the child's rows will carry: everything the parent was reached
    // through, then the parent itself.
    let Some(parent_block) = CandidateIdKind::source_id_of(parent_candidate_id) else {
        return level_error("that row does not belong to a block");
    };
    let mut ancestors = CandidateIdKind::source_ancestors_of(parent_candidate_id);
    ancestors.push((parent_block.to_string(), row.id.clone()));
    if ancestors.len() > MAX_LEVEL_DEPTH {
        return level_error(format!("{MAX_LEVEL_DEPTH} levels is as deep as this goes"));
    }

    let collected = match &block.producer {
        Producer::Run {
            command,
            cwd,
            timeout,
            format,
        } => {
            let command = look_sources::expand(command, &row);
            let cwd = cwd
                .as_deref()
                .map(|cwd| look_sources::expand_path(cwd, &row));
            match look_sources::capture(
                &command,
                cwd.as_deref(),
                capture_timeout(*timeout),
                MAX_CAPTURE_BYTES,
            )
            .and_then(|output| {
                look_sources::parse_rows(&output, look_sources::MAX_ROWS_PER_SOURCE, *format)
            }) {
                Ok((rows, truncated)) => look_sources::Collected {
                    rows,
                    truncated,
                    unreadable: Vec::new(),
                },
                Err(message) => return level_error(message),
            }
        }
        _ => match look_sources::collect_for_row(&block, &home, &row) {
            Ok(collected) => collected,
            Err(err) => return level_error(err.to_string()),
        },
    };

    if collected.rows.is_empty() {
        return level_error(format!("[{}] produced no rows", block.id));
    }

    let rows: Vec<LevelRow> = collected
        .rows
        .into_iter()
        .map(|row| LevelRow {
            candidate_id: CandidateIdKind::source_row_candidate_id(&block.id, &ancestors, &row.id),
            subtitle: row.display_subtitle(&block.name).to_string(),
            icon: row.display_icon(&home),
            id: row.id,
            title: row.title,
            path: row.path.map(|path| {
                look_sources::expand_home(&path, &home)
                    .to_string_lossy()
                    .into_owned()
            }),
        })
        .collect();

    Level {
        parent_row_id: row.id,
        rows,
        truncated: collected.truncated,
        error: None,
    }
}

/// Runs a block's declared `preview` against the selected row, or `None` when
/// it declares none.
pub fn preview(candidate_id: &str, row: &RowContext) -> Option<PreviewOutcome> {
    let block_id = CandidateIdKind::source_id_of(candidate_id)?;
    let block = find_block(block_id)?;
    let preview = block.preview.as_deref()?;

    let command = look_sources::expand(preview, row);
    Some(
        match look_sources::capture(
            &command,
            Some(&row.working_dir()),
            DEFAULT_CAPTURE_TIMEOUT,
            MAX_CAPTURE_BYTES,
        ) {
            Ok(text) => PreviewOutcome { text, error: None },
            Err(message) => PreviewOutcome {
                text: String::new(),
                error: Some(message),
            },
        },
    )
}

/// Re-runs every enabled `run` block and stores its rows, so the next index pass
/// picks them up.
///
/// A block that fails keeps the rows it had: losing them would also drop the
/// usage history keyed to their ids, which is a worse outcome than stale rows.
pub fn refresh_run_blocks() -> RefreshOutcome {
    let Some(home) = home_dir() else {
        return RefreshOutcome::default();
    };

    let mut outcome = RefreshOutcome::default();
    let started_at = Instant::now();
    let mut skipped: Vec<String> = Vec::new();
    for block in load_dir(&sources_dir(&home)).blocks {
        let Producer::Run {
            command,
            cwd,
            timeout,
            format,
        } = &block.producer
        else {
            continue;
        };

        // A block whose command names a row placeholder is a level, run live
        // when the user descends. Running it here would execute
        // `jq ... {path}/package.json` literally and report a useless failure.
        if block.needs_row() {
            continue;
        }

        if !block.enabled {
            crate::index::clear_run_rows(&block.id);
            outcome.changed = true;
            continue;
        }

        // Out of budget: the blocks left keep the rows they had, and are named
        // rather than dropped silently - "my source stopped updating" with no
        // reason given is worse than a slow one.
        let Some(timeout) = block_slice(*timeout, started_at.elapsed()) else {
            skipped.push(block.id.clone());
            continue;
        };

        let stored = look_sources::capture(command, cwd.as_deref(), timeout, MAX_CAPTURE_BYTES)
            .and_then(|output| crate::index::store_run_rows(&block.id, &output, *format));

        match stored {
            Ok(rows) => {
                outcome.refreshed += rows;
                outcome.changed = true;
            }
            Err(message) => outcome.errors.push(format!("[{}] {message}", block.id)),
        }
    }

    if !skipped.is_empty() {
        outcome.errors.push(format!(
            "refresh ran out of time after {}s; not refreshed: {}",
            REFRESH_TIME_BUDGET.as_secs(),
            skipped.join(", ")
        ));
    }

    outcome
}

/// The block `block_id` names, read from the directory rather than a cache so
/// an edited file takes effect on the next press.
pub fn find_block(block_id: &str) -> Option<Block> {
    load_dir(&sources_dir(&home_dir()?))
        .blocks
        .into_iter()
        .find(|block| block.id == block_id)
}

/// The block that produced `candidate_id`, or `None` when no block did.
///
/// Here rather than in a shell because `src:<block>:<row>` is the indexing
/// crate's format: a caller that split the id itself would be the second place
/// that has to learn the drilled spelling.
pub fn block_for_candidate(candidate_id: &str) -> Option<Block> {
    find_block(CandidateIdKind::source_id_of(candidate_id)?)
}

/// Whether this row was reached by descending, which is what makes it
/// transient: a drilled row is never written to `candidates`, so there is
/// nothing for `usage_events` to key on.
pub fn is_drilled_row(candidate_id: &str) -> bool {
    !CandidateIdKind::source_ancestors_of(candidate_id).is_empty()
}

/// The row a user's command sees, built from what the shell holds.
///
/// `{id}` is the row's OWN id, never the namespaced candidate id: a script
/// asking for a branch expects `main`, and handing it `src:branches:main` makes
/// git read the whole thing as `rev:path`.
pub fn row_context(
    candidate_id: &str,
    title: &str,
    path: &str,
    query: &str,
    parents: Vec<look_sources::ParentRow>,
) -> RowContext {
    RowContext {
        id: CandidateIdKind::source_row_id_of(candidate_id).to_string(),
        title: title.to_string(),
        path: path.to_string(),
        query: query.to_string(),
        parents,
    }
}

/// The rows a drilled row was reached through, for `{parent.*}`: nearest first,
/// as `[{id, title, path}]`.
///
/// The candidate id already carries the ancestor ids, but a placeholder can name
/// their titles and paths too, and only the shell still holds those. Anything
/// unparseable is no ancestors, which reads as empty rather than as a literal
/// placeholder reaching a shell.
pub fn parents_from_json(ancestors_json: &str) -> Vec<look_sources::ParentRow> {
    if ancestors_json.trim().is_empty() {
        return Vec::new();
    }
    serde_json::from_str(ancestors_json).unwrap_or_default()
}

/// What Enter will run: a bundle's steps, or the `open` verb that acts on a row
<<<<<<< HEAD
/// the block produced. Either way the panel shows the real commands.
/// What a block is actually given: its own `timeout`, else the default, capped.
fn capture_timeout(declared: Option<Duration>) -> Duration {
    declared
        .unwrap_or(DEFAULT_CAPTURE_TIMEOUT)
        .min(MAX_BLOCK_TIMEOUT)
}

/// The same, capped by what the refresh has left. `None` means skip it: gating
/// entry alone, a block starting just under the budget still spent its whole
/// timeout on top, so 60s could take 90.
fn block_slice(declared: Option<Duration>, elapsed: Duration) -> Option<Duration> {
    let remaining = REFRESH_TIME_BUDGET.saturating_sub(elapsed);
    (remaining >= MIN_BLOCK_SLICE).then(|| capture_timeout(declared).min(remaining))
}

fn steps_of(block: &Block) -> Vec<String> {
    match &block.producer {
        Producer::Bundle { steps } => steps.clone(),
        _ => block.verbs.open.iter().cloned().collect(),
    }
=======
/// the block produced. Either way the panel shows the real commands, expanded
/// against the row the way the runner will expand them.
fn steps_of(block: &Block, row: &RowContext) -> Vec<String> {
    let declared: Vec<&str> = match &block.producer {
        Producer::Bundle { steps } => steps.iter().map(String::as_str).collect(),
        _ => block.verbs.open.iter().map(String::as_str).collect(),
    };
    declared
        .into_iter()
        .map(|step| look_sources::expand(step, row))
        .collect()
>>>>>>> 65fc4f3 (cosmetic)
}

fn failed(errors: Vec<String>) -> PerformOutcome {
    PerformOutcome {
        errors,
        ..Default::default()
    }
}

fn level_error(message: impl Into<String>) -> Level {
    Level {
        error: Some(message.into()),
        ..Default::default()
    }
}

fn home_dir() -> Option<PathBuf> {
    crate::config::user_home_dir().map(|home| Path::new(&home).to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_block_cannot_hold_a_refresh_longer_than_the_ceiling() {
        assert_eq!(capture_timeout(None), DEFAULT_CAPTURE_TIMEOUT);
        // Under the ceiling is honoured, including asking for less than default.
        assert_eq!(
            capture_timeout(Some(Duration::from_secs(1))),
            Duration::from_secs(1)
        );
        assert_eq!(
            capture_timeout(Some(Duration::from_secs(20))),
            Duration::from_secs(20)
        );
        assert_eq!(
            capture_timeout(Some(Duration::from_secs(86_400))),
            MAX_BLOCK_TIMEOUT
        );
    }

    #[test]
    fn a_block_never_spends_more_than_the_refresh_has_left() {
        let spent = REFRESH_TIME_BUDGET - Duration::from_secs(3);
        assert_eq!(
            block_slice(Some(MAX_BLOCK_TIMEOUT), spent),
            Some(Duration::from_secs(3)),
            "the remaining budget wins over the block's own ceiling"
        );
        assert_eq!(
            block_slice(Some(Duration::from_secs(20)), Duration::ZERO),
            Some(Duration::from_secs(20))
        );

        assert_eq!(block_slice(None, REFRESH_TIME_BUDGET), None);
        assert_eq!(
            block_slice(None, REFRESH_TIME_BUDGET + Duration::from_secs(30)),
            None,
            "overrunning the budget must not underflow"
        );
        assert_eq!(
            block_slice(None, REFRESH_TIME_BUDGET - MIN_BLOCK_SLICE / 2),
            None,
            "a sliver is a skip"
        );
    }

    /// The sources directory is process-wide state, so these run one at a time.
    static GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct Fixture {
        dir: PathBuf,
    }

    impl Fixture {
        fn new(label: &str, declarations: &str) -> Self {
            let dir = std::env::temp_dir()
                .join(format!("look-engine-src-{label}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("fixture dir");
            std::fs::write(dir.join("blocks.toml"), declarations).expect("fixture file");
            unsafe { std::env::set_var(look_sources::SOURCES_DIR_ENV, &dir) };
            Self { dir }
        }

        /// Rows for a `file` block to read.
        fn rows(&self, name: &str, contents: &str) {
            let path = self.dir.join(name);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).expect("rows dir");
            }
            std::fs::write(&path, contents).expect("rows file");
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            unsafe { std::env::remove_var(look_sources::SOURCES_DIR_ENV) };
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn descend(block: &str, parent_id: &str, parent_path: &str) -> Level {
        level(block, parent_id, "parent", parent_path, "", Vec::new())
    }

    #[test]
    fn a_level_carries_the_ancestors_its_rows_were_reached_through() {
        let _guard = GUARD.lock().unwrap_or_else(|err| err.into_inner());
        // A `file` producer: ids and ancestors are not shell behaviour, and
        // `run` refuses off unix.
        let fixture = Fixture::new("rows", "[child]\nfile = \"{path}/rows.txt\"\n");
        fixture.rows("rows.txt", "one\ttitle one\ntwo\n");
        let root = fixture.dir.to_string_lossy().into_owned();

        let level = descend("child", "src:parent:alpha", &root);
        assert!(level.error.is_none(), "{:?}", level.error);
        assert_eq!(level.rows.len(), 2);
        assert_eq!(level.rows[0].candidate_id, "src:child:|parent/alpha|one");
        assert_eq!(level.rows[0].title, "title one");

        // The same block under another parent is a different id, which is what
        // keeps usage ranking from bleeding between levels.
        let other = descend("child", "src:parent:beta", &root);
        assert_eq!(other.rows[0].candidate_id, "src:child:|parent/beta|one");
    }

    #[test]
    fn a_producer_expands_against_the_row_the_level_opened_from() {
        let _guard = GUARD.lock().unwrap_or_else(|err| err.into_inner());
        let fixture = Fixture::new("expand", "[child]\nfile = \"{path}/rows.txt\"\n");
        // A space in the name: the level is empty unless `{path}` was
        // substituted and left unquoted.
        fixture.rows("some project/rows.txt", "one\n");
        let inside = fixture.dir.join("some project");

        let level = descend("child", "src:parent:alpha", &inside.to_string_lossy());
        assert!(level.error.is_none(), "{:?}", level.error);
        assert_eq!(level.rows[0].id, "one");
    }

    #[test]
    fn nothing_to_descend_into_is_an_error_rather_than_an_empty_level() {
        let _guard = GUARD.lock().unwrap_or_else(|err| err.into_inner());
        let fixture = Fixture::new(
            "empty",
            "[child]\nfile = \"{path}/rows.txt\"\n[off]\nfile = \"{path}/rows.txt\"\nenabled = false\n",
        );
        // Produced nothing, rather than failed to run: a `run` block would
        // pass this off unix by reporting the missing shell.
        fixture.rows("rows.txt", "\n");
        let root = fixture.dir.to_string_lossy().into_owned();

        let empty = descend("child", "src:parent:alpha", &root);
        assert!(
            empty.error.unwrap_or_default().contains("no rows"),
            "an empty producer must not descend"
        );
        assert!(descend("off", "src:parent:alpha", &root).error.is_some());
        assert!(
            descend("missing", "src:parent:alpha", &root)
                .error
                .is_some()
        );
        // A row that belongs to no block cannot be a parent.
        assert!(descend("child", "app:safari", &root).error.is_some());
    }

    #[test]
    fn an_ancestors_payload_reaches_the_producer_as_parent_placeholders() {
        let _guard = GUARD.lock().unwrap_or_else(|err| err.into_inner());
        let fixture = Fixture::new(
            "parents",
            "[child]\nfile = \"{path}/{parent.title}/rows.txt\"\n",
        );
        // The grandparent's title names the folder, so an empty level means
        // `{parent.title}` never reached the producer.
        fixture.rows("outer-title/rows.txt", "found\n");

        let level = level(
            "child",
            "src:mid:row",
            "mid",
            &fixture.dir.to_string_lossy(),
            "",
            parents_from_json(r#"[{"id":"outer","title":"outer-title","path":"/tmp"}]"#),
        );

        // Inside a producer the parent IS the row, so `{parent.*}` is the level
        // above that one.
        assert_eq!(level.rows[0].id, "found", "{:?}", level.error);
    }

    #[test]
    fn enter_runs_the_declared_open_whatever_producer_made_the_row() {
        let _guard = GUARD.lock().unwrap_or_else(|err| err.into_inner());
        let _fixture = Fixture::new(
            "enter",
            "[declared]\ndir = \"/tmp\"\nopen = \"true\"\n[bare]\ndir = \"/tmp\"\n[pathless]\nfile = \"/tmp/rows.txt\"\n",
        );
        let perform = |block: &str, path: &str| {
            perform_block(
                block,
                &row_context("src:probe:row", "row", path, "", Vec::new()),
                false,
            )
        };

        // Declared: run it. This is the case a `dir` block could not reach
        // before, so `open` on one was a key that did nothing.
        let ran = perform("declared", "/tmp");
        assert!(!ran.opens_path);
        // The spawn is the platform's business: `run` refuses off unix.
        #[cfg(unix)]
        assert_eq!(ran.performed, 1, "{:?}", ran.errors);

        // Nothing declared but the row has a path: the row IS the thing.
        let opens = perform("bare", "/tmp");
        assert!(opens.opens_path);
        assert!(opens.errors.is_empty());

        // Nothing declared and nowhere to point: the only case left that is an
        // error the user can act on.
        let stuck = perform("pathless", "");
        assert!(!stuck.opens_path);
        assert!(stuck.errors[0].contains("open"));
    }

    /// Refresh runs a `run` block's command, which `run` refuses off unix.
    #[cfg(unix)]
    #[test]
    fn a_refresh_leaves_the_levels_alone() {
        // A level's command is written for a selected row. Refresh has none, so
        // running it would hand the shell a literal `{path}` and report an
        // error against a block that is working exactly as declared.
        let _guard = GUARD.lock().unwrap_or_else(|err| err.into_inner());
        let _fixture = Fixture::new(
            "refresh",
            "[level]\nrun = \"jq . {path}/package.json\"\n[flat]\nrun = \"echo one\"\n",
        );

        let outcome = refresh_run_blocks();
        assert!(outcome.errors.is_empty(), "{:?}", outcome.errors);
        assert_eq!(outcome.refreshed, 1, "only the flat block has rows");
        crate::index::clear_run_rows("flat");
    }

    #[test]
    fn a_targets_question_is_expanded_against_the_row_it_will_act_on() {
        let _guard = GUARD.lock().unwrap_or_else(|err| err.into_inner());
        let _fixture = Fixture::new(
            "targets",
            "[branches]\nfile = \"/tmp/rows.txt\"\nopen = \"git checkout {id}\"\nthen = [\"drop\", \"logs\", \"missing\"]\n\n[drop]\nname = \"Delete branch\"\nconfirm = \"Delete local branch {id}?\"\ndo = [\"git branch -d {id}\"]\n\n[logs]\nname = \"Commits\"\nrun = \"git log {id}\"\n",
        );

        let row = row_context("src:branches:main", "main", "", "", Vec::new());
        let block = block_detail("src:branches:main", &row).expect("a declared block");
        // The command, not the template: the panel is what the user reads
        // before pressing Enter, and its copy button hands them something they
        // can paste.
        assert_eq!(block.steps, vec!["git checkout 'main'".to_string()]);

        // A `then` naming a block that does not exist is reported by the
        // loader, not offered as a target.
        assert_eq!(block.then.len(), 2);
        // The question names the row the user is looking at, not the template.
        // Shell-quoted like every other substitution, since the same expansion
        // serves the command that runs if the answer is yes.
        assert_eq!(
            block.then[0].confirm.as_deref(),
            Some("Delete local branch 'main'?")
        );
        assert!(block.then[0].performs, "steps to run");
        // A target that lists is a level to descend into, and says so.
        assert!(!block.then[1].performs, "rows to pick from");
        assert!(block.then[1].confirm.is_none());
        // The row's own block asks nothing, so Enter on it runs straight away.
        assert!(block.confirm.is_none());

        // A block that DOES declare one carries it here, expanded the same way:
        // Enter on its row is the same risk as reaching it through a `then`.
        let target = row_context("src:drop:main", "main", "", "", Vec::new());
        assert_eq!(
            block_detail("src:drop:main", &target)
                .expect("a declared block")
                .confirm
                .as_deref(),
            Some("Delete local branch 'main'?")
        );
    }

    #[test]
    fn the_stack_stops_at_the_declared_depth() {
        let _guard = GUARD.lock().unwrap_or_else(|err| err.into_inner());
        let _fixture = Fixture::new("depth", "[child]\nrun = \"echo x\"\n");

        let chain = (0..MAX_LEVEL_DEPTH)
            .map(|i| format!("b{i}/r{i}"))
            .collect::<Vec<_>>()
            .join(";");
        let deep = format!("src:parent:|{chain}|row");
        let level = descend("child", &deep, "/tmp");
        assert!(level.error.unwrap_or_default().contains("deep"));
    }
}
