//! Shared todo backend for Look.
//!
//! Persists the /todo command's tasks in SQLite so both the macOS app
//! (via `bridge/ffi`) and linows (via its Tauri command layer) use the
//! same store. The crate owns only the `todo_tasks` table and its
//! migration; the caller passes the path to the app's existing database
//! (`look.db`), so todos live alongside candidates/usage rather than in
//! a second file. Data is kept for one year (see [`RETENTION_DAYS`]);
//! older days are pruned on load and save.
//!
//! Persistence contract mirrors the app's "edit in memory, hit Save"
//! model: the client loads the full task set with [`TodoStore::list`],
//! edits it locally, and writes the whole set back with
//! [`TodoStore::save`], which is a lossless full replace.

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::path::Path;

/// Retention window. Tasks whose `due_date` is older than this are
/// pruned. A little over a year to give leap-day slack.
pub const RETENTION_DAYS: i64 = 366;

#[derive(Debug)]
pub enum TodoError {
    Io(std::io::Error),
    Sql(rusqlite::Error),
}

impl Display for TodoError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TodoError::Io(err) => write!(f, "io error: {err}"),
            TodoError::Sql(err) => write!(f, "sqlite error: {err}"),
        }
    }
}

impl std::error::Error for TodoError {}

impl From<std::io::Error> for TodoError {
    fn from(value: std::io::Error) -> Self {
        TodoError::Io(value)
    }
}

impl From<rusqlite::Error> for TodoError {
    fn from(value: rusqlite::Error) -> Self {
        TodoError::Sql(value)
    }
}

pub type TodoResult<T> = Result<T, TodoError>;

/// A single task. `due_date` is the day it belongs to, `yyyy-MM-dd`, and
/// also how the client groups tasks into date sections.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TodoTask {
    pub id: String,
    pub name: String,
    pub done: bool,
    pub due_date: String,
    pub created_at_unix_s: i64,
}

pub struct TodoStore {
    conn: Connection,
}

