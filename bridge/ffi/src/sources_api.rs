//! C-ABI wrappers over `look_sources`, so the Swift shell can read a
//! user-declared block, list what a row can go on to, and perform it.
//!
//! A row carries only its candidate id, so every entry point starts by
//! resolving `src:<block>:<row>` back to the block that declared it. Reading the
//! directory again on demand (rather than caching it here) means a file the user
//! just edited takes effect without a reindex.

use std::path::PathBuf;

use look_indexing::CandidateIdKind;
use look_sources::{Block, ParentRow, Producer, RowContext, load_dir, sources_dir};
use serde::Serialize;
use std::os::raw::c_char;
use std::time::Duration;

use crate::state::{cstr_to_string, json_cstring_or_null};

/// Somewhere a row can go from here. `performs` is what the target's own
/// producer decides: steps to run, or rows to descend into.
#[derive(Serialize)]
struct ThenTarget {
    id: String,
    name: String,
    icon: Option<String>,
    performs: bool,
    /// The question to ask before running it, already expanded against the row,
    /// or null when it needs no confirmation.
    confirm: Option<String>,
}

/// What the panel shows for a block row: its name, the exact steps Enter will
/// perform, and where a row can go next.
#[derive(Serialize)]
struct BlockDetail {
    id: String,
    name: String,
    steps: Vec<String>,
    /// The file this block was declared in, so the panel can show it and
    /// reveal-in-Finder has something to point at.
    file: Option<String>,
    then: Vec<ThenTarget>,
}

/// One row of the block index the shell caches: enough to render a row without
/// re-reading the directory for every one of them.
#[derive(Serialize)]
struct BlockSummary {
    id: String,
    name: String,
    icon: Option<String>,
}

#[derive(Serialize)]
struct PerformOutcome {
    performed: usize,
    errors: Vec<String>,
    /// The target makes rows to pick from rather than steps to run, so the
    /// caller should descend into it. An explicit signal, because "nothing was
    /// performed" is also what a failure looks like.
    produces_rows: bool,
}

/// `{id, name, steps, file, then}` for the block a candidate id belongs to, or
/// the JSON literal `null` when it is not a block row or no longer exists.
pub(crate) fn look_source_block_json_impl(
    candidate_id: *const c_char,
    row_id: *const c_char,
    row_title: *const c_char,
    row_path: *const c_char,
    ancestors_json: *const c_char,
) -> *mut c_char {
    let candidate_id = cstr_to_string(candidate_id);
    let row = row_context(row_id, row_title, row_path, "", ancestors_json);
    let Some(block_id) = CandidateIdKind::source_id_of(&candidate_id) else {
        return json_cstring_or_null(None);
    };
    let Some(home) = home_dir() else {
        return json_cstring_or_null(None);
    };

    let blocks = load_dir(&sources_dir(&home)).blocks;
    let Some(block) = blocks.iter().find(|block| block.id == block_id) else {
        return json_cstring_or_null(None);
    };

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
                .map(|question| look_sources::expand(question, &row)),
        })
        .collect();

    json_cstring_or_null(
        serde_json::to_string(&BlockDetail {
            id: block.id.clone(),
            name: block.name.clone(),
            steps: steps_of(block),
            file: block.source_file.clone(),
            then,
        })
        .ok(),
    )
}

/// Every declared block as `{id, name, icon}`. The shell caches this once per
/// launcher open so a row can show its declared icon without a disk read each
/// time it renders.
pub(crate) fn look_source_blocks_json_impl() -> *mut c_char {
    let Some(home) = home_dir() else {
        return json_cstring_or_null(Some("[]".to_string()));
    };
    let summaries: Vec<BlockSummary> = load_dir(&sources_dir(&home))
        .blocks
        .into_iter()
        .map(|block| BlockSummary {
            id: block.id,
            name: block.name,
            icon: block.icon,
        })
        .collect();
    json_cstring_or_null(serde_json::to_string(&summaries).ok())
}

