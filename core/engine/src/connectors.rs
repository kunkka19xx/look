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

/// How long a burst is allowed to settle before its rows are read, and the
/// floor between two reads however often rows arrive.
const SETTLE: Duration = Duration::from_millis(300);
const FLOOR: Duration = Duration::from_secs(2);
/// How soon to try again when another refresh holds the slot, and how many
/// such tries before backing off to the floor.
const BUSY_RETRY: Duration = Duration::from_millis(250);
const BUSY_PATIENCE: u32 = 8;

/// An [`OnRows`] that reads the sources soon, and at most once per `FLOOR`:
/// a burst of pushes costs one read. `refresh` says whether it could read;
/// `false` means the slot was busy and the read is retried, since nothing
/// else will ask for the rows. One per process, like the index.
pub fn coalesced(refresh: impl Fn() -> bool + Send + Sync + 'static) -> OnRows {
    let refresh = Arc::new(refresh);
    Arc::new(move |_block: &str| schedule_read(refresh.clone()))
}

/// Queues one read after `SETTLE`, folding every push that lands meanwhile
/// into it.
fn schedule_read(refresh: Arc<dyn Fn() -> bool + Send + Sync>) {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Instant;

    static QUEUED: AtomicBool = AtomicBool::new(false);
    static ARRIVED: AtomicBool = AtomicBool::new(false);
    static NEXT_ALLOWED: Mutex<Option<Instant>> = Mutex::new(None);

    /// How long until another read is allowed, never less than the settle.
    fn until_allowed() -> Duration {
        let next = NEXT_ALLOWED.lock().unwrap_or_else(|err| err.into_inner());
        next.map(|at| at.saturating_duration_since(Instant::now()))
            .unwrap_or_default()
            .max(SETTLE)
    }

    // Before the queue check, so rows landing mid-read still count.
    ARRIVED.store(true, Ordering::Release);
    if QUEUED.swap(true, Ordering::AcqRel) {
        return;
    }

    std::thread::spawn(move || {
        let mut wait = until_allowed();
        let mut busy = 0_u32;
        loop {
            std::thread::sleep(wait);

            // Before the read, never after: a push during it sets this again.
            ARRIVED.store(false, Ordering::Release);

            if !refresh() {
                busy += 1;
                wait = if busy > BUSY_PATIENCE {
                    FLOOR
                } else {
                    BUSY_RETRY
                };
                ARRIVED.store(true, Ordering::Release);
                continue;
            }
            busy = 0;

            {
                let mut next = NEXT_ALLOWED.lock().unwrap_or_else(|err| err.into_inner());
                *next = Some(Instant::now() + FLOOR);
            }
            // Released before the check, so a push between the two is not lost.
            QUEUED.store(false, Ordering::Release);
            if !ARRIVED.load(Ordering::Acquire) {
                return;
            }
            if QUEUED.swap(true, Ordering::AcqRel) {
                return;
            }
            wait = until_allowed();
        }
    });
}

/// Recent trouble per block, for a user who did not launch from a terminal.
/// A connector's failures are otherwise invisible in the app.
const LOG_LINES: usize = 20;
/// A connector may name any block in its own namespace, declared or not.
const LOG_BLOCKS: usize = 64;

fn logs() -> &'static Mutex<BTreeMap<String, std::collections::VecDeque<String>>> {
    static LOGS: OnceLock<Mutex<BTreeMap<String, std::collections::VecDeque<String>>>> =
        OnceLock::new();
    LOGS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

/// Says it to stderr and to the Extend screen.
///
/// Not `eprintln!`, which panics when the sink fails: the caller owns a
/// connector's pipe, and unwinding would close it for a log line.
fn note(block: &str, message: &str) {
    let _ = writeln!(std::io::stderr(), "look connectors: [{block}] {message}");
    // An `error` frame may carry nearly a megabyte; stderr above kept it all.
    let mut end = message.len().min(MAX_LOG_LINE);
    while end > 0 && !message.is_char_boundary(end) {
        end -= 1;
    }
    let message = &message[..end];
    let mut logs = logs().lock().unwrap_or_else(|err| err.into_inner());
    if logs.len() >= LOG_BLOCKS && !logs.contains_key(block) {
        return;
    }
    let lines = logs.entry(block.to_string()).or_default();
    if lines.len() == LOG_LINES {
        lines.pop_front();
    }
    lines.push_back(message.to_string());
}

