use crate::runtime_config::{log_error, log_info};
use look_engine::QueryEngine;
use look_engine::config::RuntimeConfig;
use notify::event::{ModifyKind, RenameMode};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::env;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Mutex, OnceLock, RwLock};
use std::thread;
use std::time::{Duration, Instant};

static ENGINE_CACHE: OnceLock<RwLock<QueryEngine>> = OnceLock::new();
static JSON_ALLOCS: OnceLock<Mutex<HashMap<usize, CString>>> = OnceLock::new();
static BOOTSTRAP_REFRESH_STARTED: OnceLock<()> = OnceLock::new();
static INDEX_WATCHER_BOOTSTRAP_STARTED: OnceLock<()> = OnceLock::new();
static INDEX_REFRESH_IN_PROGRESS: AtomicBool = AtomicBool::new(false);
static INDEX_CHANGE_VERSION: AtomicU64 = AtomicU64::new(0);
static INDEX_CLEARED_VERSION: AtomicU64 = AtomicU64::new(0);
static INDEX_WATCHER_CONTROL: OnceLock<Mutex<Option<mpsc::Sender<()>>>> = OnceLock::new();

pub(crate) fn default_db_path() -> PathBuf {
    if let Ok(custom) = env::var("LOOK_DB_PATH")
        && !custom.trim().is_empty()
    {
        return PathBuf::from(custom);
    }

    #[cfg(target_os = "windows")]
    if let Some(path) = windows_default_db_path() {
        return path;
    }

    legacy_default_db_path()
}

fn legacy_default_db_path() -> PathBuf {
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join("Library")
        .join("Application Support")
        .join("look")
        .join("look.db")
}

#[cfg(target_os = "windows")]
fn windows_default_db_path() -> Option<PathBuf> {
    env::var("LOCALAPPDATA")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|base| PathBuf::from(base).join("look").join("look.db"))
}

pub(crate) fn with_engine<T>(f: impl FnOnce(&QueryEngine) -> T) -> T {
    let lock = engine_cache();
    let guard = lock.read().unwrap_or_else(|poisoned| poisoned.into_inner());
    f(&guard)
}

pub(crate) fn with_engine_mut<T>(f: impl FnOnce(&mut QueryEngine) -> T) -> T {
    let lock = engine_cache();
    let mut guard = lock
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    f(&mut guard)
}

pub(crate) fn refresh_engine_cache() {
    if let Some(lock) = ENGINE_CACHE.get() {
        let path = default_db_path();
        if let Ok(engine) = QueryEngine::from_sqlite(path) {
            let mut guard = lock
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *guard = engine;
        }
    }
}

pub(crate) fn request_background_index_refresh() -> bool {
    request_background_index_refresh_internal()
}

pub(crate) fn mark_index_dirty() {
    INDEX_CHANGE_VERSION.fetch_add(1, Ordering::AcqRel);
}

pub(crate) fn clear_index_dirty_if_unchanged(snapshot_version: u64) {
    if INDEX_CHANGE_VERSION.load(Ordering::Acquire) == snapshot_version {
        INDEX_CLEARED_VERSION.store(snapshot_version, Ordering::Release);
    }
}

pub(crate) fn clear_index_dirty() {
    let current = INDEX_CHANGE_VERSION.load(Ordering::Acquire);
    INDEX_CLEARED_VERSION.store(current, Ordering::Release);
}

#[cfg(test)]
pub(crate) fn stop_index_watchers_for_test() {
    stop_index_watchers();
}

pub(crate) fn restart_index_watchers() {
    stop_index_watchers();

    let config = RuntimeConfig::load();
    if !config.lazy_indexing_enabled {
        log_info("index watcher: disabled by lazy_indexing_enabled=false");
        return;
    }

    let mut roots = config.app_scan_roots;
    roots.extend(config.file_scan_roots);
    roots.sort();
    roots.dedup();

    let active_roots: Vec<String> = roots
        .into_iter()
        .filter(|root| !root.trim().is_empty() && Path::new(root).exists())
        .collect();

    if active_roots.is_empty() {
        log_info("index watcher: no valid roots to monitor");
        return;
    }

    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let control = INDEX_WATCHER_CONTROL.get_or_init(|| Mutex::new(None));
    {
        let mut guard = control
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *guard = Some(stop_tx);
    }

    thread::spawn(move || {
        let (event_tx, event_rx) = mpsc::channel::<notify::Result<Event>>();
        let mut watcher = match RecommendedWatcher::new(
            move |result| {
                let _ = event_tx.send(result);
            },
            notify::Config::default(),
        ) {
            Ok(watcher) => watcher,
            Err(err) => {
                log_error(&format!("index watcher init failed error={err}"));
                return;
            }
        };

        let mut watched_count = 0usize;
        for root in &active_roots {
            if watcher
                .watch(Path::new(root), RecursiveMode::Recursive)
                .is_ok()
            {
                watched_count += 1;
            } else {
                log_error(&format!("index watcher failed root={root}"));
            }
        }

        if watched_count == 0 {
            log_error("index watcher failed: no roots watched");
            mark_index_dirty();
            return;
        }

        log_info(&format!("index watcher active roots={watched_count}"));

        loop {
            if stop_rx.try_recv().is_ok() {
                break;
            }

            match event_rx.recv_timeout(Duration::from_secs(1)) {
                Ok(Ok(event)) => {
                    if should_mark_dirty_from_event(&event) {
                        mark_index_dirty();
                    }
                }
                Ok(Err(err)) => {
                    log_error(&format!("index watcher event error={err}"));
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    log_error("index watcher channel disconnected");
                    break;
                }
            }
        }
    });
}

