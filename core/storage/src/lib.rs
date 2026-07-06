use look_indexing::{Candidate, CandidateKind};
use rusqlite::{Connection, params};
use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const SEARCH_ENGINE_DUCKDUCKGO: &str = "duckduckgo";
const SEARCH_ENGINE_GOOGLE: &str = "google";
const SEARCH_ENGINE_BING: &str = "bing";
const MAX_CANDIDATE_PREALLOC: usize = 10_000;

const SETTINGS_KEY_WEB_SEARCH_ENABLED: &str = "web_search_enabled";
const SETTINGS_KEY_WEB_SEARCH_ENGINE: &str = "web_search_engine";
const SETTINGS_TRUE: &str = "true";
const SETTINGS_FALSE: &str = "false";

/// Number of rows in the demo seed inserted by `QueryEngine::demo_candidates`.
/// `is_demo_seeded` uses this as an upper bound: anything larger is real data.
/// Keep in sync with `core/engine/src/lib.rs::demo_candidates`.
const DEMO_SEED_ROW_COUNT: i64 = 6;

/// Marker ids the demo seed always contains. `is_demo_seeded` treats the
/// database as a pristine demo only if BOTH are present, so a small real
/// DB that happens to contain one isn't wiped on the next scoped refresh.
const DEMO_MARKER_SAFARI: &str = "app:safari";
const DEMO_MARKER_VSCODE: &str = "app:vscode";

#[derive(Default)]
pub struct InMemorySettingsStore {
    values: HashMap<String, String>,
}

#[derive(Debug)]
pub enum StorageError {
    Io(std::io::Error),
    Sql(rusqlite::Error),
    Data(String),
}

impl Display for StorageError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageError::Io(err) => write!(f, "io error: {err}"),
            StorageError::Sql(err) => write!(f, "sqlite error: {err}"),
            StorageError::Data(err) => write!(f, "data error: {err}"),
        }
    }
}

impl std::error::Error for StorageError {}

impl From<std::io::Error> for StorageError {
    fn from(value: std::io::Error) -> Self {
        StorageError::Io(value)
    }
}

impl From<rusqlite::Error> for StorageError {
    fn from(value: rusqlite::Error) -> Self {
        StorageError::Sql(value)
    }
}

pub type StorageResult<T> = Result<T, StorageError>;

pub struct SqliteStore {
    conn: Connection,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchEngine {
    DuckDuckGo,
    Google,
    Bing,
}

impl SearchEngine {
    pub fn key(self) -> &'static str {
        match self {
            SearchEngine::DuckDuckGo => SEARCH_ENGINE_DUCKDUCKGO,
            SearchEngine::Google => SEARCH_ENGINE_GOOGLE,
            SearchEngine::Bing => SEARCH_ENGINE_BING,
        }
    }

    pub fn from_key(value: &str) -> Self {
        match value {
            SEARCH_ENGINE_DUCKDUCKGO => SearchEngine::DuckDuckGo,
            SEARCH_ENGINE_GOOGLE => SearchEngine::Google,
            SEARCH_ENGINE_BING => SearchEngine::Bing,
            _ => SearchEngine::Google,
        }
    }