/// Performs `block_id` against the selected row, which is what its placeholders
/// expand to. Returns `{performed, errors, produces_rows}`.
///
/// `as_target` is the caller's intent, and only the caller knows it. Enter on a
/// row means "do what this block's `open` says", so a row-producing block runs
/// its verb. A `then` target means "go to this block", so a row-producing one is
/// a level to descend into and running its `open` would act on the WRONG row -
/// the one currently selected, not one of the rows the target would list.
pub(crate) fn look_perform_block_json_impl(
    block_id: *const c_char,
    row_id: *const c_char,
    row_title: *const c_char,
    row_path: *const c_char,
    query: *const c_char,
    ancestors_json: *const c_char,
    as_target: bool,
) -> *mut c_char {
    let block_id = cstr_to_string(block_id);
    let row = row_context(
        row_id,
        row_title,
        row_path,
        &cstr_to_string(query),
        ancestors_json,
    );

    let Some(block) = find_block(&block_id) else {
        return outcome(0, vec!["that block no longer exists".into()]);
    };

    // A bundle IS its steps. Any other producer makes rows: reached as a target
    // that means descend, reached by Enter it means run the block's `open`.
    let steps: Vec<String> = match &block.producer {
        Producer::Bundle { steps } => steps.clone(),
        _ if as_target => return produces_rows(),
        _ => match block.verbs.open.as_deref() {
            Some(command) => vec![command.to_string()],
            None => {
                return outcome(
                    0,
                    vec![format!("[{}] declares no `open` for its rows", block.id)],
                );
            }
        },
    };

    let outcomes = look_sources::perform(&steps, Some(&row));
    let errors: Vec<String> = outcomes
        .iter()
        .filter_map(|step| {
            step.error
                .as_ref()
                .map(|error| format!("{}: {error}", step.step))
        })
        .collect();
    outcome(outcomes.len() - errors.len(), errors)
}

/// Fallback limits for a captured command, when the block names none. A source
/// refresh happens while the user waits, so the ceiling is low on purpose.
const DEFAULT_CAPTURE_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_CAPTURE_BYTES: usize = 256 * 1024;

/// Re-runs every enabled `run` block and stores its rows, so the next index pass
/// picks them up. Returns `{refreshed, errors}`.
///
/// A block that fails keeps the rows it had: losing them would also drop the
/// usage history keyed to their ids, which is a worse outcome than stale rows.
pub(crate) fn look_refresh_run_blocks_json_impl() -> *mut c_char {
    let Some(home) = home_dir() else {
        return json_cstring_or_null(Some("{\"refreshed\":0,\"errors\":[]}".into()));
    };

    let mut refreshed = 0usize;
    let mut errors: Vec<String> = Vec::new();
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

        // A block whose command names a row placeholder only means something
        // against a selection: it is a level, run live when the user descends
        // (§2.10). Running it here would execute `jq ... {path}/package.json`
        // literally and report a failure the user cannot act on.
        if block.needs_row() {
            continue;
        }

        if !block.enabled {
            look_engine::index::clear_run_rows(&block.id);
            continue;
        }

        let outcome = look_sources::capture(
            command,
            cwd.as_deref(),
            timeout.unwrap_or(DEFAULT_CAPTURE_TIMEOUT),
            MAX_CAPTURE_BYTES,
        )
        .and_then(|output| look_engine::index::store_run_rows(&block.id, &output, *format));

        match outcome {
            Ok(rows) => refreshed += rows,
            Err(message) => errors.push(format!("[{}] {message}", block.id)),
        }
    }

    json_cstring_or_null(
        serde_json::to_string(&serde_json::json!({
            "refreshed": refreshed,
            "errors": errors,
        }))
        .ok(),
    )
}

/// How deep a stack of levels may go (`specs/user-sources.md` §2.10). Bounds
/// both the id and the stack, and a launcher six levels deep has stopped being
/// a launcher.
const MAX_LEVEL_DEPTH: usize = 5;

/// One row of a level, ready to render and to rank: its candidate id already
/// carries the ancestors it was reached through.
#[derive(Serialize)]
struct LevelRow {
    #[serde(rename = "candidateId")]
    candidate_id: String,
    id: String,
    title: String,
    subtitle: Option<String>,
    group: Option<String>,
    path: Option<String>,
}