/// What went wrong for this block lately, oldest first.
pub fn log_of(block: &str) -> Vec<String> {
    logs()
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .get(block)
        .map(|lines| lines.iter().cloned().collect())
        .unwrap_or_default()
}

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
    /// A connector that dies on start is otherwise respawned on every open.
    attempts: u32,
    launched_at: std::time::Instant,
}

/// Wait before relaunching, by launches behind it. The first death restarts at
/// once, as "restarted on next use" promises; a crash loop backs off.
const BACKOFF: [Duration; 5] = [
    Duration::ZERO,
    Duration::from_secs(5),
    Duration::from_secs(30),
    Duration::from_secs(120),
    Duration::from_secs(600),
];

fn backoff(attempts: u32) -> Duration {
    BACKOFF[(attempts.max(1) as usize - 1).min(BACKOFF.len() - 1)]
}

/// Set by `stop`, cleared by `start`: a respawn pass in flight at quit
/// launches nothing.
static STOPPING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

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

    STOPPING.store(false, std::sync::atomic::Ordering::Release);
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
            launch(&mut running, id, found, &on_rows, 0);
        }
    }
}

/// A connector as its block declares it.
struct Declared {
    command: String,
    manifest: String,
}

/// Respawns connectors that died, and nothing else. Reads no directories, so
/// the shell can call it on every open; a manifest installed since launch is
/// what `start` is for. Returns at once, since spawning is not the opening
/// thread's work, and runs one pass at a time.
pub fn restart_dead(on_rows: OnRows) {
    use std::sync::atomic::{AtomicBool, Ordering};

    static PASS: AtomicBool = AtomicBool::new(false);

    if PASS.swap(true, Ordering::AcqRel) {
        return;
    }
    std::thread::spawn(move || {
        respawn(&on_rows);
        PASS.store(false, Ordering::Release);
    });
}

/// One pass. A connector inside its backoff is left dead, so `running_ids`
/// keeps reporting it down and the next open asks again.
fn respawn(on_rows: &OnRows) {
    let mut running = locked();
    if STOPPING.load(std::sync::atomic::Ordering::Acquire) {
        return;
    }

    let mut dead: Vec<(String, Declared, u32)> = Vec::new();
    for (id, alive) in running.iter_mut() {
        let exited = matches!(alive.child.try_wait(), Ok(Some(_)) | Err(_));
        if !exited || alive.launched_at.elapsed() < backoff(alive.attempts) {
            continue;
        }
        dead.push((
            id.clone(),
            Declared {
                command: alive.command.clone(),
                manifest: alive.manifest.clone(),
            },
            alive.attempts,
        ));
    }

    for (id, found, attempts) in dead {
        if let Some(mut gone) = running.remove(&id) {
            end(&mut gone);
        }
        launch(&mut running, id, found, on_rows, attempts);
    }
}

fn locked() -> std::sync::MutexGuard<'static, BTreeMap<String, Running>> {
    running().lock().unwrap_or_else(|err| err.into_inner())
}

/// Spawns one and records it. A connector that refuses to run is reported and
/// left out rather than retried in a loop.
fn launch(
    running: &mut BTreeMap<String, Running>,
    id: String,
    found: Declared,
    on_rows: &OnRows,
    attempts: u32,
) {
    match spawn(&id, &found, on_rows.clone()) {
        Ok((child, stdin)) => {
            running.insert(
                id,
                Running {
                    child,
                    stdin,
                    command: found.command,
                    manifest: found.manifest,
                    attempts: attempts + 1,
                    launched_at: std::time::Instant::now(),
                },
            );
        }
        Err(message) => note(&id, &message),
    }
}

/// A preview a connector sent, and what its text is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preview {
    pub text: String,
    pub markdown: bool,
}

/// What a verb would do, as the connector worked it out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Planned {
    pub title: String,
    pub detail: Option<String>,
}

/// How running one went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub ok: bool,
    pub summary: Option<String>,
}

/// Answers waiting to be collected, by the number of the ask that wanted them.
///
/// A map, not one slot: a press and a control read are in flight at once when
/// the selection changes mid-press, and whichever answers first must not
/// decide the other. A press reported as "no answer" is pressed again and
/// flipped back. The number is unique per process, so no block keying either.
struct Slot<T> {
    held: Mutex<Waiting<T>>,
    arrived: Condvar,
}