    pub fn build_search_url(self, query: &str) -> String {
        let encoded = form_encode_query(query);
        match self {
            SearchEngine::DuckDuckGo => format!("https://duckduckgo.com/?q={encoded}"),
            SearchEngine::Google => format!("https://www.google.com/search?q={encoded}"),
            SearchEngine::Bing => format!("https://www.bing.com/search?q={encoded}"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SearchSettings {
    pub web_search_enabled: bool,
    pub web_search_engine: SearchEngine,
}

impl Default for SearchSettings {
    fn default() -> Self {
        Self {
            web_search_enabled: true,
            web_search_engine: SearchEngine::Google,
        }
    }
}

impl InMemorySettingsStore {
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.values.insert(key.into(), value.into());
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    pub fn search_settings(&self) -> SearchSettings {
        let enabled = self
            .get(SETTINGS_KEY_WEB_SEARCH_ENABLED)
            .map(|value| value == SETTINGS_TRUE)
            .unwrap_or(true);
        let engine = SearchEngine::from_key(
            self.get(SETTINGS_KEY_WEB_SEARCH_ENGINE)
                .unwrap_or(SEARCH_ENGINE_GOOGLE),
        );
        SearchSettings {
            web_search_enabled: enabled,
            web_search_engine: engine,
        }
    }

    pub fn set_search_settings(&mut self, settings: SearchSettings) {
        self.set(
            SETTINGS_KEY_WEB_SEARCH_ENABLED,
            if settings.web_search_enabled {
                SETTINGS_TRUE
            } else {
                SETTINGS_FALSE
            },
        );
        self.set(
            SETTINGS_KEY_WEB_SEARCH_ENGINE,
            settings.web_search_engine.key(),
        );
    }
}

impl SqliteStore {
    pub fn open(path: impl AsRef<Path>) -> StorageResult<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    pub fn open_in_memory() -> StorageResult<Self> {
        let conn = Connection::open_in_memory()?;
        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    /// Cheap check for "is the candidates table currently just the demo seed?".
    /// Used by `bootstrap_sqlite_scoped` to decide whether to wipe the table
    /// before streaming real candidates. Point lookups on the PK index; avoids
    /// deserializing every row just to inspect the marker ids.
    ///
    /// Source of truth for the marker ids and row count is
    /// `QueryEngine::demo_candidates` in `core/engine/src/lib.rs`.
    pub fn is_demo_seeded(&self) -> StorageResult<bool> {
        let total: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM candidates", [], |row| row.get(0))?;
        // Anything larger than the demo seed is definitely real data.
        if total == 0 || total > DEMO_SEED_ROW_COUNT {
            return Ok(false);
        }
        // Demo seed always includes both markers; require BOTH families
        // present so a small real DB with only one of them doesn't trip
        // the heuristic and get wiped on the next scoped refresh.
        Ok(self.has_candidate(DEMO_MARKER_SAFARI)? && self.has_candidate(DEMO_MARKER_VSCODE)?)
    }

    fn has_candidate(&self, id: &str) -> StorageResult<bool> {
        let hit: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM candidates WHERE id = ?1",
            [id],
            |row| row.get(0),
        )?;
        Ok(hit > 0)
    }

    pub fn load_candidates(&self, limit: Option<usize>) -> StorageResult<Vec<Candidate>> {
        let sql = match limit {
            Some(_) => {
                "SELECT id, kind, title, subtitle, path, use_count, last_used_at_unix_s, fs_modified_at_unix_s FROM candidates ORDER BY title ASC LIMIT ?1"
            }
            None => {
                "SELECT id, kind, title, subtitle, path, use_count, last_used_at_unix_s, fs_modified_at_unix_s FROM candidates ORDER BY title ASC"
            }
        };

        let mut stmt = self.conn.prepare(sql)?;
        let mut rows = match limit {
            Some(max) => stmt.query([max as i64])?,
            None => stmt.query([])?,
        };

        let mut out = match limit {
            Some(max) => Vec::with_capacity(max.min(MAX_CANDIDATE_PREALLOC)),
            None => Vec::new(),
        };
        while let Some(row) = rows.next()? {
            let kind_raw: String = row.get(1)?;
            let use_count_raw: i64 = row.get(5)?;
            out.push(Candidate {
                id: row.get::<_, String>(0)?.into_boxed_str(),
                kind: parse_kind(&kind_raw)?,
                title: row.get::<_, String>(2)?.into_boxed_str(),
                subtitle: row.get::<_, Option<String>>(3)?.map(String::into_boxed_str),
                path: row.get::<_, String>(4)?.into_boxed_str(),
                use_count: to_use_count(use_count_raw)?,
                last_used_at_unix_s: row.get(6)?,
                fs_modified_at_unix_s: row.get(7)?,
            });
        }

        Ok(out)
    }

    pub fn upsert_candidates(&mut self, candidates: &[Candidate]) -> StorageResult<()> {
        self.upsert_candidates_indexed(candidates, Some(current_unix_s()?))
    }

    pub fn upsert_candidates_indexed(
        &mut self,
        candidates: &[Candidate],
        indexed_at_unix_s: Option<i64>,
    ) -> StorageResult<()> {
        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare(
                // Interim write-reduction (see specs/indexing-scale.md): the `WHERE`
                // skips the UPDATE entirely when nothing the scan controls changed,
                // so a reindex after one download/install rewrites ~1 row instead of
                // all ~8000. Pairs with `delete_unseen_candidates` for pruning - we
                // can no longer rely on bumping `indexed_at` for every seen row.
                // List EVERY scan-owned column here; a missing one means stale data
                // silently lingers. (use_count/last_used_at are intentionally never
                // overwritten on conflict, so they're not part of change detection.)
                // TODO(indexing-scale Direction A): the full walk + per-reindex
                // re-normalize are still O(total). Move to event-driven incremental
                // indexing (watcher paths) to make reindex cost ∝ changed files.
                "INSERT INTO candidates (id, kind, title, subtitle, path, use_count, last_used_at_unix_s, indexed_at_unix_s, fs_modified_at_unix_s)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(id) DO UPDATE SET
                   kind = excluded.kind,
                   title = excluded.title,
                   subtitle = excluded.subtitle,
                     path = excluded.path,
                    indexed_at_unix_s = excluded.indexed_at_unix_s,
                    fs_modified_at_unix_s = excluded.fs_modified_at_unix_s
                 WHERE candidates.kind <> excluded.kind
                    OR candidates.title <> excluded.title
                    OR candidates.subtitle IS NOT excluded.subtitle
                    OR candidates.path <> excluded.path
                    OR candidates.fs_modified_at_unix_s IS NOT excluded.fs_modified_at_unix_s",
            )?;

            for candidate in candidates {
                let use_count = from_use_count(candidate.use_count)?;
                stmt.execute(params![
                    candidate.id.as_ref(),
                    kind_key(&candidate.kind),
                    candidate.title.as_ref(),
                    candidate.subtitle.as_deref(),
                    candidate.path.as_ref(),
                    use_count,
                    candidate.last_used_at_unix_s,
                    indexed_at_unix_s,
                    candidate.fs_modified_at_unix_s,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn replace_candidates(&mut self, candidates: &[Candidate]) -> StorageResult<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM usage_events", [])?;
        tx.execute("DELETE FROM candidates", [])?;

        {
            let mut stmt = tx.prepare(
                "INSERT INTO candidates (id, kind, title, subtitle, path, use_count, last_used_at_unix_s, indexed_at_unix_s, fs_modified_at_unix_s)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            )?;

            for candidate in candidates {
                let use_count = from_use_count(candidate.use_count)?;
                stmt.execute(params![
                    candidate.id.as_ref(),
                    kind_key(&candidate.kind),
                    candidate.title.as_ref(),
                    candidate.subtitle.as_deref(),
                    candidate.path.as_ref(),
                    use_count,
                    candidate.last_used_at_unix_s,
                    None::<i64>,
                    candidate.fs_modified_at_unix_s,
                ])?;
            }
        }

        tx.commit()?;
        Ok(())
    }

    pub fn load_search_settings(&self) -> StorageResult<SearchSettings> {
        let mut settings = SearchSettings::default();
        let mut stmt = self.conn.prepare(&format!(
            "SELECT key, value FROM settings WHERE key IN ('{}', '{}')",
            SETTINGS_KEY_WEB_SEARCH_ENABLED, SETTINGS_KEY_WEB_SEARCH_ENGINE
        ))?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let key: String = row.get(0)?;
            let value: String = row.get(1)?;
            match key.as_str() {
                SETTINGS_KEY_WEB_SEARCH_ENABLED => {
                    settings.web_search_enabled = value == SETTINGS_TRUE
                }
                SETTINGS_KEY_WEB_SEARCH_ENGINE => {
                    settings.web_search_engine = SearchEngine::from_key(&value)
                }
                _ => {}
            }
        }

        Ok(settings)
    }

    pub fn save_search_settings(&mut self, settings: SearchSettings) -> StorageResult<()> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO settings(key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![
                SETTINGS_KEY_WEB_SEARCH_ENABLED,
                if settings.web_search_enabled {
                    SETTINGS_TRUE
                } else {
                    SETTINGS_FALSE
                }
            ],
        )?;
        tx.execute(
            "INSERT INTO settings(key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![
                SETTINGS_KEY_WEB_SEARCH_ENGINE,
                settings.web_search_engine.key()
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn record_usage_event(&self, candidate_id: &str, action: &str) -> StorageResult<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| StorageError::Data(format!("system time error: {err}")))?
            .as_secs() as i64;

        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO usage_events(candidate_id, action, used_at_unix_s) VALUES (?1, ?2, ?3)",
            params![candidate_id, action, now],
        )?;
        tx.execute(
            "UPDATE candidates SET use_count = use_count + 1, last_used_at_unix_s = ?2 WHERE id = ?1",
            params![candidate_id, now],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn delete_stale_candidates(&mut self, older_than_unix_s: i64) -> StorageResult<usize> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "DELETE FROM usage_events
             WHERE candidate_id IN (
               SELECT id FROM candidates
               WHERE indexed_at_unix_s IS NULL OR indexed_at_unix_s < ?1
             )",
            params![older_than_unix_s],
        )?;
        let removed = tx.execute(
            "DELETE FROM candidates
             WHERE indexed_at_unix_s IS NULL OR indexed_at_unix_s < ?1",
            params![older_than_unix_s],
        )?;
        tx.commit()?;
        Ok(removed)
    }

    /// Seen-set pruning, the counterpart to the change-detecting upsert
    /// (see specs/indexing-scale.md). Because the upsert no longer bumps
    /// `indexed_at` on unchanged rows, "was this seen in the latest scan?" can't
    /// be inferred from `indexed_at` anymore - we delete rows in scope that are
    /// NOT in `seen_ids`. The `indexed_at < older_than_unix_s` guard preserves the
    /// `i64::MAX` pinned/seeded rows (e.g. UWP), exactly like `delete_stale_*`.
    /// `prefixes` empty = whole table (ALL-scope refresh); otherwise restrict to
    /// rows whose id starts with one of the prefixes (scoped refresh).
    ///
    /// `seen_ids` is staged in an in-memory temp table (PRAGMA temp_store=MEMORY),
    /// so this adds no persistent writes beyond the rows it actually deletes.
    pub fn delete_unseen_candidates(
        &mut self,
        seen_ids: &HashSet<Box<str>>,
        older_than_unix_s: i64,
        prefixes: &[&str],
    ) -> StorageResult<usize> {
        let like_clause = if prefixes.is_empty() {
            String::new()
        } else {
            let ors = (0..prefixes.len())
                .map(|i| format!("id LIKE ?{} ESCAPE '\\'", i + 2))
                .collect::<Vec<_>>()
                .join(" OR ");
            format!(" AND ({ors})")
        };
        let escaped: Vec<String> = prefixes
            .iter()
            .map(|p| format!("{}%", p.replace('\\', "\\\\").replace('%', "\\%")))
            .collect();

        let tx = self.conn.transaction()?;
        tx.execute_batch(
            "CREATE TEMP TABLE IF NOT EXISTS _seen_ids (id TEXT PRIMARY KEY);
             DELETE FROM _seen_ids;",
        )?;
        {
            let mut stmt = tx.prepare("INSERT OR IGNORE INTO _seen_ids (id) VALUES (?1)")?;
            for id in seen_ids {
                stmt.execute(params![id.as_ref()])?;
            }
        }

        let mut bindings: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(1 + escaped.len());
        bindings.push(&older_than_unix_s);
        for s in &escaped {
            bindings.push(s);
        }

        let where_unseen = format!(
            "(indexed_at_unix_s IS NULL OR indexed_at_unix_s < ?1)
             AND id NOT IN (SELECT id FROM _seen_ids){like_clause}"
        );

        tx.execute(
            &format!(
                "DELETE FROM usage_events WHERE candidate_id IN (
                   SELECT id FROM candidates WHERE {where_unseen}
                 )"
            ),
            bindings.as_slice(),
        )?;
        let removed = tx.execute(
            &format!("DELETE FROM candidates WHERE {where_unseen}"),
            bindings.as_slice(),
        )?;
        tx.commit()?;
        Ok(removed)
    }

    /// Scoped variant of `delete_stale_candidates`: prunes only rows whose id begins
    /// with one of `prefixes`. Used by partial reindex paths that re-walk a subset
    /// of sources (e.g. apps-only) and must not prune candidates from sources that
    /// were not part of this run.
    pub fn delete_stale_candidates_with_prefixes(
        &mut self,
        older_than_unix_s: i64,
        prefixes: &[&str],
    ) -> StorageResult<usize> {
        if prefixes.is_empty() {
            return Ok(0);
        }
        let escaped: Vec<String> = prefixes
            .iter()
            .map(|p| format!("{}%", p.replace('\\', "\\\\").replace('%', "\\%")))
            .collect();
        let like_clause = (0..escaped.len())
            .map(|i| format!("id LIKE ?{} ESCAPE '\\'", i + 2))
            .collect::<Vec<_>>()
            .join(" OR ");

        let tx = self.conn.transaction()?;

        let mut bindings: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(1 + escaped.len());
        bindings.push(&older_than_unix_s);
        for s in &escaped {
            bindings.push(s);
        }

        let del_usage_sql = format!(
            "DELETE FROM usage_events
             WHERE candidate_id IN (
               SELECT id FROM candidates
               WHERE (indexed_at_unix_s IS NULL OR indexed_at_unix_s < ?1)
                 AND ({like_clause})
             )"
        );
        tx.execute(&del_usage_sql, bindings.as_slice())?;

        let del_cand_sql = format!(
            "DELETE FROM candidates
             WHERE (indexed_at_unix_s IS NULL OR indexed_at_unix_s < ?1)
               AND ({like_clause})"
        );
        let removed = tx.execute(&del_cand_sql, bindings.as_slice())?;
        tx.commit()?;
        Ok(removed)
    }

    /// Deletes every candidate whose `id` starts with `prefix` and is NOT in `keep_ids`,
    /// along with its usage_events rows. Returned value is the number of candidate rows
    /// removed. Used by the UWP seed path (bridge/ffi/src/seed_api.rs) to age out apps
    /// that disappeared from `shell:AppsFolder` between runs - those rows can't be
    /// pruned by `delete_stale_candidates` because they're written with
    /// `indexed_at_unix_s = i64::MAX` to survive the regular index-refresh sweep.
    pub fn delete_candidates_by_prefix_except(
        &mut self,
        prefix: &str,
        keep_ids: &HashSet<&str>,
    ) -> StorageResult<usize> {
        let tx = self.conn.transaction()?;
        let like_pattern = format!("{}%", prefix.replace('\\', "\\\\").replace('%', "\\%"));

        let stale_ids: Vec<String> = {
            let mut stmt = tx.prepare("SELECT id FROM candidates WHERE id LIKE ?1 ESCAPE '\\'")?;
            let rows = stmt.query_map(params![like_pattern], |row| row.get::<_, String>(0))?;
            let mut out = Vec::new();
            for row in rows {
                let id = row?;
                if !keep_ids.contains(id.as_str()) {
                    out.push(id);
                }
            }
            out
        };

        for id in &stale_ids {
            tx.execute(
                "DELETE FROM usage_events WHERE candidate_id = ?1",
                params![id],
            )?;
            tx.execute("DELETE FROM candidates WHERE id = ?1", params![id])?;
        }
        tx.commit()?;
        Ok(stale_ids.len())
    }

    pub fn prune_usage_events_older_than(&mut self, cutoff_unix_s: i64) -> StorageResult<usize> {
        let removed = self.conn.execute(
            "DELETE FROM usage_events WHERE used_at_unix_s < ?1",
            params![cutoff_unix_s],
        )?;
        Ok(removed)
    }

    pub fn prune_usage_events_to_max(&mut self, max_rows: usize) -> StorageResult<usize> {
        let total: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM usage_events", [], |row| row.get(0))?;
        let total = total.max(0) as usize;
        if total <= max_rows {
            return Ok(0);
        }

        let overflow = total - max_rows;
        let removed = self.conn.execute(
            "DELETE FROM usage_events
             WHERE id IN (
               SELECT id FROM usage_events ORDER BY used_at_unix_s ASC, id ASC LIMIT ?1
             )",
            params![overflow as i64],
        )?;
        Ok(removed)
    }

    fn migrate(&self) -> StorageResult<()> {
        self.conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;
             -- Keep the seen-id temp table used by delete_unseen_candidates in RAM.
             PRAGMA temp_store = MEMORY;

             CREATE TABLE IF NOT EXISTS settings (
                 key TEXT PRIMARY KEY,
                 value TEXT NOT NULL
             );

             CREATE TABLE IF NOT EXISTS candidates (
                 id TEXT PRIMARY KEY,
                 kind TEXT NOT NULL,
                 title TEXT NOT NULL,
                 subtitle TEXT,
                 path TEXT NOT NULL,
                 use_count INTEGER NOT NULL DEFAULT 0,
                 last_used_at_unix_s INTEGER,
                 indexed_at_unix_s INTEGER
              );

             CREATE INDEX IF NOT EXISTS idx_candidates_title ON candidates(title);
             CREATE INDEX IF NOT EXISTS idx_candidates_kind ON candidates(kind);

             CREATE TABLE IF NOT EXISTS usage_events (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 candidate_id TEXT NOT NULL,
                 action TEXT NOT NULL,
                 used_at_unix_s INTEGER NOT NULL,
                 FOREIGN KEY(candidate_id) REFERENCES candidates(id)
             );

              CREATE TABLE IF NOT EXISTS index_state (
                  source TEXT PRIMARY KEY,
                  last_indexed_at_unix_s INTEGER NOT NULL
              );",
        )?;

        ensure_column_exists(&self.conn, "candidates", "indexed_at_unix_s", "INTEGER")?;
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_candidates_indexed_at ON candidates(indexed_at_unix_s)",
            [],
        )?;

        // Filesystem mtime, captured at index time, powers the recent view's
        // "newly added/changed" signal. Additive column on existing DBs.
        ensure_column_exists(&self.conn, "candidates", "fs_modified_at_unix_s", "INTEGER")?;

        Ok(())
    }
}

fn ensure_column_exists(
    conn: &Connection,
    table: &str,
    column: &str,
    column_type: &str,
) -> StorageResult<()> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == column {
            return Ok(());
        }
    }

