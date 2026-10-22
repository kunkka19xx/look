//! The processes behind `connect` blocks: one child per block, kept alive,
//! speaking the frames in `look_sources::connect`.
//!
//! Rows it sends go to the same cache a `run` block uses, so indexing, ranking
//! and the last-good-rows rule all stay where they are. Nothing here blocks a
//! search: a connector that is slow, silent or dead simply has no new rows.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::Duration;

use look_sources::{Ask, Content, PROTOCOL_VERSION, Producer, Say, SourceRow};

/// Told the block id whenever a connector sends rows, so the shell can mark its
/// index dirty. A callback rather than a flag here: the engine does not own the
/// index's freshness, the shell does.
pub type OnRows = Arc<dyn Fn(&str) + Send + Sync>;

/// A connector's stdin, shared so a write never holds the registry.
type Pipe = Arc<Mutex<ChildStdin>>;

struct Running {
    child: Child,
    /// Held open: dropping it is EOF to a connector that reads stdin. Shared,
    /// so a write to a connector that stopped reading blocks on its own pipe
    /// rather than on the registry every other connector needs.
    stdin: Option<Pipe>,
    /// To tell a restart from a changed command.
    command: String,
    /// The file that declared it: the namespace it may speak for.
    manifest: String,
}

fn running() -> &'static Mutex<BTreeMap<String, Running>> {
    static RUNNING: OnceLock<Mutex<BTreeMap<String, Running>>> = OnceLock::new();
    RUNNING.get_or_init(|| Mutex::new(BTreeMap::new()))
}

/// Starts every declared connector not already running, and stops any whose
/// block is gone or whose command changed. Safe to call repeatedly: it is how
/// a reload reaches connectors.
pub fn start(home: &Path, on_rows: OnRows) {
    let declared: BTreeMap<String, Declared> = crate::sources::declared(home)
        .blocks
        .into_iter()
        .filter(|block| block.enabled)
        .filter_map(|block| {
            let manifest = Path::new(block.source_file.as_deref()?)
                .file_stem()?
                .to_string_lossy()
                .into_owned();
            match block.producer {
                Producer::Connect { command } => Some((block.id, Declared { command, manifest })),
                _ => None,
            }
        })
        .collect();

    let mut running = locked();

    running.retain(|id, alive| {
        let still_declared = declared.get(id).map(|found| &found.command) == Some(&alive.command);
        let exited = matches!(alive.child.try_wait(), Ok(Some(_)) | Err(_));
        if !still_declared || exited {
            end(alive);
            return false;
        }
        true
    });

    for (id, found) in declared {
        if !running.contains_key(&id) {
            launch(&mut running, id, found, &on_rows);
        }
    }
}

/// A connector as its block declares it.
struct Declared {
    command: String,
    manifest: String,
}

/// Respawns connectors that died, and nothing else. Reads no directories, so
/// the shell can call it every time the launcher opens; noticing a manifest
/// installed since launch is what `start` is for.
pub fn restart_dead(on_rows: OnRows) {
    let mut running = locked();
    let dead: Vec<(String, Declared)> = running
        .iter_mut()
        .filter_map(|(id, alive)| {
            matches!(alive.child.try_wait(), Ok(Some(_)) | Err(_)).then(|| {
                (
                    id.clone(),
                    Declared {
                        command: alive.command.clone(),
                        manifest: alive.manifest.clone(),
                    },
                )
            })
        })
        .collect();

    for (id, found) in dead {
        if let Some(mut gone) = running.remove(&id) {
            end(&mut gone);
        }
        launch(&mut running, id, found, &on_rows);
    }
}

fn locked() -> std::sync::MutexGuard<'static, BTreeMap<String, Running>> {
    running().lock().unwrap_or_else(|err| err.into_inner())
}

