//! Shared todo backend for Look.
//!
//! Persists the /todo command's tasks in SQLite so both the macOS app
//! (via `bridge/ffi`) and linows (via its Tauri command layer) use the
//! same store. The crate owns the `todo_tasks` and `todo_tombstones`
//! tables and their migration; the caller passes the path to the app's
//! existing database (`look.db`), so todos live alongside
//! candidates/usage rather than in a second file. Data is kept for one
//! year (see [`RETENTION_DAYS`]); older days are pruned on load and save.
//!
//! Persistence contract mirrors the app's "edit in memory, hit Save"
//! model: the client loads the full task set with [`TodoStore::list`],
//! edits it locally, and writes the whole set back with
//! [`TodoStore::save`], which is a lossless full replace.
//!
//! The store also keeps per-task update stamps and deletion tombstones
//! (see docs/todo-sync.md). Clients never see either; [`TodoStore::save`]
//! maintains them by diffing the incoming set against what is stored, so
//! a future cross-machine merge can order edits and tell "deleted here"
//! from "created elsewhere".

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

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

/// Stored row content used to decide whether a saved task changed.
struct StoredTask {
    name: String,
    done: bool,
    due_date: String,
    updated_at_unix_s: i64,
}

fn now_unix_s() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_secs() as i64
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
            // The tables share look.db with the engine's connection, so WAL
            // (already the file's mode) plus a busy timeout keeps the two
            // writers from tripping over each other.
            "PRAGMA journal_mode = WAL;
             PRAGMA busy_timeout = 2000;

             CREATE TABLE IF NOT EXISTS todo_tasks (
                 id TEXT PRIMARY KEY,
                 name TEXT NOT NULL,
                 done INTEGER NOT NULL DEFAULT 0,
                 due_date TEXT NOT NULL,
                 created_at_unix_s INTEGER NOT NULL,
                 updated_at_unix_s INTEGER NOT NULL DEFAULT 0
             );

             CREATE INDEX IF NOT EXISTS idx_todo_due ON todo_tasks(due_date);

             CREATE TABLE IF NOT EXISTS todo_tombstones (
                 id TEXT PRIMARY KEY,
                 deleted_at_unix_s INTEGER NOT NULL
             );",
        )?;

        // Databases created before the sync prep lack the update stamp;
        // backfill from created_at so a first merge has usable timestamps.
        let has_updated_at = self.conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('todo_tasks')
             WHERE name = 'updated_at_unix_s'",
            [],
            |row| row.get::<_, i64>(0).map(|n| n > 0),
        )?;
        if !has_updated_at {
            self.conn.execute_batch(
                "ALTER TABLE todo_tasks
                     ADD COLUMN updated_at_unix_s INTEGER NOT NULL DEFAULT 0;
                 UPDATE todo_tasks SET updated_at_unix_s = created_at_unix_s;",
            )?;
        }
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
    ///
    /// Sync bookkeeping happens here, invisibly to callers: a task whose
    /// content changed gets a fresh update stamp, an unchanged task keeps
    /// its stored one, and an id that vanishes from the set is tombstoned.
    /// Saving an id again clears its tombstone.
    pub fn save(&mut self, tasks: &[TodoTask]) -> TodoResult<()> {
        self.save_at(tasks, now_unix_s())
    }

    fn save_at(&mut self, tasks: &[TodoTask], now_unix_s: i64) -> TodoResult<()> {
        let tx = self.conn.transaction()?;

        let mut stored: HashMap<String, StoredTask> = HashMap::new();
        {
            let mut stmt =
                tx.prepare("SELECT id, name, done, due_date, updated_at_unix_s FROM todo_tasks")?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    StoredTask {
                        name: row.get(1)?,
                        done: row.get::<_, i64>(2)? != 0,
                        due_date: row.get(3)?,
                        updated_at_unix_s: row.get(4)?,
                    },
                ))
            })?;
            for row in rows {
                let (id, task) = row?;
                stored.insert(id, task);
            }
        }

        tx.execute("DELETE FROM todo_tasks", [])?;
        {
            let mut insert = tx.prepare(
                "INSERT OR REPLACE INTO todo_tasks
                     (id, name, done, due_date, created_at_unix_s, updated_at_unix_s)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            let mut clear_tombstone = tx.prepare("DELETE FROM todo_tombstones WHERE id = ?1")?;
            for task in tasks {
                let name = task.name.trim();
                if name.is_empty() {
                    continue;
                }
                let updated_at = match stored.remove(&task.id) {
                    Some(old)
                        if old.name == name
                            && old.done == task.done
                            && old.due_date == task.due_date =>
                    {
                        old.updated_at_unix_s
                    }
                    _ => now_unix_s,
                };
                insert.execute(params![
                    task.id,
                    name,
                    task.done as i64,
                    task.due_date,
                    task.created_at_unix_s,
                    updated_at,
                ])?;
                clear_tombstone.execute(params![task.id])?;
            }

            // Whatever is left in `stored` was deleted by this save.
            let mut add_tombstone = tx.prepare(
                "INSERT OR REPLACE INTO todo_tombstones (id, deleted_at_unix_s)
                 VALUES (?1, ?2)",
            )?;
            for id in stored.keys() {
                add_tombstone.execute(params![id, now_unix_s])?;
            }
        }
        tx.commit()?;
        self.prune_expired()?;
        Ok(())
    }

    /// Deletes tasks whose day is older than the retention window, and
    /// tombstones old enough that every machine has pruned the task they
    /// point at (same window, so a retention prune on one machine never
    /// reads as a deletion to another). Uses SQLite's own date math so it
    /// stays correct without a date crate.
    pub fn prune_expired(&self) -> TodoResult<usize> {
        let window = format!("-{RETENTION_DAYS} days");
        let removed = self.conn.execute(
            "DELETE FROM todo_tasks WHERE due_date < date('now', ?1)",
            params![window],
        )?;
        self.conn.execute(
            "DELETE FROM todo_tombstones
             WHERE deleted_at_unix_s < CAST(strftime('%s', 'now', ?1) AS INTEGER)",
            params![window],
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

    fn updated_at(store: &TodoStore, id: &str) -> i64 {
        store
            .conn
            .query_row(
                "SELECT updated_at_unix_s FROM todo_tasks WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .expect("updated_at")
    }

    fn tombstones(store: &TodoStore) -> Vec<(String, i64)> {
        let mut stmt = store
            .conn
            .prepare("SELECT id, deleted_at_unix_s FROM todo_tombstones ORDER BY id")
            .expect("prepare");
        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .expect("query");
        rows.map(|row| row.expect("row")).collect()
    }

    #[test]
    fn unchanged_tasks_keep_their_update_stamp() {
        let mut store = TodoStore::open_in_memory().expect("open");
        store
            .save_at(&[task("a", "First", false, "2999-01-01")], 100)
            .expect("save 1");
        assert_eq!(updated_at(&store, "a"), 100);

        store
            .save_at(&[task("a", "First", false, "2999-01-01")], 200)
            .expect("save 2");
        assert_eq!(updated_at(&store, "a"), 100, "unchanged keeps stamp");

        store
            .save_at(&[task("a", "First", true, "2999-01-01")], 300)
            .expect("save 3");
        assert_eq!(updated_at(&store, "a"), 300, "content change bumps stamp");
    }

    // Tombstone purging compares against the real clock, so stamps used in
    // tombstone tests must sit inside the retention window. Far-future
    // keeps them deterministic.
    const NOW: i64 = 4_000_000_000;

    #[test]
    fn deleted_tasks_leave_tombstones() {
        let mut store = TodoStore::open_in_memory().expect("open");
        store
            .save_at(
                &[
                    task("a", "Keep", false, "2999-01-01"),
                    task("b", "Drop", false, "2999-01-01"),
                ],
                NOW,
            )
            .expect("save 1");
        assert!(tombstones(&store).is_empty());

        store
            .save_at(&[task("a", "Keep", false, "2999-01-01")], NOW + 100)
            .expect("save 2");
        assert_eq!(tombstones(&store), vec![("b".to_string(), NOW + 100)]);
    }

    #[test]
    fn blanking_a_name_counts_as_deletion() {
        let mut store = TodoStore::open_in_memory().expect("open");
        store
            .save_at(&[task("a", "Real", false, "2999-01-01")], NOW)
            .expect("save 1");
        store
            .save_at(&[task("a", "   ", false, "2999-01-01")], NOW + 100)
            .expect("save 2");
        assert_eq!(tombstones(&store), vec![("a".to_string(), NOW + 100)]);
    }

    #[test]
    fn resaving_an_id_clears_its_tombstone() {
        let mut store = TodoStore::open_in_memory().expect("open");
        store
            .save_at(&[task("a", "Task", false, "2999-01-01")], NOW)
            .expect("save 1");
        store.save_at(&[], NOW + 100).expect("save 2");
        assert_eq!(tombstones(&store).len(), 1);

        store
            .save_at(&[task("a", "Task", false, "2999-01-01")], NOW + 200)
            .expect("save 3");
        assert!(tombstones(&store).is_empty());
        assert_eq!(
            updated_at(&store, "a"),
            NOW + 200,
            "resurrected counts as new"
        );
    }

    #[test]
    fn expired_tombstones_are_pruned() {
        let store = TodoStore::open_in_memory().expect("open");
        store
            .conn
            .execute(
                "INSERT INTO todo_tombstones (id, deleted_at_unix_s) VALUES
                     ('old', 1000), ('new', ?1)",
                params![now_unix_s()],
            )
            .expect("insert");
        store.prune_expired().expect("prune");
        assert_eq!(tombstones(&store).len(), 1);
        assert_eq!(tombstones(&store)[0].0, "new");
    }

    #[test]
    fn migrate_backfills_updated_at_from_created_at() {
        // A database from before the sync prep: no stamp column, no
        // tombstone table.
        let conn = Connection::open_in_memory().expect("open");
        conn.execute_batch(
            "CREATE TABLE todo_tasks (
                 id TEXT PRIMARY KEY,
                 name TEXT NOT NULL,
                 done INTEGER NOT NULL DEFAULT 0,
                 due_date TEXT NOT NULL,
                 created_at_unix_s INTEGER NOT NULL
             );
             INSERT INTO todo_tasks VALUES ('a', 'Old row', 0, '2999-01-01', 4242);",
        )
        .expect("old schema");

        let store = TodoStore { conn };
        store.migrate().expect("migrate");
        assert_eq!(updated_at(&store, "a"), 4242);
        assert!(tombstones(&store).is_empty());
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