    conn.execute(
        &format!("ALTER TABLE {table} ADD COLUMN {column} {column_type}"),
        [],
    )?;
    Ok(())
}

fn current_unix_s() -> StorageResult<i64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| StorageError::Data(format!("system time error: {err}")))
        .map(|d| d.as_secs() as i64)
}

fn to_use_count(value: i64) -> StorageResult<u64> {
    u64::try_from(value)
        .map_err(|_| StorageError::Data(format!("invalid use_count in sqlite: {value}")))
}

fn from_use_count(value: u64) -> StorageResult<i64> {
    i64::try_from(value)
        .map_err(|_| StorageError::Data(format!("use_count overflow for sqlite: {value}")))
}

/// RFC 3986 percent-encoding: unreserved characters are passed through,
/// everything else (including spaces) is encoded as `%XX`.
const HEX_CHARS: &[u8; 16] = b"0123456789ABCDEF";

pub fn percent_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            out.push('%');
            out.push(HEX_CHARS[(byte >> 4) as usize] as char);
            out.push(HEX_CHARS[(byte & 0x0F) as usize] as char);
        }
    }
    out
}

/// Form-style encoding for search query parameters: same as [`percent_encode`]
/// but encodes spaces as `+` instead of `%20`.
fn form_encode_query(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else if byte == b' ' {
            out.push('+');
        } else {
            out.push('%');
            out.push(HEX_CHARS[(byte >> 4) as usize] as char);
            out.push(HEX_CHARS[(byte & 0x0F) as usize] as char);
        }
    }
    out
}