/// Spawns one and records it. A connector that refuses to run is reported and
/// left out rather than retried in a loop.
fn launch(running: &mut BTreeMap<String, Running>, id: String, found: Declared, on_rows: &OnRows) {
    match spawn(&id, &found, on_rows.clone()) {
        Ok((child, stdin)) => {
            running.insert(
                id,
                Running {
                    child,
                    stdin,
                    command: found.command,
                    manifest: found.manifest,
                },
            );
        }
        Err(message) => eprintln!("look connectors: [{id}] {message}"),
    }
}

/// A preview a connector sent, and what its text is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preview {
    pub text: String,
    pub markdown: bool,
}

struct Latest {
    block: String,
    row: String,
    req: u64,
    preview: Preview,
}

/// One slot, not a cache: the panel shows one row at a time, so the answer to
/// the row before it is already stale. Bounded by construction rather than by
/// an eviction rule nobody would exercise.
fn latest() -> &'static (Mutex<Option<Latest>>, Condvar) {
    static LATEST: OnceLock<(Mutex<Option<Latest>>, Condvar)> = OnceLock::new();
    LATEST.get_or_init(|| (Mutex::new(None), Condvar::new()))
}

/// A preview past this is truncated. The frame cap already bounds one message;
/// this bounds what a panel is asked to lay out.
const MAX_PREVIEW_BYTES: usize = 64 * 1024;

/// Asks the connector behind `block` for a row's preview, and gives back the
/// request number to wait for. None when none is running, which is the
/// caller's cue not to wait for an answer at all.
///
/// Never blocks: an undelivered request simply has no answer, which the wait
/// already handles.
pub fn ask_preview(block: &str, row: &str) -> Option<u64> {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let req = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    // Emptied before asking, or the answer to the last look at this same row
    // is still sitting there and `preview_of` returns it without waiting.
    *latest().0.lock().unwrap_or_else(|err| err.into_inner()) = None;
    let handle = locked().get(block).and_then(|alive| alive.stdin.clone())?;

    let line = Ask::Preview {
        block: block.to_string(),
        id: row.to_string(),
        req,
    }
    .line();

    // Off this thread: a connector that stopped reading its stdin fills its
    // pipe, and the write would then block the panel here, before the wait
    // that is supposed to bound it even starts. `try_lock` holds a stuck
    // connector to one blocked thread however often it is asked.
    std::thread::spawn(move || {
        if let Ok(mut stdin) = handle.try_lock() {
            let _ = stdin.write_all(line.as_bytes());
            let _ = stdin.flush();
        }
    });
    Some(req)
}

/// The preview for that row, waiting up to `wait` for it to arrive. A
/// connector that never answers costs exactly this much and nothing more.
pub fn preview_of(block: &str, row: &str, req: u64, wait: Duration) -> Option<Preview> {
    let (slot, arrived) = latest();
    let held = slot.lock().unwrap_or_else(|err| err.into_inner());
    // Checks before it waits, so an answer already in the slot costs nothing,
    // and re-checks on every wake, so a spurious one does not end the wait.
    // An answer to an older ask is somebody else's, and arrives here when a
    // connector replies after its 400ms was up.
    let (held, expired) = arrived
        .wait_timeout_while(held, wait, |slot| {
            !matches!(
                slot.as_ref(),
                Some(found) if found.block == block && found.row == row && found.req >= req
            )
        })
        .unwrap_or_else(|err| err.into_inner());
    if expired.timed_out() {
        return None;
    }
    held.as_ref().map(|found| found.preview.clone())
}

fn store_preview(block: &str, row: &str, req: u64, content: Content, mut text: String) {
    let (slot, arrived) = latest();
    let mut held = slot.lock().unwrap_or_else(|err| err.into_inner());
    if text.len() > MAX_PREVIEW_BYTES {
        // Back off to a boundary: a cut mid-codepoint would panic.
        let at = (0..=MAX_PREVIEW_BYTES)
            .rev()
            .find(|at| text.is_char_boundary(*at))
            .unwrap_or(0);
        text.truncate(at);
    }
    *held = Some(Latest {
        block: block.to_string(),
        row: row.to_string(),
        req,
        preview: Preview {
            text,
            markdown: content == Content::Markdown,
        },
    });
    arrived.notify_all();
}