struct Waiting<T> {
    answers: BTreeMap<u64, T>,
    /// Asks still inside `take`. Never evicted, however old.
    claimed: std::collections::BTreeSet<u64>,
}

/// A caller that timed out never comes back, but its connector may still
/// answer, so uncollected answers are bounded.
const MAX_WAITING: usize = 32;

impl<T> Slot<T> {
    const fn new() -> Self {
        Self {
            held: Mutex::new(Waiting {
                answers: BTreeMap::new(),
                claimed: std::collections::BTreeSet::new(),
            }),
            arrived: Condvar::new(),
        }
    }

    fn put(&self, req: u64, value: T) {
        let mut held = self.held.lock().unwrap_or_else(|err| err.into_inner());
        // Split so the scan and the removal borrow separately.
        let Waiting { answers, claimed } = &mut *held;
        while answers.len() >= MAX_WAITING {
            // Lowest unclaimed: numbers only go up, so that caller gave up
            // longest ago. Skipping the claimed is what protects a press.
            let Some(stale) = answers.keys().copied().find(|req| !claimed.contains(req)) else {
                break;
            };
            answers.remove(&stale);
        }
        answers.insert(req, value);
        self.arrived.notify_all();
    }

    /// Waits up to `wait` for the answer to `req` and takes it. Removed on a
    /// timeout too, so a late answer is not left for anyone else.
    fn take(&self, req: u64, wait: Duration) -> Option<T> {
        let mut held = self.held.lock().unwrap_or_else(|err| err.into_inner());
        // Before the wait: an answer arriving while asleep must not be evicted.
        held.claimed.insert(req);
        // Checks before waiting, and on every wake so a spurious one does not
        // end it.
        let (mut held, _) = self
            .arrived
            .wait_timeout_while(held, wait, |held| !held.answers.contains_key(&req))
            .unwrap_or_else(|err| err.into_inner());
        held.claimed.remove(&req);
        held.answers.remove(&req)
    }
}

static PREVIEWS: Slot<Preview> = Slot::new();
static PLANS: Slot<Planned> = Slot::new();
static RESULTS: Slot<Outcome> = Slot::new();
static STATES: Slot<bool> = Slot::new();

/// One counter for every ask, so a number never means two questions.
fn next_req() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// A preview past this is truncated. The frame cap already bounds one message;
/// this bounds what a panel is asked to lay out.
const MAX_PREVIEW_BYTES: usize = 64 * 1024;

/// Writes one frame to the connector behind `block`, off this thread. False
/// when none is running, which is the caller's cue not to wait for an answer.
///
/// Never blocks the caller: a connector that stopped reading its stdin fills
/// its pipe, and the write would otherwise block the panel before the wait
/// that is supposed to bound it even starts.
///
/// `must_arrive` says what to do when another frame holds the pipe. A preview
/// gives up, since one is sent per selection and a lost one costs nothing; a
/// verb waits, because a dropped `run` would be reported as a run that
/// happened. Verbs are rare and user-initiated, so the thread is too.
fn tell(block: &str, ask: Ask, must_arrive: bool) -> bool {
    let Some(handle) = locked().get(block).and_then(|alive| alive.stdin.clone()) else {
        return false;
    };
    let line = ask.line();
    std::thread::spawn(move || {
        WRITERS.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        let pipe = match must_arrive {
            true => Some(handle.lock().unwrap_or_else(|err| err.into_inner())),
            false => handle.try_lock().ok(),
        };
        if let Some(mut stdin) = pipe {
            let _ = stdin.write_all(line.as_bytes());
            let _ = stdin.flush();
        }
        WRITERS.fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
    });
    true
}

static WRITERS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Threads currently writing a frame to a connector. Watched by the
/// wedged-connector test; nothing in the app reads it. At most one sticks: it
/// holds the pipe and every frame after it gives up rather than queueing.
pub fn writers_in_flight() -> usize {
    WRITERS.load(std::sync::atomic::Ordering::Acquire)
}

/// Asks for a row's preview, and gives back the number to wait for.
pub fn ask_preview(block: &str, row: &str) -> Option<u64> {
    let req = next_req();
    tell(
        block,
        Ask::Preview {
            block: block.to_string(),
            id: row.to_string(),
            req,
        },
        false,
    )
    .then_some(req)
}