fn kind_key(kind: &CandidateKind) -> &'static str {
    kind.as_str()
}

fn parse_kind(value: &str) -> StorageResult<CandidateKind> {
    CandidateKind::from_key(value)
        .ok_or_else(|| StorageError::Data(format!("unknown candidate kind: {value}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(id: &str, title: &str, path: &str) -> Candidate {
        Candidate {
            id: id.into(),
            kind: CandidateKind::App,
            title: title.into(),
            subtitle: Some("test subtitle".into()),
            path: path.into(),
            ..Default::default()
        }
    }

    #[test]
    fn percent_encode_leaves_unreserved_chars_intact() {
        assert_eq!(percent_encode("hello"), "hello");
        assert_eq!(percent_encode("a-b_c.d~e"), "a-b_c.d~e");
        assert_eq!(percent_encode("ABC123"), "ABC123");
    }

    #[test]
    fn percent_encode_encodes_spaces_and_special_chars() {
        assert_eq!(percent_encode("hello world"), "hello%20world");
        assert_eq!(percent_encode("a&b=c"), "a%26b%3Dc");
        assert_eq!(percent_encode("foo/bar"), "foo%2Fbar");
    }

    #[test]
    fn percent_encode_handles_unicode() {
        let encoded = percent_encode("café");
        assert!(encoded.starts_with("caf%"));
        assert!(!encoded.contains('é'));
    }

    #[test]
    fn percent_encode_handles_empty_string() {
        assert_eq!(percent_encode(""), "");
    }

    #[test]
    fn form_encode_query_encodes_spaces_as_plus() {
        assert_eq!(form_encode_query("hello world"), "hello+world");
        assert_eq!(form_encode_query("a&b"), "a%26b");
    }

    #[test]
    fn open_in_memory_runs_migrations() {
        let store = SqliteStore::open_in_memory().expect("open sqlite in memory");

        let mut stmt = store
            .conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name=?1")
            .expect("prepare sqlite_master query");

        for name in ["settings", "candidates", "usage_events", "index_state"] {
            let mut rows = stmt.query([name]).expect("query table name");
            assert!(rows.next().expect("fetch table row").is_some());
        }
    }

    #[test]
    fn upsert_and_load_candidates_round_trip() {
        let mut store = SqliteStore::open_in_memory().expect("open sqlite in memory");
        let first = candidate("app:test", "Test App", "/Applications/Test.app");

        store.upsert_candidates(&[first]).expect("insert candidate");

        let loaded = store.load_candidates(None).expect("load candidates");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id.as_ref(), "app:test");
        assert_eq!(loaded[0].title.as_ref(), "Test App");
        assert_eq!(loaded[0].path.as_ref(), "/Applications/Test.app");
        assert_eq!(loaded[0].use_count, 0);
        assert_eq!(loaded[0].last_used_at_unix_s, None);
    }

    #[test]
    fn usage_recording_and_upsert_preserves_usage_metrics() {
        let mut store = SqliteStore::open_in_memory().expect("open sqlite in memory");
        let first = candidate("app:test", "Test App", "/Applications/Test.app");

        store.upsert_candidates(&[first]).expect("insert candidate");

        store
            .record_usage_event("app:test", "open_app")
            .expect("record usage event");

        let after_usage = store
            .load_candidates(None)
            .expect("load candidates after usage");
        assert_eq!(after_usage[0].use_count, 1);
        assert!(after_usage[0].last_used_at_unix_s.is_some());

        let updated = Candidate {
            id: "app:test".into(),
            kind: CandidateKind::App,
            title: "Renamed App".into(),
            subtitle: Some("updated subtitle".into()),
            path: "/Applications/Renamed.app".into(),
            ..Default::default()
        };

        store
            .upsert_candidates(&[updated])
            .expect("upsert updated candidate");

        let final_rows = store.load_candidates(None).expect("load final candidates");
        assert_eq!(final_rows.len(), 1);
        assert_eq!(final_rows[0].title.as_ref(), "Renamed App");
        assert_eq!(final_rows[0].path.as_ref(), "/Applications/Renamed.app");
        assert_eq!(final_rows[0].use_count, 1);
        assert!(final_rows[0].last_used_at_unix_s.is_some());

        let usage_count: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM usage_events", [], |row| row.get(0))
            .expect("count usage events");
        assert_eq!(usage_count, 1);
    }

    #[test]
    fn delete_stale_candidates_removes_older_rows() {
        let mut store = SqliteStore::open_in_memory().expect("open sqlite in memory");
        let old = candidate("app:old", "Old", "/Applications/Old.app");
        let fresh = candidate("app:fresh", "Fresh", "/Applications/Fresh.app");

        store
            .upsert_candidates_indexed(&[old], Some(100))
            .expect("insert old");
        store
            .upsert_candidates_indexed(&[fresh], Some(200))
            .expect("insert fresh");

        let removed = store
            .delete_stale_candidates(150)
            .expect("delete stale candidates");
        assert_eq!(removed, 1);

        let loaded = store.load_candidates(None).expect("load candidates");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id.as_ref(), "app:fresh");
    }

    #[test]
    fn delete_stale_candidates_also_cleans_null_indexed_rows() {
        let mut store = SqliteStore::open_in_memory().expect("open sqlite in memory");
        let old_style = candidate("app:null", "Null Indexed", "/Applications/Null.app");
        store
            .upsert_candidates_indexed(&[old_style], None)
            .expect("insert null-indexed row");

        let removed = store
            .delete_stale_candidates(1)
            .expect("delete stale including null");
        assert_eq!(removed, 1);

        let loaded = store.load_candidates(None).expect("load candidates");
        assert!(loaded.is_empty());
    }

    #[test]
    fn upsert_skips_write_when_row_unchanged() {
        let mut store = SqliteStore::open_in_memory().expect("open sqlite in memory");
        let c = candidate("file:a", "a.txt", "/x/a.txt");
        store
            .upsert_candidates_indexed(std::slice::from_ref(&c), Some(100))
            .expect("first insert");
        // Re-upsert identical content on a later run: the WHERE guard should skip
        // the UPDATE, so indexed_at stays at the original value (no rewrite).
        store
            .upsert_candidates_indexed(&[c], Some(200))
            .expect("no-op upsert");
        let indexed: i64 = store
            .conn
            .query_row(
                "SELECT indexed_at_unix_s FROM candidates WHERE id = 'file:a'",
                [],
                |row| row.get(0),
            )
            .expect("read indexed_at");
        assert_eq!(indexed, 100, "unchanged row must not be rewritten");
    }

    #[test]
    fn upsert_writes_when_content_changes() {
        let mut store = SqliteStore::open_in_memory().expect("open sqlite in memory");
        store
            .upsert_candidates_indexed(&[candidate("file:a", "a.txt", "/x/a.txt")], Some(100))
            .expect("first insert");
        store
            .upsert_candidates_indexed(&[candidate("file:a", "renamed.txt", "/x/a.txt")], Some(200))
            .expect("changed upsert");
        let indexed: i64 = store
            .conn
            .query_row(
                "SELECT indexed_at_unix_s FROM candidates WHERE id = 'file:a'",
                [],
                |row| row.get(0),
            )
            .expect("read indexed_at");
        assert_eq!(indexed, 200, "changed row must be rewritten");
        let loaded = store.load_candidates(None).expect("load");
        assert_eq!(loaded[0].title.as_ref(), "renamed.txt");
    }

    #[test]
    fn delete_unseen_removes_unseen_keeps_seen_and_pinned() {
        let mut store = SqliteStore::open_in_memory().expect("open sqlite in memory");
        store
            .upsert_candidates_indexed(
                &[
                    candidate("file:a", "a", "/x/a"),
                    candidate("file:b", "b", "/x/b"),
                ],
                Some(100),
            )
            .expect("insert files");
        // Pinned/seeded row uses i64::MAX indexed_at to survive sweeps.
        store
            .upsert_candidates_indexed(&[candidate("uwp:pinned", "Pinned", "/p")], Some(i64::MAX))
            .expect("insert pinned");

        // The latest scan only saw file:a.
        let mut seen: HashSet<Box<str>> = HashSet::new();
        seen.insert("file:a".into());
        let removed = store
            .delete_unseen_candidates(&seen, 200, &[])
            .expect("prune unseen");
        assert_eq!(removed, 1);

        let ids: Vec<String> = store
            .load_candidates(None)
            .expect("load")
            .iter()
            .map(|c| c.id.to_string())
            .collect();
        assert!(ids.contains(&"file:a".to_string()), "seen row kept");
        assert!(ids.contains(&"uwp:pinned".to_string()), "pinned row kept");
        assert!(!ids.contains(&"file:b".to_string()), "unseen row removed");
    }

    #[test]
    fn delete_unseen_respects_prefixes() {
        let mut store = SqliteStore::open_in_memory().expect("open sqlite in memory");
        store
            .upsert_candidates_indexed(
                &[
                    candidate("app:x", "X", "/x"),
                    candidate("file:y", "Y", "/y"),
                ],
                Some(100),
            )
            .expect("insert");
        // Scoped (apps-only) refresh saw nothing: only app:* may be pruned.
        let seen: HashSet<Box<str>> = HashSet::new();
        let removed = store
            .delete_unseen_candidates(&seen, 200, &["app:"])
            .expect("scoped prune");
        assert_eq!(removed, 1);
        let ids: Vec<String> = store
            .load_candidates(None)
            .expect("load")
            .iter()
            .map(|c| c.id.to_string())
            .collect();
        assert!(ids.contains(&"file:y".to_string()), "out-of-scope row kept");
        assert!(
            !ids.contains(&"app:x".to_string()),
            "in-scope unseen removed"
        );
    }

    #[test]
    fn is_demo_seeded_recognizes_demo_rows() {
        let mut store = SqliteStore::open_in_memory().expect("open sqlite in memory");
        let safari = candidate("app:safari", "Safari", "/Applications/Safari.app");
        let vscode = candidate("app:vscode", "VS Code", "/Applications/Code.app");
        store
            .upsert_candidates_indexed(&[safari, vscode], Some(100))
            .expect("seed demo rows");

        assert!(store.is_demo_seeded().expect("query"));
    }

    #[test]
    fn is_demo_seeded_returns_false_for_empty_table() {
        let store = SqliteStore::open_in_memory().expect("open sqlite in memory");
        assert!(!store.is_demo_seeded().expect("query"));
    }

    #[test]
    fn is_demo_seeded_returns_false_when_real_data_present() {
        let mut store = SqliteStore::open_in_memory().expect("open sqlite in memory");
        // > 6 rows means we're past the demo seed regardless of contents.
        let rows: Vec<Candidate> = (0..10)
            .map(|i| {
                candidate(
                    &format!("app:user-app-{i}"),
                    &format!("App {i}"),
                    &format!("/Applications/App{i}.app"),
                )
            })
            .collect();
        store
            .upsert_candidates_indexed(&rows, Some(100))
            .expect("seed");
        assert!(!store.is_demo_seeded().expect("query"));
    }

    #[test]
    fn is_demo_seeded_returns_false_for_partial_demo_match() {
        let mut store = SqliteStore::open_in_memory().expect("open sqlite in memory");
        // Only one demo marker - not the full seed.
        let only_safari = candidate("app:safari", "Safari", "/Applications/Safari.app");
        store
            .upsert_candidates_indexed(&[only_safari], Some(100))
            .expect("seed");
        assert!(!store.is_demo_seeded().expect("query"));
    }

    #[test]
    fn delete_stale_candidates_with_prefixes_is_noop_on_empty_prefix_list() {
        let mut store = SqliteStore::open_in_memory().expect("open sqlite in memory");
        let old = candidate("app:old", "Old", "/Applications/Old.app");
        store
            .upsert_candidates_indexed(&[old], Some(100))
            .expect("insert");

        let removed = store
            .delete_stale_candidates_with_prefixes(200, &[])
            .expect("noop");
        assert_eq!(removed, 0);
        let loaded = store.load_candidates(None).expect("load");
        assert_eq!(loaded.len(), 1, "row must be preserved");
    }

    #[test]
    fn delete_stale_candidates_with_prefixes_accepts_multiple_prefixes() {
        let mut store = SqliteStore::open_in_memory().expect("open sqlite in memory");
        let old_app = candidate("app:old", "Old App", "/Applications/Old.app");
        let old_file = candidate("file:old", "Old File", "/Users/demo/old.txt");
        let old_folder = candidate("folder:old", "Old Folder", "/Users/demo/old");
        let old_setting = candidate("setting:old", "Old Setting", "settings://old");

        store
            .upsert_candidates_indexed(&[old_app, old_file, old_folder, old_setting], Some(100))
            .expect("seed");

        // Sweep apps + files (mirrors what BootstrapScope::FILES_ONLY produces
        // via id_prefixes() - file + folder together - plus apps).
        let removed = store
            .delete_stale_candidates_with_prefixes(200, &["app:", "file:", "folder:"])
            .expect("sweep");
        assert_eq!(removed, 3);

        let ids: Vec<String> = store
            .load_candidates(None)
            .expect("load")
            .into_iter()
            .map(|c| c.id.as_ref().to_string())
            .collect();
        assert_eq!(ids, vec!["setting:old"]);
    }

    #[test]
    fn delete_stale_candidates_with_prefixes_purges_matching_usage_events() {
        let mut store = SqliteStore::open_in_memory().expect("open sqlite in memory");
        let old_app = candidate("app:old", "Old App", "/Applications/Old.app");
        let old_file = candidate("file:old", "Old File", "/Users/demo/old.txt");
        store
            .upsert_candidates_indexed(&[old_app.clone(), old_file.clone()], Some(100))
            .expect("seed");
        store
            .record_usage_event(old_app.id.as_ref(), "open_app")
            .expect("record app usage");
        store
            .record_usage_event(old_file.id.as_ref(), "open_file")
            .expect("record file usage");

        let removed = store
            .delete_stale_candidates_with_prefixes(200, &["app:"])
            .expect("sweep apps");
        assert_eq!(removed, 1);

        // app:old's usage_events row must be gone; file:old's must remain.
        let app_usage: i64 = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM usage_events WHERE candidate_id = ?1",
                params!["app:old"],
                |row| row.get(0),
            )
            .expect("count app usage");
        assert_eq!(app_usage, 0);
        let file_usage: i64 = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM usage_events WHERE candidate_id = ?1",
                params!["file:old"],
                |row| row.get(0),
            )
            .expect("count file usage");
        assert_eq!(file_usage, 1);
    }

    #[test]
    fn delete_stale_candidates_with_prefixes_only_touches_matching_kinds() {
        let mut store = SqliteStore::open_in_memory().expect("open sqlite in memory");
        let old_app = candidate("app:old", "Old App", "/Applications/Old.app");
        let old_file = candidate("file:old", "Old File", "/Users/demo/old.txt");
        let fresh_app = candidate("app:fresh", "Fresh App", "/Applications/Fresh.app");
        let fresh_file = candidate("file:fresh", "Fresh File", "/Users/demo/fresh.txt");

        store
            .upsert_candidates_indexed(&[old_app, old_file], Some(100))
            .expect("insert old rows");
        store
            .upsert_candidates_indexed(&[fresh_app, fresh_file], Some(200))
            .expect("insert fresh rows");

        let removed = store
            .delete_stale_candidates_with_prefixes(150, &["app:"])
            .expect("prune stale apps");
        assert_eq!(removed, 1);

        let ids: Vec<String> = store
            .load_candidates(None)
            .expect("load")
            .into_iter()
            .map(|c| c.id.as_ref().to_string())
            .collect();
        assert!(!ids.contains(&"app:old".to_string()));
        assert!(ids.contains(&"app:fresh".to_string()));
        // file:old is older than the cutoff but must be untouched because we
        // restricted the sweep to the `app:` prefix.
        assert!(ids.contains(&"file:old".to_string()));
        assert!(ids.contains(&"file:fresh".to_string()));
    }

    #[test]
    fn delete_candidates_by_prefix_except_removes_vanished_rows_and_preserves_kept() {
        let mut store = SqliteStore::open_in_memory().expect("open sqlite in memory");
        let kept = candidate(
            "app:uwp:Microsoft.WindowsTerminal_8wekyb3d8bbwe!App",
            "Terminal",
            "shell:AppsFolder\\Microsoft.WindowsTerminal_8wekyb3d8bbwe!App",
        );
        let stale = candidate(
            "app:uwp:Old.PackageThatGotUninstalled_abc!App",
            "Old App",
            "shell:AppsFolder\\Old.PackageThatGotUninstalled_abc!App",
        );
        let win32 = candidate(
            "app:edge_c:/programdata/microsoft/windows/start menu/programs/microsoft edge.lnk",
            "Microsoft Edge",
            "C:/ProgramData/Microsoft/Windows/Start Menu/Programs/Microsoft Edge.lnk",
        );

        store
            .upsert_candidates_indexed(&[kept, stale.clone(), win32], Some(i64::MAX))
            .expect("seed candidates");

        // Record usage on the stale row so we can verify usage_events are also removed.
        store
            .record_usage_event(stale.id.as_ref(), "open_app")
            .expect("record stale usage");

        let mut keep_set = HashSet::new();
        keep_set.insert("app:uwp:Microsoft.WindowsTerminal_8wekyb3d8bbwe!App");

        let removed = store
            .delete_candidates_by_prefix_except("app:uwp:", &keep_set)
            .expect("prune");
        assert_eq!(removed, 1);

        let loaded = store.load_candidates(None).expect("load");
        let ids: Vec<&str> = loaded.iter().map(|c| c.id.as_ref()).collect();
        assert!(ids.contains(&"app:uwp:Microsoft.WindowsTerminal_8wekyb3d8bbwe!App"));
        assert!(!ids.contains(&"app:uwp:Old.PackageThatGotUninstalled_abc!App"));
        // The non-uwp `app:` row must be untouched even though it shares the broader
        // `app:` prefix - only `app:uwp:` rows are eligible for this sweep.
        assert!(ids.contains(
            &"app:edge_c:/programdata/microsoft/windows/start menu/programs/microsoft edge.lnk"
        ));

        // usage_events for the deleted candidate should also be cleaned up.
        let usage_count: i64 = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM usage_events WHERE candidate_id = ?1",
                params![stale.id.as_ref()],
                |row| row.get(0),
            )
            .expect("count usage events");
        assert_eq!(usage_count, 0);
    }

    #[test]
    fn delete_candidates_by_prefix_except_is_noop_when_all_kept() {
        let mut store = SqliteStore::open_in_memory().expect("open sqlite in memory");
        let cand = candidate(
            "app:uwp:Microsoft.Notepad_8wekyb3d8bbwe!App",
            "Notepad",
            "shell:AppsFolder\\Microsoft.Notepad_8wekyb3d8bbwe!App",
        );
        store
            .upsert_candidates_indexed(&[cand], Some(i64::MAX))
            .expect("seed");

        let mut keep_set = HashSet::new();
        keep_set.insert("app:uwp:Microsoft.Notepad_8wekyb3d8bbwe!App");

        let removed = store
            .delete_candidates_by_prefix_except("app:uwp:", &keep_set)
            .expect("prune");
        assert_eq!(removed, 0);
    }

    #[test]
    fn prune_usage_events_applies_age_and_count_limits() {
        let mut store = SqliteStore::open_in_memory().expect("open sqlite in memory");
        let first = candidate("app:test", "Test App", "/Applications/Test.app");
        store.upsert_candidates(&[first]).expect("insert candidate");

        store
            .conn
            .execute(
                "INSERT INTO usage_events(candidate_id, action, used_at_unix_s) VALUES (?1, ?2, ?3)",
                params!["app:test", "open", 100_i64],
            )
            .expect("insert old usage");
        store
            .conn
            .execute(
                "INSERT INTO usage_events(candidate_id, action, used_at_unix_s) VALUES (?1, ?2, ?3)",
                params!["app:test", "open", 200_i64],
            )
            .expect("insert usage 2");
        store
            .conn
            .execute(
                "INSERT INTO usage_events(candidate_id, action, used_at_unix_s) VALUES (?1, ?2, ?3)",
                params!["app:test", "open", 300_i64],
            )
            .expect("insert usage 3");

        let removed_old = store
            .prune_usage_events_older_than(150)
            .expect("prune old usage");
        assert_eq!(removed_old, 1);

        let removed_overflow = store
            .prune_usage_events_to_max(1)
            .expect("prune usage overflow");
        assert_eq!(removed_overflow, 1);

        let remaining: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM usage_events", [], |row| row.get(0))
            .expect("count remaining usage");
        assert_eq!(remaining, 1);
    }
}