/// The blocks whose connector is running right now, for a screen that lists
/// what is installed and what is actually up.
pub fn running_ids() -> std::collections::BTreeSet<String> {
    let mut running = locked();
    running
        .iter_mut()
        .filter_map(|(id, alive)| matches!(alive.child.try_wait(), Ok(None)).then(|| id.clone()))
        .collect()
}

/// Ends every connector. The rows they sent stay cached, which is what a user
/// who quit mid-sync should see on the way back in.
pub fn stop() {
    let mut running = locked();
    for (_, mut alive) in std::mem::take(&mut *running) {
        end(&mut alive);
    }
}

/// Goodbye, then the whole process group, then reaped. A connector leads its
/// own session, so killing the shell alone leaves its children behind.
fn end(alive: &mut Running) {
    if let Some(handle) = alive.stdin.take()
        && let Ok(mut stdin) = handle.try_lock()
    {
        let _ = stdin.write_all(Ask::Bye.line().as_bytes());
        let _ = stdin.flush();
    }
    look_sources::end_connector(&mut alive.child);
}

fn spawn(id: &str, found: &Declared, on_rows: OnRows) -> Result<(Child, Option<Pipe>), String> {
    let mut child = look_sources::spawn_connector(&found.command, None)?;
    let stdout = child.stdout.take().ok_or("no stdout")?;

    // Kept, not dropped: closing it is EOF to a connector that reads stdin.
    let mut stdin = child.stdin.take();
    if let Some(stdin) = stdin.as_mut() {
        let hello = Ask::Hello {
            v: PROTOCOL_VERSION,
            look: env!("CARGO_PKG_VERSION").to_string(),
        };
        let _ = stdin.write_all(hello.line().as_bytes());
        let _ = stdin.flush();
    }

    // One reader per connector, for its lifetime. It ends when the child closes
    // its stdout, which is also how a crash is noticed.
    let owner = Owner {
        block: id.to_string(),
        manifest: found.manifest.clone(),
    };
    std::thread::spawn(move || read_frames(&owner, stdout, &on_rows));
    Ok((child, stdin.map(|stdin| Arc::new(Mutex::new(stdin)))))
}

/// Which block a connector is, and which namespace it may speak for.
struct Owner {
    block: String,
    manifest: String,
}

/// The largest frame a connector may send. `lines()` would read a line of any
/// length into memory, so a connector could hand Look a gigabyte and that would
/// be Look's problem. A frame past the cap is a protocol breach, not a big
/// message: the connector is dropped and restarted on next use.
const MAX_FRAME_BYTES: usize = 1 << 20;

fn read_frames(owner: &Owner, stdout: std::process::ChildStdout, on_rows: &OnRows) {
    let mut reader = BufReader::new(stdout);
    let mut frame = Vec::new();
    loop {
        frame.clear();
        let read = reader
            .by_ref()
            .take(MAX_FRAME_BYTES as u64)
            .read_until(b'\n', &mut frame);
        match read {
            Ok(0) => return,
            // No newline and the cap reached: the line is longer than allowed.
            // Short of the cap it is the last frame before the pipe closed.
            Ok(_) if frame.len() == MAX_FRAME_BYTES && !frame.ends_with(b"\n") => {
                eprintln!(
                    "look connectors: [{}] sent a frame over {MAX_FRAME_BYTES} bytes",
                    owner.block
                );
                return;
            }
            Ok(_) => {
                if let Some(say) = look_sources::read_frame(&String::from_utf8_lossy(&frame)) {
                    handle(owner, say, on_rows);
                }
            }
            Err(_) => return,
        }
    }
}