fn request_background_index_refresh_internal() -> bool {
    let _ = engine_cache();

    let lazy_indexing_enabled = RuntimeConfig::load().lazy_indexing_enabled;
    // Lazy indexing ON: refresh only when watcher marked index as dirty.
    // Lazy indexing OFF: refresh on every Cmd+Space request.
    if !refresh_allowed_by_dirty_mode(lazy_indexing_enabled, is_index_dirty()) {
        return false;
    }
    if !try_acquire_index_refresh_slot() {
        return false;
    }

    let dirty_version_snapshot = INDEX_CHANGE_VERSION.load(Ordering::Acquire);
    thread::spawn(move || {
        // Keep refresh slot release tied to scope exit so the in-progress flag
        // is reset for success, error, and panic-unwind paths.
        let _refresh_slot = IndexRefreshGuard;
        let started_at = Instant::now();
        let run_result = panic::catch_unwind(AssertUnwindSafe(|| {
            let path = default_db_path();
            match QueryEngine::bootstrap_sqlite(&path) {
                Ok(()) => {
                    refresh_engine_cache();
                    clear_index_dirty_if_unchanged(dirty_version_snapshot);
                    let candidate_count = with_engine(|engine| engine.search("", 2000).len());
                    log_info(&format!(
                        "background index refresh ok candidates={} elapsed_ms={}",
                        candidate_count,
                        started_at.elapsed().as_millis()
                    ));
                }
                Err(err) => {
                    mark_index_dirty();
                    log_error(&format!(
                        "background index refresh failed error={} elapsed_ms={}",
                        err,
                        started_at.elapsed().as_millis()
                    ));
                }
            }
        }));

        if run_result.is_err() {
            mark_index_dirty();
            log_error(&format!(
                "background index refresh panicked elapsed_ms={}",
                started_at.elapsed().as_millis()
            ));
        }
    });

    true
}

fn refresh_allowed_by_dirty_mode(lazy_indexing_enabled: bool, index_dirty: bool) -> bool {
    if lazy_indexing_enabled {
        return index_dirty;
    }
    true
}

fn is_index_dirty() -> bool {
    INDEX_CHANGE_VERSION.load(Ordering::Acquire) != INDEX_CLEARED_VERSION.load(Ordering::Acquire)
}

fn try_acquire_index_refresh_slot() -> bool {
    INDEX_REFRESH_IN_PROGRESS
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
}

// Ensures INDEX_REFRESH_IN_PROGRESS is always cleared when the refresh worker
// exits, preventing a stuck "refresh in progress" state.
struct IndexRefreshGuard;

impl Drop for IndexRefreshGuard {
    fn drop(&mut self) {
        INDEX_REFRESH_IN_PROGRESS.store(false, Ordering::Release);
    }
}

pub(crate) fn store_json_allocation(cstring: CString) -> *mut c_char {
    let ptr = cstring.as_ptr() as usize;

    let lock = JSON_ALLOCS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut allocations = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    allocations.insert(ptr, cstring);

    ptr as *mut c_char
}

pub(crate) fn free_json_allocation(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }

    if let Some(lock) = JSON_ALLOCS.get() {
        let mut allocations = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        allocations.remove(&(ptr as usize));
    }
}

pub(crate) fn cstr_to_string(ptr: *const c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }

    unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned()
}

fn engine_cache() -> &'static RwLock<QueryEngine> {
    let cache = ENGINE_CACHE.get_or_init(|| {
        let path = default_db_path();
        let engine = QueryEngine::from_sqlite(&path).unwrap_or_else(|_| QueryEngine::demo_seed());
        RwLock::new(engine)
    });
    start_background_bootstrap_refresh();
    start_index_watcher_bootstrap();
    cache
}

fn start_background_bootstrap_refresh() {
    let _ = BOOTSTRAP_REFRESH_STARTED.get_or_init(|| {
        thread::spawn(|| {
            let started_at = Instant::now();
            let dirty_version_snapshot = INDEX_CHANGE_VERSION.load(Ordering::Acquire);
            let path = default_db_path();
            match QueryEngine::bootstrap_sqlite(&path) {
                Ok(()) => {
                    refresh_engine_cache();
                    clear_index_dirty_if_unchanged(dirty_version_snapshot);
                    let candidate_count = with_engine(|engine| engine.search("", 2000).len());
                    log_info(&format!(
                        "bootstrap refresh ok candidates={} elapsed_ms={}",
                        candidate_count,
                        started_at.elapsed().as_millis()
                    ));
                }
                Err(err) => {
                    mark_index_dirty();
                    log_error(&format!(
                        "bootstrap refresh failed error={} elapsed_ms={}",
                        err,
                        started_at.elapsed().as_millis()
                    ));
                }
            }
        });
    });
}

