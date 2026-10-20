//! The processes behind `connect` blocks: one child per block, kept alive,
//! speaking the frames in `look_sources::connect`.
//!
//! Rows it sends go to the same cache a `run` block uses, so indexing, ranking
//! and the last-good-rows rule all stay where they are. Nothing here blocks a
//! search: a connector that is slow, silent or dead simply has no new rows.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::Child;
use std::sync::{Arc, Mutex, OnceLock};

use look_sources::{Ask, PROTOCOL_VERSION, Producer, Say, SourceRow};

/// Told the block id whenever a connector sends rows, so the shell can mark its
/// index dirty. A callback rather than a flag here: the engine does not own the
/// index's freshness, the shell does.
pub type OnRows = Arc<dyn Fn(&str) + Send + Sync>;

struct Running {
    child: Child,
    /// What the block declared, to tell a restart from a changed command.
    command: String,
}

fn running() -> &'static Mutex<BTreeMap<String, Running>> {
    static RUNNING: OnceLock<Mutex<BTreeMap<String, Running>>> = OnceLock::new();
    RUNNING.get_or_init(|| Mutex::new(BTreeMap::new()))
}

/// Starts every declared connector not already running, and stops any whose
/// block is gone or whose command changed. Safe to call repeatedly: it is how
/// a reload reaches connectors.
pub fn start(home: &Path, on_rows: OnRows) {
    let declared: BTreeMap<String, String> = crate::sources::declared(home)
        .blocks
        .into_iter()
        .filter(|block| block.enabled)
        .filter_map(|block| match block.producer {
            Producer::Connect { command } => Some((block.id, command)),
            _ => None,
        })
        .collect();

    let mut running = locked();

    running.retain(|id, alive| {
        let still_declared = declared.get(id) == Some(&alive.command);
        let exited = matches!(alive.child.try_wait(), Ok(Some(_)) | Err(_));
        if !still_declared || exited {
            let _ = alive.child.kill();
            return false;
        }
        true
    });

    for (id, command) in declared {
        if !running.contains_key(&id) {
            launch(&mut running, id, command, &on_rows);
        }
    }
}

/// Respawns connectors that died, and nothing else. Reads no directories, so
/// the shell can call it every time the launcher opens; noticing a manifest
/// installed since launch is what `start` is for.
pub fn restart_dead(on_rows: OnRows) {
    let mut running = locked();
    let dead: Vec<(String, String)> = running
        .iter_mut()
        .filter_map(|(id, alive)| {
            matches!(alive.child.try_wait(), Ok(Some(_)) | Err(_))
                .then(|| (id.clone(), alive.command.clone()))
        })
        .collect();

    for (id, command) in dead {
        running.remove(&id);
        launch(&mut running, id, command, &on_rows);
    }
}

fn locked() -> std::sync::MutexGuard<'static, BTreeMap<String, Running>> {
    running().lock().unwrap_or_else(|err| err.into_inner())
}

/// Spawns one and records it. A connector that refuses to run is reported and
/// left out rather than retried in a loop.
fn launch(running: &mut BTreeMap<String, Running>, id: String, command: String, on_rows: &OnRows) {
    match spawn(&id, &command, on_rows.clone()) {
        Ok(child) => {
            running.insert(id, Running { child, command });
        }
        Err(message) => eprintln!("look connectors: [{id}] {message}"),
    }
}

/// Ends every connector. The rows they sent stay cached, which is what a user
/// who quit mid-sync should see on the way back in.
pub fn stop() {
    let mut running = locked();
    for (_, mut alive) in std::mem::take(&mut *running) {
        let _ = alive.child.kill();
    }
}

fn spawn(id: &str, command: &str, on_rows: OnRows) -> Result<Child, String> {
    let mut child = look_sources::spawn_connector(command, None)?;
    let stdout = child.stdout.take().ok_or("no stdout")?;

    if let Some(mut stdin) = child.stdin.take() {
        let hello = Ask::Hello {
            v: PROTOCOL_VERSION,
            look: env!("CARGO_PKG_VERSION").to_string(),
        };
        let _ = stdin.write_all(hello.line().as_bytes());
        let _ = stdin.flush();
    }

    // One reader per connector, for its lifetime. It ends when the child closes
    // its stdout, which is also how a crash is noticed.
    let owner = id.to_string();
    std::thread::spawn(move || read_frames(&owner, stdout, &on_rows));
    Ok(child)
}

/// The largest frame a connector may send. `lines()` would read a line of any
/// length into memory, so a connector could hand Look a gigabyte and that would
/// be Look's problem. A frame past the cap is a protocol breach, not a big
/// message: the connector is dropped and restarted on next use.
const MAX_FRAME_BYTES: usize = 1 << 20;

fn read_frames(owner: &str, stdout: std::process::ChildStdout, on_rows: &OnRows) {
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
                eprintln!("look connectors: [{owner}] sent a frame over {MAX_FRAME_BYTES} bytes");
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

fn handle(owner: &str, say: Say, on_rows: &OnRows) {
    match say {
        Say::Rows { block, rows } => {
            // A connector may only speak for the blocks of its own manifest.
            // Taking any name it offered would let one app rewrite another's
            // rows, which is the same boundary `then` is held to.
            if !speaks_for(owner, &block) {
                eprintln!("look connectors: [{owner}] tried to send rows for [{block}]");
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
        Say::Ready { .. } => {}
        Say::Error { message } => eprintln!("look connectors: [{owner}] {message}"),
    }
}

/// A connector speaks for its own block, and for others under the same manifest
/// namespace, so one process may serve every block in the file that declared it.
fn speaks_for(owner: &str, block: &str) -> bool {
    if owner == block {
        return true;
    }
    match (owner.split_once('.'), block.split_once('.')) {
        (Some((manifest, _)), Some((other, _))) => manifest == other,
        _ => false,
    }
}

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

    #[test]
    fn a_connector_speaks_only_for_its_own_manifest() {
        assert!(speaks_for("myapp.notes", "myapp.notes"));
        assert!(speaks_for("myapp.notes", "myapp.tags"));
        assert!(!speaks_for("myapp.notes", "other.notes"));
        // A block the user wrote has no namespace, so nothing reaches it.
        assert!(!speaks_for("myapp.notes", "projects"));
        assert!(!speaks_for("projects", "myapp.notes"));
    }
}