fn handle(owner: &Owner, say: Say, on_rows: &OnRows) {
    match say {
        Say::Rows { block, rows } => {
            // A connector may only speak for the blocks of its own manifest.
            // Taking any name it offered would let one app rewrite another's
            // rows, which is the same boundary `then` is held to.
            if !speaks_for(owner, &block) {
                eprintln!(
                    "look connectors: [{}] tried to send rows for [{block}]",
                    owner.block
                );
                return;
            }
            let rows: Vec<SourceRow> = rows.into_iter().map(SourceRow::from).collect();
            match store(&block, &rows) {
                Ok(()) => on_rows(&block),
                // An empty message is the same rows again, which is not news.
                Err(message) if message.is_empty() => {}
                Err(message) => eprintln!("look connectors: [{block}] {message}"),
            }
        }
        Say::Preview {
            block,
            id,
            req,
            content_type,
            text,
        } => {
            // The same boundary rows are held to: a connector may not answer
            // for another app's block.
            if !speaks_for(owner, &block) {
                return;
            }
            // Required, not optional: without it a late answer cannot be told
            // from this one's, and the protocol is new enough to insist.
            match req {
                Some(req) => store_preview(&block, &id, req, content_type, text),
                None => eprintln!(
                    "look connectors: [{block}] a preview with no `req` cannot be matched to an ask"
                ),
            }
        }
        Say::Ready { .. } => {}
        Say::Error { message } => {
            eprintln!("look connectors: [{}] {message}", owner.block)
        }
    }
}

/// Its own block, and others from the same manifest, so one process can serve
/// every block in its file. The manifest is carried rather than cut out of the
/// id: both a stem (`com.example.app`) and a quoted header may hold dots, so
/// no single dot separates them.
fn speaks_for(owner: &Owner, block: &str) -> bool {
    if owner.block == block {
        return true;
    }
    let namespace = format!("{}{NAMESPACE}", owner.manifest);
    owner.block.starts_with(&namespace) && block.starts_with(&namespace)
}

/// As `load_connectors` joins manifest to block id.
const NAMESPACE: char = '.';