#[derive(Serialize)]
struct Level {
    /// The parent's OWN id, as the core decoded it. Handed back so the shell can
    /// name the ancestor in later calls without keeping a second decoder for the
    /// id format (§2.10).
    #[serde(rename = "parentRowId")]
    parent_row_id: String,
    rows: Vec<LevelRow>,
    /// The row cap dropped rows, so the caller can say so rather than implying
    /// the level is all there is.
    truncated: bool,
    /// Set when the level could not be produced. The caller must not descend:
    /// an empty level and a broken command look identical on screen, so neither
    /// is silently entered.
    error: Option<String>,
}

fn level_error(message: impl Into<String>) -> *mut c_char {
    json_cstring_or_null(
        serde_json::to_string(&Level {
            parent_row_id: String::new(),
            rows: Vec::new(),
            truncated: false,
            error: Some(message.into()),
        })
        .ok(),
    )
}

/// The rows of `block_id` produced against the selected row, for descending into
/// it (`specs/user-sources.md` §2.10).
///
/// Live on every call, never cached: `~/.look/cache/rows/<block>` is keyed by
/// block alone, and a level's rows depend on the row it was launched from. The
/// user has already picked that row, so one command is a wait they asked for and
/// a stale level is worse.
pub(crate) fn look_source_rows_json_impl(
    block_id: *const c_char,
    parent_candidate_id: *const c_char,
    parent_title: *const c_char,
    parent_path: *const c_char,
    query: *const c_char,
    ancestors_json: *const c_char,
) -> *mut c_char {
    let block_id = cstr_to_string(block_id);
    let parent_candidate_id = cstr_to_string(parent_candidate_id);
    // Built here rather than through `row_context`, which takes C pointers: the
    // parent id is already an owned Rust string and handing its bytes back as a
    // C string would read past the end of it.
    let row = RowContext {
        id: CandidateIdKind::source_row_id_of(&parent_candidate_id).to_string(),
        title: cstr_to_string(parent_title),
        path: cstr_to_string(parent_path),
        query: cstr_to_string(query),
        // The parent's own ancestors: inside this call the parent IS the row, so
        // a producer saying `{parent.path}` means the grandparent.
        parents: parents_from(ancestors_json),
    };

    let Some(home) = home_dir() else {
        return level_error("no home directory");
    };
    let Some(block) = find_block(&block_id) else {
        return level_error("that block no longer exists");
    };
    if !block.enabled {
        return level_error(format!("[{}] is turned off", block.id));
    }

    // The chain the child's rows will carry: everything the parent was reached
    // through, then the parent itself.
    let Some(parent_block) = CandidateIdKind::source_id_of(&parent_candidate_id) else {
        return level_error("that row does not belong to a block");
    };
    let mut ancestors = CandidateIdKind::source_ancestors_of(&parent_candidate_id);
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
                timeout.unwrap_or(DEFAULT_CAPTURE_TIMEOUT),
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
            id: row.id,
            title: row.title,
            subtitle: row.subtitle,
            group: row.group,
            path: row.path.map(|path| {
                look_sources::expand_home(&path, &home)
                    .to_string_lossy()
                    .into_owned()
            }),
        })
        .collect();

    json_cstring_or_null(
        serde_json::to_string(&Level {
            parent_row_id: row.id.clone(),
            rows,
            truncated: collected.truncated,
            error: None,
        })
        .ok(),
    )
}

/// Runs a block's declared `preview` against the selected row and returns its
/// output as `{rows, error}`-shaped text, or `null` when it declares none.
pub(crate) fn look_source_preview_json_impl(
    candidate_id: *const c_char,
    row_id: *const c_char,
    row_title: *const c_char,
    row_path: *const c_char,
    ancestors_json: *const c_char,
) -> *mut c_char {
    let candidate_id = cstr_to_string(candidate_id);
    let Some(block_id) = CandidateIdKind::source_id_of(&candidate_id) else {
        return json_cstring_or_null(None);
    };
    let Some(block) = find_block(block_id) else {
        return json_cstring_or_null(None);
    };
    let Some(preview) = block.preview.as_deref() else {
        return json_cstring_or_null(None);
    };

    let row = row_context(row_id, row_title, row_path, "", ancestors_json);
    let command = look_sources::expand(preview, &row);
    let text = look_sources::capture(
        &command,
        Some(&row.working_dir()),
        DEFAULT_CAPTURE_TIMEOUT,
        MAX_CAPTURE_BYTES,
    );

    json_cstring_or_null(
        serde_json::to_string(&match text {
            Ok(text) => serde_json::json!({ "text": text, "error": serde_json::Value::Null }),
            Err(message) => serde_json::json!({ "text": "", "error": message }),
        })
        .ok(),
    )
}