fn start_index_watcher_bootstrap() {
    let _ = INDEX_WATCHER_BOOTSTRAP_STARTED.get_or_init(restart_index_watchers);
}

fn stop_index_watchers() {
    if let Some(control) = INDEX_WATCHER_CONTROL.get() {
        let mut guard = control
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(tx) = guard.take() {
            let _ = tx.send(());
        }
    }
}

fn should_mark_dirty_from_event(event: &Event) -> bool {
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

#[cfg(test)]
mod tests {
    use super::should_mark_dirty_from_event;
    use super::{
        INDEX_CHANGE_VERSION, INDEX_REFRESH_IN_PROGRESS, IndexRefreshGuard, clear_index_dirty,
        clear_index_dirty_if_unchanged, is_index_dirty, legacy_default_db_path, mark_index_dirty,
        refresh_allowed_by_dirty_mode, try_acquire_index_refresh_slot,
    };
    use notify::event::{CreateKind, DataChange, ModifyKind, RemoveKind, RenameMode};
    use notify::{Event, EventKind};
    use std::path::PathBuf;
    use std::sync::{Mutex, OnceLock};

    fn test_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .expect("test lock should not be poisoned")
    }

    fn event(kind: EventKind, path: &str) -> Event {
        Event {
            kind,
            paths: vec![PathBuf::from(path)],
            attrs: Default::default(),
        }
    }

    #[test]
    fn dirty_marking_accepts_create_remove_and_rename() {
        let _guard = test_lock();
        let created = event(EventKind::Create(CreateKind::File), "/tmp/foo.txt");
        assert!(should_mark_dirty_from_event(&created));

        let removed = event(EventKind::Remove(RemoveKind::File), "/tmp/foo.txt");
        assert!(should_mark_dirty_from_event(&removed));

        let renamed = event(
            EventKind::Modify(ModifyKind::Name(RenameMode::To)),
            "/tmp/foo-renamed.txt",
        );
        assert!(should_mark_dirty_from_event(&renamed));
    }

    #[test]
    fn dirty_marking_ignores_non_rename_modify_and_empty_paths() {
        let _guard = test_lock();
        let content_write = event(
            EventKind::Modify(ModifyKind::Data(DataChange::Content)),
            "/tmp/foo.txt",
        );
        assert!(!should_mark_dirty_from_event(&content_write));

        let empty_paths = Event {
            kind: EventKind::Create(CreateKind::File),
            paths: vec![],
            attrs: Default::default(),
        };
        assert!(!should_mark_dirty_from_event(&empty_paths));
    }

    #[test]
    fn clear_index_dirty_if_unchanged_clears_when_version_matches() {
        let _guard = test_lock();

        clear_index_dirty();
        mark_index_dirty();
        let snapshot = INDEX_CHANGE_VERSION.load(std::sync::atomic::Ordering::Acquire);

        clear_index_dirty_if_unchanged(snapshot);
        assert!(!is_index_dirty());
    }

    #[test]
    fn clear_index_dirty_if_unchanged_keeps_dirty_when_version_moves() {
        let _guard = test_lock();

        clear_index_dirty();
        mark_index_dirty();
        let snapshot = INDEX_CHANGE_VERSION.load(std::sync::atomic::Ordering::Acquire);
        mark_index_dirty();

        clear_index_dirty_if_unchanged(snapshot);
        assert!(is_index_dirty());
    }

    #[test]
    fn refresh_mode_logic_matches_lazy_toggle_expectations() {
        let _guard = test_lock();

        assert!(!refresh_allowed_by_dirty_mode(true, false));
        assert!(refresh_allowed_by_dirty_mode(true, true));
        assert!(refresh_allowed_by_dirty_mode(false, false));
        assert!(refresh_allowed_by_dirty_mode(false, true));
    }

    #[test]
    fn refresh_slot_acquire_and_guard_release_are_consistent() {
        let _guard = test_lock();

        INDEX_REFRESH_IN_PROGRESS.store(false, std::sync::atomic::Ordering::Release);
        assert!(try_acquire_index_refresh_slot());
        assert!(!try_acquire_index_refresh_slot());

        {
            let _slot_guard = IndexRefreshGuard;
        }

        assert!(try_acquire_index_refresh_slot());

        {
            let _slot_guard = IndexRefreshGuard;
        }

        INDEX_REFRESH_IN_PROGRESS.store(false, std::sync::atomic::Ordering::Release);
    }

    #[test]
    fn legacy_path_points_to_macos_location_shape() {
        let path = legacy_default_db_path();
        let path_str = path.to_string_lossy();
        assert!(path_str.contains("Library"));
        assert!(path_str.contains("Application Support"));
        assert!(path_str.ends_with("look.db"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_path_uses_localappdata_shape() {
        let path = super::windows_default_db_path();
        if let Some(path) = path {
            let path_str = path.to_string_lossy().to_ascii_lowercase();
            assert!(path_str.contains("look"));
            assert!(path_str.ends_with("look.db"));
        }
    }
}