impl TodoStore {
    pub fn open(path: impl AsRef<Path>) -> TodoResult<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    pub fn open_in_memory() -> TodoResult<Self> {
        let conn = Connection::open_in_memory()?;
        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> TodoResult<()> {
        self.conn.execute_batch(
            // The table shares look.db with the engine's connection, so WAL
            // (already the file's mode) plus a busy timeout keeps the two
            // writers from tripping over each other.
            "PRAGMA journal_mode = WAL;
             PRAGMA busy_timeout = 2000;

             CREATE TABLE IF NOT EXISTS todo_tasks (
                 id TEXT PRIMARY KEY,
                 name TEXT NOT NULL,
                 done INTEGER NOT NULL DEFAULT 0,
                 due_date TEXT NOT NULL,
                 created_at_unix_s INTEGER NOT NULL
             );

             CREATE INDEX IF NOT EXISTS idx_todo_due ON todo_tasks(due_date);",
        )?;
        Ok(())
    }

    /// All tasks, newest day first, after pruning anything past the
    /// one-year retention window.
    pub fn list(&self) -> TodoResult<Vec<TodoTask>> {
        self.prune_expired()?;
        let mut stmt = self.conn.prepare(
            "SELECT id, name, done, due_date, created_at_unix_s
             FROM todo_tasks
             ORDER BY due_date DESC, created_at_unix_s ASC, id ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(TodoTask {
                id: row.get(0)?,
                name: row.get(1)?,
                done: row.get::<_, i64>(2)? != 0,
                due_date: row.get(3)?,
                created_at_unix_s: row.get(4)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Replaces the entire task set with `tasks`. The client always holds
    /// the full set it loaded, so a full replace is lossless. Tasks with a
    /// blank name are dropped. Expired days are pruned afterwards.
    pub fn save(&mut self, tasks: &[TodoTask]) -> TodoResult<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM todo_tasks", [])?;
        {
            let mut stmt = tx.prepare(
                "INSERT OR REPLACE INTO todo_tasks
                     (id, name, done, due_date, created_at_unix_s)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            for task in tasks {
                let name = task.name.trim();
                if name.is_empty() {
                    continue;
                }
                stmt.execute(params![
                    task.id,
                    name,
                    task.done as i64,
                    task.due_date,
                    task.created_at_unix_s,
                ])?;
            }
        }
        tx.commit()?;
        self.prune_expired()?;
        Ok(())
    }

    /// Deletes tasks whose day is older than the retention window. Uses
    /// SQLite's own date math so it stays correct without a date crate.
    pub fn prune_expired(&self) -> TodoResult<usize> {
        let removed = self.conn.execute(
            "DELETE FROM todo_tasks WHERE due_date < date('now', ?1)",
            params![format!("-{RETENTION_DAYS} days")],
        )?;
        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(id: &str, name: &str, done: bool, due: &str) -> TodoTask {
        TodoTask {
            id: id.into(),
            name: name.into(),
            done,
            due_date: due.into(),
            created_at_unix_s: 1_000,
        }
    }

    #[test]
    fn open_in_memory_creates_schema() {
        let store = TodoStore::open_in_memory().expect("open");
        assert!(store.list().expect("list").is_empty());
    }

    #[test]
    fn save_then_list_round_trips() {
        let mut store = TodoStore::open_in_memory().expect("open");
        store
            .save(&[
                task("a", "Ship PR", true, "2026-07-05"),
                task("b", "Review spec", false, "2026-07-05"),
                task("c", "Grocery run", false, "2026-07-04"),
            ])
            .expect("save");

        let loaded = store.list().expect("list");
        assert_eq!(loaded.len(), 3);
        // Newest day first.
        assert_eq!(loaded[0].due_date, "2026-07-05");
        assert_eq!(loaded[2].due_date, "2026-07-04");
        assert!(loaded[0].done);
    }

    #[test]
    fn save_is_a_full_replace() {
        let mut store = TodoStore::open_in_memory().expect("open");
        store
            .save(&[task("a", "First", false, "2026-07-05")])
            .expect("save 1");
        store
            .save(&[task("b", "Second", false, "2026-07-05")])
            .expect("save 2");

        let loaded = store.list().expect("list");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "b");
    }

    #[test]
    fn save_drops_blank_names() {
        let mut store = TodoStore::open_in_memory().expect("open");
        store
            .save(&[
                task("a", "Real", false, "2026-07-05"),
                task("b", "   ", false, "2026-07-05"),
            ])
            .expect("save");
        assert_eq!(store.list().expect("list").len(), 1);
    }

    #[test]
    fn json_shape_is_the_ffi_contract() {
        // The Swift and (future) linows clients decode these exact keys;
        // a rename here is a breaking cross-layer change.
        let t = task("a", "Ship PR", true, "2026-07-05");
        let json = serde_json::to_string(&t).expect("serialize");
        for key in [
            "\"id\"",
            "\"name\"",
            "\"done\"",
            "\"due_date\"",
            "\"created_at_unix_s\"",
        ] {
            assert!(json.contains(key), "missing {key} in {json}");
        }
        let back: TodoTask = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, t);
    }

    #[test]
    fn expired_days_are_pruned() {
        let mut store = TodoStore::open_in_memory().expect("open");
        // A day well outside the retention window is dropped; a recent day
        // is kept. `date('now')` is evaluated by SQLite at prune time.
        store
            .save(&[
                task("old", "Ancient", true, "2000-01-01"),
                task("new", "Recent", false, "2999-01-01"),
            ])
            .expect("save");
        let loaded = store.list().expect("list");
        let ids: Vec<&str> = loaded.iter().map(|t| t.id.as_str()).collect();
        assert!(!ids.contains(&"old"), "expired day pruned");
        assert!(ids.contains(&"new"), "recent day kept");
    }
}