/// What Enter will run: a bundle's steps, or the `open` verb that acts on a
/// row the block produced. Either way the panel shows the real commands.
/// The selected row as a user's command sees it.
///
/// `{id}` is the row's OWN id, never the namespaced candidate id: a script
/// asking for a branch expects `main`, and handing it `src:branches:main` makes
/// git read the whole thing as `rev:path`.
fn row_context(
    row_id: *const c_char,
    row_title: *const c_char,
    row_path: *const c_char,
    query: &str,
    ancestors_json: *const c_char,
) -> RowContext {
    let candidate_id = cstr_to_string(row_id);
    RowContext {
        id: CandidateIdKind::source_row_id_of(&candidate_id).to_string(),
        title: cstr_to_string(row_title),
        path: cstr_to_string(row_path),
        query: query.to_string(),
        parents: parents_from(ancestors_json),
    }
}

/// The rows a drilled row was reached through, for `{parent.*}`: nearest first,
/// as `[{id, title, path}]`.
///
/// The candidate id already carries the ancestor ids, but a placeholder can name
/// their titles and paths too, and only the shell still holds those. Anything
/// unparseable is no ancestors, which reads as empty rather than as a literal
/// placeholder reaching a shell.
fn parents_from(ancestors_json: *const c_char) -> Vec<ParentRow> {
    let raw = cstr_to_string(ancestors_json);
    if raw.trim().is_empty() {
        return Vec::new();
    }
    serde_json::from_str::<Vec<ParentRow>>(&raw).unwrap_or_default()
}

fn steps_of(block: &Block) -> Vec<String> {
    match &block.producer {
        Producer::Bundle { steps } => steps.clone(),
        _ => block.verbs.open.iter().cloned().collect(),
    }
}

fn outcome(performed: usize, errors: Vec<String>) -> *mut c_char {
    json_cstring_or_null(
        serde_json::to_string(&PerformOutcome {
            performed,
            errors,
            produces_rows: false,
        })
        .ok(),
    )
}

fn produces_rows() -> *mut c_char {
    json_cstring_or_null(
        serde_json::to_string(&PerformOutcome {
            performed: 0,
            errors: Vec::new(),
            produces_rows: true,
        })
        .ok(),
    )
}

fn find_block(block_id: &str) -> Option<Block> {
    let home = home_dir()?;
    load_dir(&sources_dir(&home))
        .blocks
        .into_iter()
        .find(|block| block.id == block_id)
}

/// The config file this build reads and writes, migrating a legacy
/// `~/.look.config` into `~/.look/` the first time.
///
/// The shell asks Rust rather than deciding for itself, so the two can never
/// resolve differently and end up reading one file while saving to another.
pub(crate) fn look_config_path_impl(dev: bool) -> *mut c_char {
    let Some(home) = home_dir() else {
        return json_cstring_or_null(None);
    };
    let resolved = look_engine::config_path::resolve_home_variant(&home, dev);
    json_cstring_or_null(Some(resolved.path.to_string_lossy().into_owned()))
}

