use look_indexing::{Candidate, CandidateKind, ClipboardContentType, ClipboardItem};
use rusqlite::{Connection, params};
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

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
            SearchEngine::DuckDuckGo => "duckduckgo",
            SearchEngine::Google => "google",
            SearchEngine::Bing => "bing",
        }
    }

    pub fn from_key(value: &str) -> Self {
        match value {
            "duckduckgo" => SearchEngine::DuckDuckGo,
            "google" => SearchEngine::Google,
            "bing" => SearchEngine::Bing,
            _ => SearchEngine::Google,
        }
    }

    pub fn build_search_url(self, query: &str) -> String {
        let encoded = percent_encode_query(query);
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
            .get("web_search_enabled")
            .map(|value| value == "true")
            .unwrap_or(true);
        let engine = SearchEngine::from_key(self.get("web_search_engine").unwrap_or("google"));
        SearchSettings {
            web_search_enabled: enabled,
            web_search_engine: engine,
        }
    }

    pub fn set_search_settings(&mut self, settings: SearchSettings) {
        self.set(
            "web_search_enabled",
            if settings.web_search_enabled {
                "true"
            } else {
                "false"
            },
        );
        self.set("web_search_engine", settings.web_search_engine.key());
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

    pub fn load_candidates(&self, limit: Option<usize>) -> StorageResult<Vec<Candidate>> {
        let sql = match limit {
            Some(_) => {
                "SELECT id, kind, title, subtitle, path, use_count, last_used_at_unix_s FROM candidates ORDER BY title ASC LIMIT ?1"
            }
            None => {
                "SELECT id, kind, title, subtitle, path, use_count, last_used_at_unix_s FROM candidates ORDER BY title ASC"
            }
        };

        let mut stmt = self.conn.prepare(sql)?;
        let mut rows = match limit {
            Some(max) => stmt.query([max as i64])?,
            None => stmt.query([])?,
        };

        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            let kind_raw: String = row.get(1)?;
            out.push(Candidate {
                id: row.get(0)?,
                kind: parse_kind(&kind_raw)?,
                title: row.get(2)?,
                subtitle: row.get(3)?,
                path: row.get(4)?,
                use_count: row.get(5)?,
                last_used_at_unix_s: row.get(6)?,
            });
        }

        Ok(out)
    }

    pub fn upsert_candidates(&mut self, candidates: &[Candidate]) -> StorageResult<()> {
        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO candidates (id, kind, title, subtitle, path, use_count, last_used_at_unix_s)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(id) DO UPDATE SET
                   kind = excluded.kind,
                   title = excluded.title,
                   subtitle = excluded.subtitle,
                    path = excluded.path",
            )?;

            for candidate in candidates {
                stmt.execute(params![
                    candidate.id,
                    kind_key(&candidate.kind),
                    candidate.title,
                    candidate.subtitle,
                    candidate.path,
                    candidate.use_count,
                    candidate.last_used_at_unix_s,
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
                "INSERT INTO candidates (id, kind, title, subtitle, path, use_count, last_used_at_unix_s)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;

            for candidate in candidates {
                stmt.execute(params![
                    candidate.id,
                    kind_key(&candidate.kind),
                    candidate.title,
                    candidate.subtitle,
                    candidate.path,
                    candidate.use_count,
                    candidate.last_used_at_unix_s,
                ])?;
            }
        }

        tx.commit()?;
        Ok(())
    }

    pub fn load_search_settings(&self) -> StorageResult<SearchSettings> {
        let mut settings = SearchSettings::default();
        let mut stmt = self
            .conn
            .prepare("SELECT key, value FROM settings WHERE key IN ('web_search_enabled', 'web_search_engine')")?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let key: String = row.get(0)?;
            let value: String = row.get(1)?;
            match key.as_str() {
                "web_search_enabled" => settings.web_search_enabled = value == "true",
                "web_search_engine" => settings.web_search_engine = SearchEngine::from_key(&value),
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
                "web_search_enabled",
                if settings.web_search_enabled {
                    "true"
                } else {
                    "false"
                }
            ],
        )?;
        tx.execute(
            "INSERT INTO settings(key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params!["web_search_engine", settings.web_search_engine.key()],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn record_usage_event(&self, candidate_id: &str, action: &str) -> StorageResult<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| StorageError::Data(format!("system time error: {err}")))?
            .as_secs() as i64;

        self.conn.execute(
            "INSERT INTO usage_events(candidate_id, action, used_at_unix_s) VALUES (?1, ?2, ?3)",
            params![candidate_id, action, now],
        )?;

        self.conn.execute(
            "UPDATE candidates SET use_count = use_count + 1, last_used_at_unix_s = ?2 WHERE id = ?1",
            params![candidate_id, now],
        )?;
        Ok(())
    }

    pub fn insert_clipboard_item(&self, item: &ClipboardItem) -> StorageResult<()> {
        self.conn.execute(
            "INSERT INTO clipboard_history (id, content_type, content, preview, source_app, created_at_unix_s, last_used_at_unix_s, use_count, pinned)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(id) DO UPDATE SET
               last_used_at_unix_s = excluded.created_at_unix_s,
               use_count = clipboard_history.use_count + 1",
            params![
                item.id,
                item.content_type.to_string(),
                item.content,
                item.preview,
                item.source_app,
                item.created_at_unix_s,
                item.last_used_at_unix_s,
                item.use_count,
                item.pinned as i32,
            ],
        )?;
        Ok(())
    }

    pub fn load_clipboard_items(
        &self,
        query: Option<&str>,
        content_type: Option<&str>,
        limit: usize,
    ) -> StorageResult<Vec<ClipboardItem>> {
        let mut conditions = Vec::new();
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(q) = query {
            if !q.trim().is_empty() {
                conditions.push(format!("content LIKE ?{}", param_values.len() + 1));
                param_values.push(Box::new(format!("%{q}%")));
            }
        }

        if let Some(ct) = content_type {
            if !ct.trim().is_empty() {
                conditions.push(format!("content_type = ?{}", param_values.len() + 1));
                param_values.push(Box::new(ct.to_string()));
            }
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let sql = format!(
            "SELECT id, content_type, content, preview, source_app, created_at_unix_s, last_used_at_unix_s, use_count, pinned
             FROM clipboard_history
             {where_clause}
             ORDER BY pinned DESC, created_at_unix_s DESC
             LIMIT ?{}",
            param_values.len() + 1
        );

        param_values.push(Box::new(limit as i64));

        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();

        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query(params_refs.as_slice())?;

        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            let ct_raw: String = row.get(1)?;
            let pinned_raw: i32 = row.get(8)?;
            out.push(ClipboardItem {
                id: row.get(0)?,
                content_type: parse_clipboard_content_type(&ct_raw)?,
                content: row.get(2)?,
                preview: row.get(3)?,
                source_app: row.get(4)?,
                created_at_unix_s: row.get(5)?,
                last_used_at_unix_s: row.get(6)?,
                use_count: row.get(7)?,
                pinned: pinned_raw != 0,
            });
        }

        Ok(out)
    }

    pub fn delete_clipboard_item(&self, item_id: &str) -> StorageResult<bool> {
        let count = self.conn.execute(
            "DELETE FROM clipboard_history WHERE id = ?1",
            params![item_id],
        )?;
        Ok(count > 0)
    }

    pub fn clear_clipboard_history(&self, older_than_unix_s: Option<i64>) -> StorageResult<u64> {
        let count = match older_than_unix_s {
            Some(ts) => self.conn.execute(
                "DELETE FROM clipboard_history WHERE pinned = 0 AND created_at_unix_s < ?1",
                params![ts],
            )?,
            None => self.conn.execute(
                "DELETE FROM clipboard_history WHERE pinned = 0",
                [],
            )?,
        };
        Ok(count as u64)
    }

    pub fn toggle_clipboard_pin(&self, item_id: &str) -> StorageResult<bool> {
        self.conn.execute(
            "UPDATE clipboard_history SET pinned = CASE WHEN pinned = 0 THEN 1 ELSE 0 END WHERE id = ?1",
            params![item_id],
        )?;

        let pinned: i32 = self.conn.query_row(
            "SELECT pinned FROM clipboard_history WHERE id = ?1",
            params![item_id],
            |row| row.get(0),
        )?;
        Ok(pinned != 0)
    }

    pub fn record_clipboard_usage(&self, item_id: &str) -> StorageResult<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| StorageError::Data(format!("system time error: {err}")))?
            .as_secs() as i64;

        self.conn.execute(
            "UPDATE clipboard_history SET use_count = use_count + 1, last_used_at_unix_s = ?2 WHERE id = ?1",
            params![item_id, now],
        )?;
        Ok(())
    }

    fn migrate(&self) -> StorageResult<()> {
        self.conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;

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
                 last_used_at_unix_s INTEGER
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
             );

             CREATE TABLE IF NOT EXISTS clipboard_history (
                 id TEXT PRIMARY KEY,
                 content_type TEXT NOT NULL DEFAULT 'text',
                 content TEXT NOT NULL,
                 preview TEXT,
                 source_app TEXT,
                 created_at_unix_s INTEGER NOT NULL,
                 last_used_at_unix_s INTEGER,
                 use_count INTEGER NOT NULL DEFAULT 0,
                 pinned INTEGER NOT NULL DEFAULT 0
             );

             CREATE INDEX IF NOT EXISTS idx_clipboard_created ON clipboard_history(created_at_unix_s DESC);
             CREATE INDEX IF NOT EXISTS idx_clipboard_content_type ON clipboard_history(content_type);
             CREATE INDEX IF NOT EXISTS idx_clipboard_pinned ON clipboard_history(pinned);",
        )?;

        Ok(())
    }
}