pub fn preview_of(req: u64, wait: Duration) -> Option<Preview> {
    PREVIEWS.take(req, wait)
}

/// Asks what `verb` would do to those rows, and gives back the plan number.
pub fn ask_plan(block: &str, verb: &str, ids: Vec<String>) -> Option<u64> {
    let plan = next_req();
    tell(
        block,
        Ask::Plan {
            block: block.to_string(),
            verb: verb.to_string(),
            ids,
            plan,
        },
        true,
    )
    .then_some(plan)
}

pub fn planned_of(plan: u64, wait: Duration) -> Option<Planned> {
    PLANS.take(plan, wait)
}

/// Runs a plan the connector already described. The same number: it is the
/// plan the user confirmed, not a new question.
pub fn run_plan(block: &str, plan: u64) -> bool {
    tell(
        block,
        Ask::Run {
            block: block.to_string(),
            plan,
        },
        true,
    )
}

pub fn result_of(plan: u64, wait: Duration) -> Option<Outcome> {
    RESULTS.take(plan, wait)
}

/// Asks where a control stands, or presses it, and gives back the number to
/// wait for. One call for both: a connector answers a press with where it
/// ended up, so both wait on the same slot for the same answer.
pub fn ask_control(block: &str, control: &str, press: bool) -> Option<u64> {
    let req = next_req();
    let (block, control) = (block.to_string(), control.to_string());
    let ask = if press {
        Ask::Press {
            block: block.clone(),
            control,
            req,
        }
    } else {
        Ask::State {
            block: block.clone(),
            control,
            req,
        }
    };
    // A read like a preview, a press like a verb: waiting for the pipe on a
    // read leaks a thread per selection against a connector gone deaf.
    tell(&block, ask, press).then_some(req)
}

pub fn state_of(req: u64, wait: Duration) -> Option<bool> {
    STATES.take(req, wait)
}

fn store_preview(req: u64, content: Content, mut text: String) {
    if text.len() > MAX_PREVIEW_BYTES {
        // Back off to a boundary: a cut mid-codepoint would panic.
        let at = (0..=MAX_PREVIEW_BYTES)
            .rev()
            .find(|at| text.is_char_boundary(*at))
            .unwrap_or(0);
        text.truncate(at);
    }
    PREVIEWS.put(
        req,
        Preview {
            text,
            markdown: content == Content::Markdown,
        },
    );
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
    STOPPING.store(true, std::sync::atomic::Ordering::Release);
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
    let stderr = child.stderr.take();

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

    // Its own thread: a connector that logs without reading its stdin would
    // otherwise fill this pipe and wedge waiting for the other to drain.
    if let Some(stderr) = stderr {
        let block = id.to_string();
        std::thread::spawn(move || {
            log_lines(stderr, |line| note(&block, line));
        });
    }

    std::thread::spawn(move || read_frames(&owner, stdout, &on_rows));
    Ok((child, stdin.map(|stdin| Arc::new(Mutex::new(stdin)))))
}

/// A connector's stderr, line by line. Takes any `Read` so the splitting can
/// be tested without a child process.
fn log_lines(source: impl Read, mut say: impl FnMut(&str)) {
    let mut reader = BufReader::new(source);
    let mut line = Vec::new();
    loop {
        line.clear();
        // A long line is cut and continues on the next one. Giving up on the
        // pipe, the way an over-long frame does, would lose the rest of the log.
        match reader
            .by_ref()
            .take(MAX_LOG_LINE as u64)
            .read_until(b'\n', &mut line)
        {
            Ok(0) | Err(_) => return,
            Ok(_) => match String::from_utf8_lossy(&line).trim_end() {
                "" => {}
                text => say(text),
            },
        }
    }
}

const MAX_LOG_LINE: usize = 4096;

/// Which block a connector is, and which namespace it may speak for.
struct Owner {
    block: String,
    manifest: String,
}

/// The largest frame a connector may send, not counting the newline that ends
/// it. `lines()` would read a line of any length into memory, so a connector
/// could hand Look a gigabyte and that would be Look's problem. A frame past
/// the cap is a protocol breach, not a big message: the connector is dropped
/// and restarted on next use.
pub const MAX_FRAME_BYTES: usize = 1 << 20;

