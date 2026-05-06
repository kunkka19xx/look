use look_engine::QueryEngine;
use look_engine::config::RuntimeConfig;
use notify::event::{ModifyKind, RenameMode};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::env;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, RwLock, mpsc};
use std::thread;
use std::time::Instant;

pub struct AppState {
    engine: RwLock<QueryEngine>,
    index_change_version: AtomicU64,
    index_cleared_version: AtomicU64,
    index_refresh_in_progress: AtomicBool,
    watcher_control: Mutex<Option<mpsc::Sender<()>>>,
}

impl AppState {
    pub fn new() -> Self {
        let path = default_db_path();
        let engine = QueryEngine::from_sqlite(&path).unwrap_or_else(|_| QueryEngine::demo_seed());

        let state = Self {
            engine: RwLock::new(engine),
            index_change_version: AtomicU64::new(0),
            index_cleared_version: AtomicU64::new(0),
            index_refresh_in_progress: AtomicBool::new(false),
            watcher_control: Mutex::new(None),
        };

        state.start_background_bootstrap();
        state.start_index_watchers();
        state
    }

    pub fn with_engine<T>(&self, f: impl FnOnce(&QueryEngine) -> T) -> T {
        let guard = self
            .engine
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        f(&guard)
    }

    pub fn with_engine_mut<T>(&self, f: impl FnOnce(&mut QueryEngine) -> T) -> T {
        let mut guard = self
            .engine
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        f(&mut guard)
    }

    pub fn request_index_refresh(&self) -> bool {
        let config = RuntimeConfig::load();
        if config.lazy_indexing_enabled && !self.is_index_dirty() {
            return false;
        }
        if !self.try_acquire_refresh_slot() {
            return false;
        }

        let dirty_snapshot = self.index_change_version.load(Ordering::Acquire);

        // We need to spawn a thread that does the refresh. Since we can't move &self
        // into the thread, we'll use the same global approach but through a helper.
        let db_path = default_db_path();
        let engine_lock = &self.engine as *const RwLock<QueryEngine>;
        let change_version = &self.index_change_version as *const AtomicU64;
        let cleared_version = &self.index_cleared_version as *const AtomicU64;
        let in_progress = &self.index_refresh_in_progress as *const AtomicBool;

        // SAFETY: AppState is managed by Tauri and lives for the app's lifetime.
        // The spawned thread will complete before the app exits.
        unsafe {
            let engine_lock = &*engine_lock;
            let change_version = &*change_version;
            let cleared_version = &*cleared_version;
            let in_progress = &*in_progress;

            thread::spawn(move || {
                let started_at = Instant::now();
                match QueryEngine::bootstrap_sqlite(&db_path) {
                    Ok(()) => {
                        if let Ok(new_engine) = QueryEngine::from_sqlite(&db_path) {
                            let mut guard = engine_lock
                                .write()
                                .unwrap_or_else(|poisoned| poisoned.into_inner());
                            *guard = new_engine;
                        }
                        if change_version.load(Ordering::Acquire) == dirty_snapshot {
                            cleared_version.store(dirty_snapshot, Ordering::Release);
                        }
                        eprintln!(
                            "look: index refresh ok elapsed_ms={}",
                            started_at.elapsed().as_millis()
                        );
                    }
                    Err(err) => {
                        change_version.fetch_add(1, Ordering::AcqRel);
                        eprintln!("look: index refresh failed error={err}");
                    }
                }
                in_progress.store(false, Ordering::Release);
            });
        }

        true
    }

    fn start_background_bootstrap(&self) {
        let db_path = default_db_path();
        let engine_lock = &self.engine as *const RwLock<QueryEngine>;
        let change_version = &self.index_change_version as *const AtomicU64;
        let cleared_version = &self.index_cleared_version as *const AtomicU64;

        // SAFETY: AppState lives for the app's lifetime.
        unsafe {
            let engine_lock = &*engine_lock;
            let change_version = &*change_version;
            let cleared_version = &*cleared_version;

            thread::spawn(move || {
                let started_at = Instant::now();
                let dirty_snapshot = change_version.load(Ordering::Acquire);
                match QueryEngine::bootstrap_sqlite(&db_path) {
                    Ok(()) => {
                        if let Ok(new_engine) = QueryEngine::from_sqlite(&db_path) {
                            let mut guard = engine_lock
                                .write()
                                .unwrap_or_else(|poisoned| poisoned.into_inner());
                            *guard = new_engine;
                        }
                        if change_version.load(Ordering::Acquire) == dirty_snapshot {
                            cleared_version.store(dirty_snapshot, Ordering::Release);
                        }
                        eprintln!(
                            "look: bootstrap ok elapsed_ms={}",
                            started_at.elapsed().as_millis()
                        );
                    }
                    Err(err) => {
                        change_version.fetch_add(1, Ordering::AcqRel);
                        eprintln!("look: bootstrap failed error={err}");
                    }
                }
            });
        }
    }