fn percent_encode_query(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else if byte == b' ' {
            out.push('+');
        } else {
            out.push('%');
            out.push_str(&format!("{byte:02X}"));
        }
    }
    out
}

fn kind_key(kind: &CandidateKind) -> &'static str {
    match kind {
        CandidateKind::App => "app",
        CandidateKind::File => "file",
        CandidateKind::Folder => "folder",
    }
}

fn parse_clipboard_content_type(value: &str) -> StorageResult<ClipboardContentType> {
    match value {
        "text" => Ok(ClipboardContentType::Text),
        "image" => Ok(ClipboardContentType::Image),
        "file_list" => Ok(ClipboardContentType::FileList),
        other => Err(StorageError::Data(format!(
            "unknown clipboard content type: {other}"
        ))),
    }
}

fn parse_kind(value: &str) -> StorageResult<CandidateKind> {
    match value {
        "app" => Ok(CandidateKind::App),
        "file" => Ok(CandidateKind::File),
        "folder" => Ok(CandidateKind::Folder),
        other => Err(StorageError::Data(format!(
            "unknown candidate kind: {other}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(id: &str, title: &str, path: &str) -> Candidate {
        Candidate {
            id: id.to_string(),
            kind: CandidateKind::App,
            title: title.to_string(),
            subtitle: Some("test subtitle".to_string()),
            path: path.to_string(),
            use_count: 0,
            last_used_at_unix_s: None,
        }
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
        assert_eq!(loaded[0].id, "app:test");
        assert_eq!(loaded[0].title, "Test App");
        assert_eq!(loaded[0].path, "/Applications/Test.app");
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
            id: "app:test".to_string(),
            kind: CandidateKind::App,
            title: "Renamed App".to_string(),
            subtitle: Some("updated subtitle".to_string()),
            path: "/Applications/Renamed.app".to_string(),
            use_count: 0,
            last_used_at_unix_s: None,
        };

        store
            .upsert_candidates(&[updated])
            .expect("upsert updated candidate");

        let final_rows = store.load_candidates(None).expect("load final candidates");
        assert_eq!(final_rows.len(), 1);
        assert_eq!(final_rows[0].title, "Renamed App");
        assert_eq!(final_rows[0].path, "/Applications/Renamed.app");
        assert_eq!(final_rows[0].use_count, 1);
        assert!(final_rows[0].last_used_at_unix_s.is_some());

        let usage_count: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM usage_events", [], |row| row.get(0))
            .expect("count usage events");
        assert_eq!(usage_count, 1);
    }
}