fn read_frames(owner: &Owner, stdout: std::process::ChildStdout, on_rows: &OnRows) {
    let mut reader = BufReader::new(stdout);
    let mut frame = Vec::new();
    loop {
        frame.clear();
        // One past the cap, so a frame of exactly `MAX_FRAME_BYTES` still has
        // room for its newline. Budgeting the delimiter inside the cap would
        // reject a frame the contract allows.
        let read = reader
            .by_ref()
            .take(MAX_FRAME_BYTES as u64 + 1)
            .read_until(b'\n', &mut frame);
        match read {
            Ok(0) => return,
            // No newline and past the cap: the line is longer than allowed.
            // Short of it, this is the last frame before the pipe closed.
            Ok(_) if frame.len() > MAX_FRAME_BYTES && !frame.ends_with(b"\n") => {
                note(
                    &owner.block,
                    &format!("sent a frame over {MAX_FRAME_BYTES} bytes"),
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
                note(&owner.block, &format!("tried to send rows for [{block}]"));
                return;
            }
            let rows: Vec<SourceRow> = rows.into_iter().map(SourceRow::from).collect();
            match store(&block, &rows) {
                Ok(()) => on_rows(&block),
                // An empty message is the same rows again, which is not news.
                Err(message) if message.is_empty() => {}
                Err(message) => note(&block, &message),
            }
        }
        Say::Preview {
            block,
            req,
            content_type,
            text,
            ..
        } => answer(owner, &block, req, "preview", |req| {
            store_preview(req, content_type, text)
        }),
        Say::Planned {
            block,
            plan,
            title,
            detail,
        } => answer(owner, &block, plan, "planned", |plan| {
            PLANS.put(plan, Planned { title, detail })
        }),
        Say::Result {
            block,
            plan,
            ok,
            summary,
        } => answer(owner, &block, plan, "result", |plan| {
            RESULTS.put(plan, Outcome { ok, summary })
        }),
        Say::State { block, req, on, .. } => {
            answer(owner, &block, req, "state", |req| STATES.put(req, on))
        }
        Say::Ready { .. } => {}
        Say::Error { message } => note(&owner.block, &message),
    }
}

/// The boundary and the number, which every answer has to clear: a connector
/// may only speak for its own manifest, and an answer with no number cannot be
/// told from a late reply to an older ask.
fn answer(owner: &Owner, block: &str, req: Option<u64>, what: &str, store: impl FnOnce(u64)) {
    if !speaks_for(owner, block) {
        note(&owner.block, &format!("tried to answer for [{block}]"));
        return;
    }
    match req {
        Some(req) => store(req),
        None => note(
            block,
            &format!("a {what} with no number cannot be matched to an ask"),
        ),
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
    // The normal state of a connector watching something unchanged, and it
    // costs about 30us for a full block. Hashing instead of reading the file
    // back was tried and is slower: see `connector_push`.
    if crate::index::run_cache_holds(block, &text) {
        return Err(String::new());
    }
    crate::index::store_counted_rows(block, &text, json.len()).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn logged(bytes: &[u8]) -> Vec<String> {
        let mut lines = Vec::new();
        log_lines(bytes, |line| lines.push(line.to_string()));
        lines
    }

    #[test]
    fn a_connectors_log_is_split_into_lines_worth_printing() {
        // Blank lines carry nothing, CRLF and padding are formatting, and a
        // last line with no newline is what a dying connector leaves behind.
        assert_eq!(
            logged(b"up\n\n\nwindows\r\npad   \ndied"),
            ["up", "windows", "pad", "died"]
        );
        assert!(logged(b"").is_empty());
        // Bytes, not text: a panic here would kill the thread.
        assert_eq!(logged(b"caf\xff\n"), ["caf\u{fffd}"]);

        // A long line is cut and the log carries on past it.
        let mut long = vec![b'x'; MAX_LOG_LINE + 10];
        long.extend_from_slice(b"\nstill here\n");
        let lines = logged(&long);
        assert_eq!(lines[0].len(), MAX_LOG_LINE);
        assert_eq!(lines.last().unwrap(), "still here");
    }

    /// The preview slot is global, and tests run in parallel. Without this a
    /// store from one test answers another test's read.
    fn one_at_a_time() -> std::sync::MutexGuard<'static, ()> {
        static ALONE: OnceLock<Mutex<()>> = OnceLock::new();
        let guard = ALONE
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        // These tests use literal numbers where the real asks use a counter.
        for drain in [
            &PREVIEWS.held as &Mutex<dyn Drain>,
            &PLANS.held,
            &RESULTS.held,
            &STATES.held,
        ] {
            drain.lock().unwrap_or_else(|err| err.into_inner()).empty();
        }
        guard
    }

    /// Four maps of different value types, without `Slot` growing a method
    /// only the tests want.
    trait Drain {
        fn empty(&mut self);
    }

    impl<T> Drain for Waiting<T> {
        fn empty(&mut self) {
            self.answers.clear();
            self.claimed.clear();
        }
    }

    fn owner(manifest: &str, block: &str) -> Owner {
        Owner {
            block: block.to_string(),
            manifest: manifest.to_string(),
        }
    }

    #[test]
    fn a_stored_answer_goes_to_the_ask_that_wanted_it() {
        let _alone = one_at_a_time();
        store_preview(1, Content::Markdown, "# Title".into());
        let found = preview_of(1, Duration::from_millis(0)).expect("preview");
        assert_eq!(found.text, "# Title");
        assert!(found.markdown);
    }

    #[test]
    fn a_text_preview_is_not_markdown() {
        let _alone = one_at_a_time();
        store_preview(1, Content::Text, "plain".into());
        assert!(
            !preview_of(1, Duration::from_millis(0))
                .expect("preview")
                .markdown
        );
    }

    #[test]
    fn a_preview_is_truncated_on_a_character_boundary() {
        let _alone = one_at_a_time();
        // A cut mid-codepoint would panic on truncate, so the cap has to look
        // for a boundary rather than trust the byte count.
        store_preview(1, Content::Text, "é".repeat(MAX_PREVIEW_BYTES));
        let found = preview_of(1, Duration::from_millis(0)).expect("preview");
        assert!(found.text.len() <= MAX_PREVIEW_BYTES);
        assert!(!found.text.is_empty());
    }

    #[test]
    fn waiting_for_an_answer_that_never_comes_gives_up() {
        let _alone = one_at_a_time();
        let began = std::time::Instant::now();
        assert!(preview_of(1, Duration::from_millis(30)).is_none());
        assert!(began.elapsed() >= Duration::from_millis(25));
    }

    #[test]
    fn an_answer_is_collected_once() {
        // Numbers come from a counter, so there is no identical question to
        // guard against. What matters is that collecting an answer takes it.
        let _alone = one_at_a_time();
        store_preview(1, Content::Text, "mine".into());
        assert_eq!(
            preview_of(1, Duration::from_millis(0))
                .expect("preview")
                .text,
            "mine"
        );
        assert!(preview_of(1, Duration::from_millis(0)).is_none());
    }

    #[test]
    fn a_press_and_a_read_in_flight_together_each_get_their_own_answer() {
        // A selection changing mid-press. One shared slot let the read's
        // answer evict the press's, and `control` then reported "did not
        // answer" for a press that happened.
        let _alone = one_at_a_time();
        STATES.put(11, true);
        STATES.put(10, false);
        assert_eq!(state_of(11, Duration::from_millis(0)), Some(true));
        assert_eq!(
            state_of(10, Duration::from_millis(0)),
            Some(false),
            "the press lost its answer to a read that answered first"
        );
    }

    #[test]
    fn a_claimed_answer_is_never_evicted() {
        // The press's answer is the lowest number, so eviction by age alone
        // drops it for reads a slow connector is still answering.
        //
        // The invariant, not the race: forcing 32 puts between one
        // `notify_all` and the waiter re-taking the lock is not reliable.
        let _alone = one_at_a_time();
        let press = 1_u64;
        // As `take` does before it sleeps.
        STATES
            .held
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .claimed
            .insert(press);
        STATES.put(press, true);
        for req in 2..(MAX_WAITING as u64 * 3) {
            STATES.put(req, false);
        }

        let held = STATES.held.lock().unwrap_or_else(|err| err.into_inner());
        assert!(
            held.answers.contains_key(&press),
            "the press lost its answer to eviction"
        );
        assert!(held.answers.len() <= MAX_WAITING);
    }

    #[test]
    fn answers_nobody_came_back_for_do_not_pile_up() {
        // A timed-out ask may still be answered, and nothing collects those.
        let _alone = one_at_a_time();
        for req in 0..(MAX_WAITING as u64 * 3) {
            STATES.put(req, true);
        }
        let held = STATES.held.lock().unwrap_or_else(|err| err.into_inner());
        let kept = held.answers.len();
        assert!(kept <= MAX_WAITING, "{kept} answers held");
        // The newest survive: the oldest caller gave up longest ago.
        assert!(held.answers.contains_key(&(MAX_WAITING as u64 * 3 - 1)));
    }

    #[test]
    fn a_late_answer_to_an_earlier_ask_is_not_taken_for_this_one() {
        let _alone = one_at_a_time();
        // Ask 1 timed out, ask 2 went out, then 1's answer finally arrived.
        store_preview(1, Content::Text, "stale".into());
        assert!(preview_of(2, Duration::from_millis(10)).is_none());
        store_preview(2, Content::Text, "fresh".into());
        assert_eq!(
            preview_of(2, Duration::from_millis(0))
                .expect("preview")
                .text,
            "fresh"
        );
    }

    /// The same slot discipline, for the two action answers.
    #[test]
    fn a_plan_and_its_result_wait_on_their_own_number() {
        let _alone = one_at_a_time();
        PLANS.put(
            4,
            Planned {
                title: "Archive 1 note".into(),
                detail: Some("a.md".into()),
            },
        );
        assert!(planned_of(5, Duration::from_millis(10)).is_none());
        let found = planned_of(4, Duration::from_millis(0)).expect("plan");
        assert_eq!(found.title, "Archive 1 note");

        RESULTS.put(
            4,
            Outcome {
                ok: true,
                summary: Some("Archived 1 note".into()),
            },
        );
        assert!(result_of(4, Duration::from_millis(0)).expect("result").ok);
    }

    #[test]
    fn asking_a_block_with_no_connector_fails_rather_than_waits() {
        // Asking clears a slot even when nothing is running.
        let _alone = one_at_a_time();
        assert!(ask_preview("nothing.running", "r1").is_none());
        assert!(ask_plan("nothing.running", "archive", vec!["r1".into()]).is_none());
        assert!(!run_plan("nothing.running", 1));
    }

    #[test]
    fn a_later_answer_is_not_evicted_by_an_earlier_one() {
        // A second connector's late reply must not take an answer a waiter
        // is about to read.
        let _alone = one_at_a_time();
        store_preview(5, Content::Text, "mine".into());
        store_preview(3, Content::Text, "somebody else's, and late".into());
        assert_eq!(
            preview_of(5, Duration::from_millis(0))
                .expect("preview")
                .text,
            "mine"
        );
    }

    #[test]
    fn a_crash_loop_waits_longer_every_time() {
        // The first death restarts at once; past that it backs off.
        assert_eq!(backoff(1), Duration::ZERO);
        assert!(backoff(2) > Duration::ZERO);
        assert!(backoff(3) > backoff(2));
        // Never panics past the table: a connector can die any number of times.
        assert_eq!(backoff(99), *BACKOFF.last().expect("a backoff"));
        // Never 0 for a launched connector, but the floor keeps it in range.
        assert_eq!(backoff(0), Duration::ZERO);
    }

    #[test]
    fn trouble_is_kept_per_block_and_bounded() {
        note("myapp.notes", "first");
        note("myapp.notes", "second");
        let log = log_of("myapp.notes");
        assert_eq!(log.last().map(String::as_str), Some("second"));
        assert!(log.contains(&"first".to_string()));
        // A connector that fails in a loop must not grow this without end.
        for line in 0..LOG_LINES * 2 {
            note("myapp.noisy", &format!("line {line}"));
        }
        assert_eq!(log_of("myapp.noisy").len(), LOG_LINES);
        // Oldest first, so the ring dropped the front and not the back.
        assert_eq!(
            log_of("myapp.noisy").last().expect("a line"),
            &format!("line {}", LOG_LINES * 2 - 1)
        );
        assert!(log_of("myapp.never-complained").is_empty());

        // The cut must land on a character boundary: with a 3-byte character
        // the cap itself falls mid-character, which would panic on a slice.
        note("myapp.loud", &"\u{20ac}".repeat(MAX_LOG_LINE));
        let loud = log_of("myapp.loud");
        assert!(
            loud[0].len() <= MAX_LOG_LINE,
            "{} bytes kept",
            loud[0].len()
        );
        assert!(!loud[0].is_empty());
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