    fn start_index_watchers(&self) {
        let config = RuntimeConfig::load();
        if !config.lazy_indexing_enabled {
            return;
        }

        let mut roots = config.app_scan_roots;
        roots.extend(config.file_scan_roots);
        roots.extend(config.file_scan_extra_roots);

        // Include Linux additional app roots that the indexer also scans
        #[cfg(target_os = "linux")]
        {
            if let Ok(home) = std::env::var("HOME") {
                let home = home.trim().to_string();
                if !home.is_empty() {
                    roots.push(format!("{home}/.local/share/applications"));
                }
            }
            if let Ok(data_dirs) = std::env::var("XDG_DATA_DIRS") {
                for dir in data_dirs.split(':') {
                    let dir = dir.trim();
                    if !dir.is_empty() {
                        roots.push(format!("{dir}/applications"));
                    }
                }
            }
        }

        roots.sort();
        roots.dedup();

        let active_roots: Vec<String> = roots
            .into_iter()
            .filter(|root| !root.trim().is_empty() && Path::new(root).exists())
            .collect();

        if active_roots.is_empty() {
            return;
        }

        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        {
            let mut guard = self
                .watcher_control
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *guard = Some(stop_tx);
        }

        let change_version = &self.index_change_version as *const AtomicU64;

        // SAFETY: AppState lives for the app's lifetime.
        unsafe {
            let change_version = &*change_version;

            thread::spawn(move || {
                let (event_tx, event_rx) = mpsc::channel::<notify::Result<Event>>();
                let mut watcher = match RecommendedWatcher::new(
                    move |result| {
                        let _ = event_tx.send(result);
                    },
                    notify::Config::default(),
                ) {
                    Ok(w) => w,
                    Err(_) => return,
                };

                for root in &active_roots {
                    let _ = watcher.watch(Path::new(root), RecursiveMode::Recursive);
                }

                loop {
                    if stop_rx.try_recv().is_ok() {
                        break;
                    }

                    match event_rx.recv_timeout(std::time::Duration::from_secs(1)) {
                        Ok(Ok(event)) => {
                            if should_mark_dirty(&event) {
                                change_version.fetch_add(1, Ordering::AcqRel);
                            }
                        }
                        Ok(Err(_)) => {}
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
            });
        }
    }

    fn is_index_dirty(&self) -> bool {
        self.index_change_version.load(Ordering::Acquire)
            != self.index_cleared_version.load(Ordering::Acquire)
    }

    fn try_acquire_refresh_slot(&self) -> bool {
        self.index_refresh_in_progress
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }
}

pub fn default_db_path() -> PathBuf {
    if let Ok(custom) = env::var("LOOK_DB_PATH") {
        let trimmed = custom.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(base) = env::var("LOCALAPPDATA") {
            let trimmed = base.trim();
            if !trimmed.is_empty() {
                return PathBuf::from(trimmed).join("look").join("look.db");
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(data_home) = env::var("XDG_DATA_HOME") {
            let trimmed = data_home.trim();
            if !trimmed.is_empty() {
                return PathBuf::from(trimmed).join("look").join("look.db");
            }
        }
        if let Ok(home) = env::var("HOME") {
            return PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("look")
                .join("look.db");
        }
    }

    // Fallback
    let home = env::var("HOME")
        .or_else(|_| env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".look").join("look.db")
}

fn should_mark_dirty(event: &Event) -> bool {
    if event.paths.is_empty() {
        return false;
    }

    matches!(
        event.kind,
        EventKind::Create(_)
            | EventKind::Remove(_)
            | EventKind::Any
            | EventKind::Modify(ModifyKind::Name(RenameMode::Any))
            | EventKind::Modify(ModifyKind::Name(RenameMode::Both))
            | EventKind::Modify(ModifyKind::Name(RenameMode::From))
            | EventKind::Modify(ModifyKind::Name(RenameMode::To))
            | EventKind::Modify(ModifyKind::Name(RenameMode::Other))
    )
}
