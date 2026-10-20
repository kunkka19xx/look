//! The processes behind `connect` blocks.
//!
//! One child per block, spawned once and kept alive, speaking the frames in
//! `look_sources::connect`. Rows it sends are written to the same cache a `run`
//! block uses, so indexing, ranking and the last-good-rows rule all stay where
//! they are and nothing else had to learn what a connector is.
//!
//! Nothing here ever blocks a search. A connector that is slow, silent or dead
//! simply has no new rows, and the cache still holds the ones it sent before.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
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

/// Starts every declared connector that is not already running, and stops any
/// whose block is gone or whose command changed.
///
/// Safe to call repeatedly: it is how a reload reaches connectors, and how a
/// connector that died comes back.
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

    let mut running = running().lock().unwrap_or_else(|err| err.into_inner());

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
        if running.contains_key(&id) {
            continue;
        }
        match spawn(&id, &command, on_rows.clone()) {
            Ok(child) => {
                running.insert(id, Running { child, command });
            }
            Err(message) => eprintln!("look connectors: [{id}] {message}"),
        }
    }
}

/// Respawns connectors that died, and nothing else.
///
/// Cheap on purpose: it reads no directories, so the shell can call it every
/// time the launcher opens. Noticing a manifest installed since launch is what
/// `start` is for, and a reload is when that happens.
pub fn restart_dead(on_rows: OnRows) {
    let mut running = running().lock().unwrap_or_else(|err| err.into_inner());
    let dead: Vec<(String, String)> = running
        .iter_mut()
        .filter_map(|(id, alive)| {
            matches!(alive.child.try_wait(), Ok(Some(_)) | Err(_))
                .then(|| (id.clone(), alive.command.clone()))
        })
        .collect();

    for (id, command) in dead {
        running.remove(&id);
        match spawn(&id, &command, on_rows.clone()) {
            Ok(child) => {
                running.insert(id, Running { child, command });
            }
            Err(message) => eprintln!("look connectors: [{id}] {message}"),
        }
    }
}

/// Ends every connector. The rows they sent stay cached, which is what a user
/// who quit mid-sync should see on the way back in.
pub fn stop() {
    let mut running = running().lock().unwrap_or_else(|err| err.into_inner());
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
    // its stdout, which is also how a crash is noticed: there is nothing to
    // poll and nothing to time out.
    let owner = id.to_string();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let Some(say) = look_sources::read_frame(&line) else {
                continue;
            };
            handle(&owner, say, &on_rows);
        }
    });
    Ok(child)
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

/// Through the row cache, so a connector's rows are indexed, ranked and kept
/// exactly like a `run` block's, including keeping the last good ones.
fn store(block: &str, rows: &[SourceRow]) -> Result<(), String> {
    let json: Vec<serde_json::Value> = rows
        .iter()
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
    crate::index::store_run_rows(block, &text, look_sources::RowFormat::Json).map(|_| ())
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