/// Through the row cache, so connector rows are indexed, ranked and kept
/// exactly like a `run` block's.
fn store(block: &str, rows: &[SourceRow]) -> Result<(), String> {
    // Capped here rather than on the way out of the cache: writing rows that
    // will never be read costs the user disk for nothing, and a connector is
    // the one producer whose output nothing else bounds.
    let json: Vec<serde_json::Value> = rows
        .iter()
        .take(look_sources::MAX_ROWS_PER_SOURCE)
        .map(|row| {
            serde_json::json!({
                "id": row.id,
                "title": row.title,
                "subtitle": row.subtitle,
                "path": row.path,
                "icon": row.icon,
            })
        })
        .collect();
    let text = serde_json::to_string(&json).map_err(|err| err.to_string())?;
    // The same rows again is the normal state of a connector watching
    // something unchanged, and should cost nothing beyond this comparison.
    if crate::index::run_cache_holds(block, &text) {
        return Err(String::new());
    }
    crate::index::store_counted_rows(block, &text, json.len()).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The preview slot is global, and tests run in parallel. Without this a
    /// store from one test answers another test's read.
    fn one_at_a_time() -> std::sync::MutexGuard<'static, ()> {
        static ALONE: OnceLock<Mutex<()>> = OnceLock::new();
        ALONE
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|err| err.into_inner())
    }

    fn owner(manifest: &str, block: &str) -> Owner {
        Owner {
            block: block.to_string(),
            manifest: manifest.to_string(),
        }
    }

    #[test]
    fn a_stored_preview_is_returned_to_the_row_that_asked() {
        let _alone = one_at_a_time();
        store_preview("myapp.notes", "n1", 1, Content::Markdown, "# Title".into());
        let found = preview_of("myapp.notes", "n1", 1, Duration::from_millis(0)).expect("preview");
        assert_eq!(found.text, "# Title");
        assert!(found.markdown);
        // The slot holds one answer, and it is not this row's.
        assert!(preview_of("myapp.notes", "n2", 1, Duration::from_millis(0)).is_none());
        assert!(preview_of("other.notes", "n1", 1, Duration::from_millis(0)).is_none());
    }

    #[test]
    fn a_text_preview_is_not_markdown() {
        let _alone = one_at_a_time();
        store_preview("b", "r", 1, Content::Text, "plain".into());
        let found = preview_of("b", "r", 1, Duration::from_millis(0)).expect("preview");
        assert!(!found.markdown);
    }

    #[test]
    fn a_preview_is_truncated_on_a_character_boundary() {
        let _alone = one_at_a_time();
        // A cut mid-codepoint would panic on truncate, so the cap has to look
        // for a boundary rather than trust the byte count.
        let long = "é".repeat(MAX_PREVIEW_BYTES);
        store_preview("b", "big", 1, Content::Text, long);
        let found = preview_of("b", "big", 1, Duration::from_millis(0)).expect("preview");
        assert!(found.text.len() <= MAX_PREVIEW_BYTES);
        assert!(!found.text.is_empty());
    }

    #[test]
    fn waiting_for_a_preview_that_never_comes_gives_up() {
        let _alone = one_at_a_time();
        store_preview("b", "other", 1, Content::Text, "x".into());
        let began = std::time::Instant::now();
        assert!(preview_of("b", "never", 1, Duration::from_millis(30)).is_none());
        assert!(began.elapsed() >= Duration::from_millis(25));
    }

    #[test]
    fn asking_again_does_not_hand_back_the_last_answer() {
        let _alone = one_at_a_time();
        store_preview("b", "r", 1, Content::Text, "old".into());
        ask_preview("b", "r");
        assert!(preview_of("b", "r", 1, Duration::from_millis(0)).is_none());
    }

    #[test]
    fn a_late_answer_to_an_earlier_ask_is_not_taken_for_this_one() {
        let _alone = one_at_a_time();
        // Ask 1 timed out, ask 2 went out, then 1's answer finally arrived.
        store_preview("b", "r", 1, Content::Text, "stale".into());
        assert!(preview_of("b", "r", 2, Duration::from_millis(10)).is_none());
        // 2's own answer is taken.
        store_preview("b", "r", 2, Content::Text, "fresh".into());
        let found = preview_of("b", "r", 2, Duration::from_millis(0)).expect("preview");
        assert_eq!(found.text, "fresh");
    }

    #[test]
    fn asking_a_block_with_no_connector_fails_rather_than_waits() {
        assert!(ask_preview("nothing.running", "r1").is_none());
    }

    #[test]
    fn a_connector_speaks_only_for_its_own_manifest() {
        let myapp = owner("myapp", "myapp.notes");
        assert!(speaks_for(&myapp, "myapp.notes"));
        assert!(speaks_for(&myapp, "myapp.tags"));
        assert!(!speaks_for(&myapp, "other.notes"));
        // A block the user wrote has no namespace, so nothing reaches it.
        assert!(!speaks_for(&myapp, "projects"));
        assert!(!speaks_for(&owner("mine", "projects"), "myapp.notes"));
    }

    #[test]
    fn a_dotted_manifest_name_is_one_namespace_not_its_first_word() {
        // `com.example.app` and `com.other` share a first word and nothing
        // else. Cutting at the first dot let one send the other's rows.
        let app = owner("com.example.app", "com.example.app.notes");
        assert!(speaks_for(&app, "com.example.app.tags"));
        assert!(!speaks_for(&app, "com.other.tags"));
        assert!(!speaks_for(&app, "com.example.appstore.notes"));
    }

    #[test]
    fn a_quoted_block_id_may_hold_dots_of_its_own() {
        // `["notes.v2"]` under `myapp.toml` is `myapp.notes.v2`, so the LAST
        // dot does not separate manifest from block either.
        let app = owner("myapp", "myapp.notes.v2");
        assert!(speaks_for(&app, "myapp.notes.v2"));
        assert!(speaks_for(&app, "myapp.tags"));
        // Still the same manifest, dots in the block id or not.
        assert!(speaks_for(&app, "myapp.notes.other"));
        assert!(!speaks_for(&app, "myapp2.notes"));
    }
}