fn home_dir() -> Option<PathBuf> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::{CStr, CString};

    /// The sources directory is process-wide state, so these run one at a time.
    static GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct Fixture {
        dir: PathBuf,
    }

    impl Fixture {
        fn new(label: &str, declarations: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("look-ffi-level-{label}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("fixture dir");
            std::fs::write(dir.join("blocks.toml"), declarations).expect("fixture file");
            unsafe { std::env::set_var(look_sources::SOURCES_DIR_ENV, &dir) };
            Self { dir }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            unsafe { std::env::remove_var(look_sources::SOURCES_DIR_ENV) };
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn descend(block: &str, parent_id: &str, parent_path: &str) -> serde_json::Value {
        let block = CString::new(block).unwrap();
        let parent = CString::new(parent_id).unwrap();
        let title = CString::new("parent").unwrap();
        let path = CString::new(parent_path).unwrap();
        let query = CString::new("").unwrap();
        let ancestors = CString::new("[]").unwrap();
        let ptr = look_source_rows_json_impl(
            block.as_ptr(),
            parent.as_ptr(),
            title.as_ptr(),
            path.as_ptr(),
            query.as_ptr(),
            ancestors.as_ptr(),
        );
        let raw = unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        crate::state::free_json_allocation(ptr);
        serde_json::from_str(&raw).expect("level json")
    }

    #[test]
    fn a_level_carries_the_ancestors_its_rows_were_reached_through() {
        let _guard = GUARD.lock().unwrap_or_else(|err| err.into_inner());
        let _fixture = Fixture::new(
            "rows",
            "[child]\nrun = \"printf 'one\\ttitle one\\ntwo\\n'\"\n",
        );

        let level = descend("child", "src:parent:alpha", "/tmp");
        assert!(level["error"].is_null(), "{level}");
        let rows = level["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["candidateId"], "src:child:|parent/alpha|one");
        assert_eq!(rows[0]["title"], "title one");

        // The same block under another parent is a different id, which is what
        // keeps usage ranking from bleeding between levels.
        let other = descend("child", "src:parent:beta", "/tmp");
        assert_eq!(
            other["rows"][0]["candidateId"],
            "src:child:|parent/beta|one"
        );
    }

    #[test]
    fn a_producer_expands_against_the_row_the_level_opened_from() {
        let _guard = GUARD.lock().unwrap_or_else(|err| err.into_inner());
        let _fixture = Fixture::new("expand", "[child]\nrun = \"basename {path}\"\n");

        let level = descend("child", "src:parent:alpha", "/tmp/some project");
        assert!(level["error"].is_null(), "{level}");
        // Quoted on substitution, so a space in the path stays one argument.
        assert_eq!(level["rows"][0]["id"], "some project");
    }

    #[test]
    fn nothing_to_descend_into_is_an_error_rather_than_an_empty_level() {
        let _guard = GUARD.lock().unwrap_or_else(|err| err.into_inner());
        let _fixture = Fixture::new(
            "empty",
            "[child]\nrun = \"true\"\n[off]\nrun = \"echo x\"\nenabled = false\n",
        );

        assert!(descend("child", "src:parent:alpha", "/tmp")["error"].is_string());
        assert!(descend("off", "src:parent:alpha", "/tmp")["error"].is_string());
        assert!(descend("missing", "src:parent:alpha", "/tmp")["error"].is_string());
        // A row that belongs to no block cannot be a parent.
        assert!(descend("child", "app:safari", "/tmp")["error"].is_string());
    }

    #[test]
    fn an_ancestors_payload_reaches_the_producer_as_parent_placeholders() {
        let _guard = GUARD.lock().unwrap_or_else(|err| err.into_inner());
        let _fixture = Fixture::new("parents", "[child]\nrun = \"echo {parent.title}\"\n");

        let block = CString::new("child").unwrap();
        let parent = CString::new("src:mid:row").unwrap();
        let title = CString::new("mid").unwrap();
        let path = CString::new("/tmp").unwrap();
        let query = CString::new("").unwrap();
        let ancestors =
            CString::new(r#"[{"id":"outer","title":"the grandparent","path":"/tmp"}]"#).unwrap();
        let ptr = look_source_rows_json_impl(
            block.as_ptr(),
            parent.as_ptr(),
            title.as_ptr(),
            path.as_ptr(),
            query.as_ptr(),
            ancestors.as_ptr(),
        );
        let raw = unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        crate::state::free_json_allocation(ptr);
        let level: serde_json::Value = serde_json::from_str(&raw).unwrap();

        // Inside a producer the parent IS the row, so `{parent.*}` is the level
        // above that one.
        assert_eq!(level["rows"][0]["id"], "the grandparent", "{level}");
    }

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

        let ptr = look_refresh_run_blocks_json_impl();
        let raw = unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        crate::state::free_json_allocation(ptr);
        let outcome: serde_json::Value = serde_json::from_str(&raw).unwrap();

        assert_eq!(outcome["errors"].as_array().unwrap().len(), 0, "{outcome}");
        assert_eq!(outcome["refreshed"], 1, "only the flat block has rows");
        look_engine::index::clear_run_rows("flat");
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
        assert!(level["error"].as_str().unwrap().contains("deep"), "{level}");
    }
}
