use look_engine::BootstrapScope;
use look_engine::QueryEngine;
use look_engine::config::RuntimeConfig;
use notify::event::ModifyKind;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::env;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock, RwLock, mpsc};
use std::thread;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tauri::{Emitter, Manager};

const WATCHER_DEBOUNCE_SECS: u64 = 2;
const WATCHER_POLL_MS: u64 = 500;
/// Minimum gap, in milliseconds, between two watcher-triggered refreshes.
/// Bounds CPU/IO when something (sync client, active downloader, package
/// manager) keeps re-dirtying the index. Explicit user-driven refreshes
/// (window show, force_index_refresh) deliberately bypass this gate.
const WATCHER_REFRESH_COOLDOWN_MS: u64 = 10_000;

/// Exact filenames that the watcher should ignore — OS metadata droppings
/// that file managers create as a side effect of normal directory access.
const NOISY_NAMES: &[&str] = &[".DS_Store", "Thumbs.db", "desktop.ini", ".directory"];

/// Filename prefixes that signal a transient atomic-save artifact: Office
/// lockfile (`~$`), Emacs autosave (`.#`), and the vim `.~` family.
const NOISY_PREFIXES: &[&str] = &["~$", ".~", ".#"];

/// Lowercased filename suffixes that signal transient files: editor swaps,
/// browser partial downloads, generic tmp/lock/bak artifacts.
const NOISY_SUFFIXES: &[&str] = &[
    ".swp",
    ".swo",
    ".swn",
    ".swx",
    ".tmp",
    ".temp",
    ".crdownload",
    ".part",
    ".partial",
    ".download",
    ".lock",
    ".lck",
    ".bak",
    ".cache",
];

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(target_os = "linux")]
fn extra_apps_roots() -> Vec<String> {
    let mut roots = Vec::new();
    if let Ok(home) = std::env::var("HOME") {
        let home = home.trim();
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
    roots
}

#[cfg(not(target_os = "linux"))]
fn extra_apps_roots() -> Vec<String> {
    Vec::new()
}

/// Resets the `in_progress` flag on drop, so a panic inside the refresh worker
/// can't leak it as permanently `true` (which would silently disable all
/// further auto-refreshes for the rest of the session).
struct RefreshSlotGuard<'a> {
    flag: &'a AtomicBool,
}

impl Drop for RefreshSlotGuard<'_> {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::Release);
    }
}

static APP_HANDLE: OnceLock<tauri::AppHandle> = OnceLock::new();

/// Bundle of `AppState` field addresses shared with the watcher loop and each
/// reindex worker it spawns. Stored as `usize` because raw pointers are not
/// `Send`, and Rust 2021's disjoint closure capture would otherwise see each
/// captured field as the underlying `*const T` regardless of any wrapper impl.
/// `AppState` is owned by Tauri and outlives every thread the watcher spawns,
/// so the casts back to references in `unsafe` blocks are valid for the
/// program's lifetime.
#[derive(Clone, Copy)]
struct WatcherStatePtrs {
    change_version: usize,
    cleared_version: usize,
    in_progress: usize,
    engine_lock: usize,
    last_refresh_at: usize,
}

pub struct AppState {
    engine: RwLock<QueryEngine>,
    index_change_version: AtomicU64,
    index_cleared_version: AtomicU64,
    index_refresh_in_progress: AtomicBool,
    /// UNIX millis at which the most recent refresh (of any source) completed.
    /// Read by the watcher loop to enforce `WATCHER_REFRESH_COOLDOWN_MS`.
    /// `0` means "no refresh has completed yet" — first run is always allowed.
    last_refresh_completed_unix_ms: AtomicU64,
    watcher_control: Mutex<Option<mpsc::Sender<()>>>,
}

impl AppState {
    pub fn new() -> Self {
        let path = default_db_path();
        let engine = QueryEngine::from_sqlite(&path).unwrap_or_else(|_| QueryEngine::new(vec![]));

        let state = Self {
            engine: RwLock::new(engine),
            index_change_version: AtomicU64::new(0),
            index_cleared_version: AtomicU64::new(0),
            index_refresh_in_progress: AtomicBool::new(false),
            last_refresh_completed_unix_ms: AtomicU64::new(0),
            watcher_control: Mutex::new(None),
        };

        state.start_index_watchers();
        state
    }

    pub fn init_app_handle(app: &tauri::App) {
        let _ = APP_HANDLE.set(app.handle().clone());
    }

    /// Must be called after `init_app_handle` so the `index-ready` event
    /// can be emitted once the bootstrap finishes.
    pub fn start_bootstrap(&self) {
        self.start_background_bootstrap();
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
        if !self.try_acquire_refresh_slot() {
            return false;
        }
        let dirty_snapshot = self.index_change_version.load(Ordering::Acquire);
        spawn_refresh_worker(
            self.ptrs(),
            BootstrapScope::ALL,
            dirty_snapshot,
            true,  // we just acquired in_progress; worker releases it on drop
            false, // user-initiated refresh; frontend already knows it asked
            "look: index refresh",
        );
        true
    }

    fn start_background_bootstrap(&self) {
        // Initial bootstrap doesn't gate on `in_progress` (it's the first
        // refresh of the session, nothing else has tried to acquire yet).
        let dirty_snapshot = self.index_change_version.load(Ordering::Acquire);
        spawn_refresh_worker(
            self.ptrs(),
            BootstrapScope::ALL,
            dirty_snapshot,
            false, // no slot held — don't release on drop
            true,  // frontend needs the EVENT_INDEX_READY signal to render
            "look: bootstrap",
        );
    }

    fn ptrs(&self) -> WatcherStatePtrs {
        WatcherStatePtrs {
            change_version: &self.index_change_version as *const AtomicU64 as usize,
            cleared_version: &self.index_cleared_version as *const AtomicU64 as usize,
            in_progress: &self.index_refresh_in_progress as *const AtomicBool as usize,
            engine_lock: &self.engine as *const RwLock<QueryEngine> as usize,
            last_refresh_at: &self.last_refresh_completed_unix_ms as *const AtomicU64 as usize,
        }
    }

    fn start_index_watchers(&self) {
        let config = RuntimeConfig::load_cached();
        if !config.lazy_indexing_enabled {
            return;
        }

        // Apps roots: small directories holding .desktop / .app entries. Safe to
        // watch recursively — inode budget is tiny.
        let mut apps_roots: Vec<String> = config.app_scan_roots.clone();
        apps_roots.extend(extra_apps_roots());

        // File roots: Documents / Downloads / Desktop and any extras. These can
        // be huge; recursive watches would chew through `fs.inotify.max_user_watches`
        // and silently drop deep subdirs. Watched non-recursively — top-level
        // adds/removes (the common "I just downloaded a thing" case) still
        // refresh promptly, and the on-show full refresh reconciles deeper
        // changes.
        let mut file_roots: Vec<String> = config.file_scan_roots.clone();
        file_roots.extend(config.file_scan_extra_roots);

        let normalize_roots = |mut v: Vec<String>| -> Vec<String> {
            v.sort();
            v.dedup();
            v.into_iter()
                .filter(|root| !root.trim().is_empty() && Path::new(root).exists())
                .collect()
        };
        let apps_roots = normalize_roots(apps_roots);
        let file_roots = normalize_roots(file_roots);

        if apps_roots.is_empty() && file_roots.is_empty() {
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

        let ptrs = WatcherStatePtrs {
            change_version: &self.index_change_version as *const AtomicU64 as usize,
            cleared_version: &self.index_cleared_version as *const AtomicU64 as usize,
            in_progress: &self.index_refresh_in_progress as *const AtomicBool as usize,
            engine_lock: &self.engine as *const RwLock<QueryEngine> as usize,
            last_refresh_at: &self.last_refresh_completed_unix_ms as *const AtomicU64 as usize,
        };

        // SAFETY: AppState lives for the app's lifetime, so the addresses
        // remain valid for the watcher thread and any reindex worker it spawns.
        thread::spawn(move || {
            let (change_version, in_progress, last_refresh_at) = unsafe {
                (
                    &*(ptrs.change_version as *const AtomicU64),
                    &*(ptrs.in_progress as *const AtomicBool),
                    &*(ptrs.last_refresh_at as *const AtomicU64),
                )
            };

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

            for root in &apps_roots {
                match watcher.watch(Path::new(root), RecursiveMode::Recursive) {
                    Ok(()) => eprintln!("[watcher] watching apps (recursive): {root}"),
                    Err(e) => eprintln!("[watcher] failed to watch apps {root}: {e}"),
                }
            }
            for root in &file_roots {
                match watcher.watch(Path::new(root), RecursiveMode::NonRecursive) {
                    Ok(()) => eprintln!("[watcher] watching files (non-recursive): {root}"),
                    Err(e) => eprintln!("[watcher] failed to watch files {root}: {e}"),
                }
            }

            let apps_roots_paths: Vec<PathBuf> = apps_roots.iter().map(PathBuf::from).collect();
            let file_roots_paths: Vec<PathBuf> = file_roots.iter().map(PathBuf::from).collect();

            let debounce = std::time::Duration::from_secs(WATCHER_DEBOUNCE_SECS);
            let mut last_dirty_at: Option<Instant> = None;
            let mut apps_dirty = false;
            let mut files_dirty = false;

            loop {
                if stop_rx.try_recv().is_ok() {
                    break;
                }

                match event_rx.recv_timeout(std::time::Duration::from_millis(WATCHER_POLL_MS)) {
                    Ok(Ok(event)) => {
                        // Only relevant events update dirty state, but we always
                        // fall through to the debounce check below — otherwise a
                        // steady stream of ignored events (e.g. `Modify(Data)`
                        // during a long save/download) would starve the debounce
                        // timer and leave the index stale until the stream stops.
                        if should_mark_dirty(&event) {
                            let mut matched = false;
                            for path in &event.paths {
                                if path_under_any(path, &apps_roots_paths) {
                                    apps_dirty = true;
                                    matched = true;
                                }
                                if path_under_any(path, &file_roots_paths) {
                                    files_dirty = true;
                                    matched = true;
                                }
                            }
                            if matched {
                                let v = change_version.fetch_add(1, Ordering::AcqRel);
                                eprintln!(
                                    "[watcher] dirty! v={} apps={} files={} {:?} {:?}",
                                    v + 1,
                                    apps_dirty,
                                    files_dirty,
                                    event.kind,
                                    event.paths
                                );
                                last_dirty_at = Some(Instant::now());
                            }
                        }
                    }
                    Ok(Err(_)) => {}
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }

                // Debounce expired and something is dirty: spawn a worker so the
                // watcher loop keeps draining the event channel while the
                // (potentially slow) reindex runs. The cooldown gate bounds
                // refresh frequency when a noisy producer (sync client, package
                // manager, active downloader) keeps re-dirtying the index — we
                // still defer the refresh but don't fire it until enough time
                // has passed since the last completion.
                let cooldown_ok = {
                    let last = last_refresh_at.load(Ordering::Acquire);
                    last == 0 || now_unix_ms().saturating_sub(last) >= WATCHER_REFRESH_COOLDOWN_MS
                };
                if let Some(t) = last_dirty_at
                    && t.elapsed() >= debounce
                    && cooldown_ok
                    && (apps_dirty || files_dirty)
                    && in_progress
                        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                        .is_ok()
                {
                    let scope = BootstrapScope {
                        apps: apps_dirty,
                        files: files_dirty,
                        settings: false,
                    };
                    last_dirty_at = None;
                    apps_dirty = false;
                    files_dirty = false;
                    let dirty_snapshot = change_version.load(Ordering::Acquire);

                    eprintln!("[watcher] auto-refresh start scope={scope:?}");
                    spawn_refresh_worker(
                        ptrs,
                        scope,
                        dirty_snapshot,
                        true, // outer loop already CAS-acquired in_progress
                        true, // watcher refreshes are async; frontend needs the signal
                        "[watcher] auto-refresh",
                    );
                }
            }
        });
    }

    pub fn force_index_refresh(&self) -> bool {
        // Mark dirty so lazy indexing check passes
        self.index_change_version.fetch_add(1, Ordering::AcqRel);
        self.request_index_refresh()
    }

    fn try_acquire_refresh_slot(&self) -> bool {
        self.index_refresh_in_progress
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }
}

/// Spawns the background thread that runs a single index refresh and folds the
/// result back into `AppState`. Used by all three callers: the initial
/// bootstrap, the user-triggered window-show refresh, and the watcher's
/// debounced auto-refresh. Each caller passes its own knobs:
///
/// - `scope`: `ALL` for full bootstrap, `APPS_ONLY`/`FILES_ONLY` for scoped.
/// - `dirty_snapshot`: captured by the caller *before* spawning so that
///   events arriving during the refresh keep the cleared_version stale.
/// - `holds_slot`: `true` if the caller already CAS-acquired `in_progress` and
///   wants this worker to release it via `RefreshSlotGuard` on drop. The
///   initial bootstrap passes `false` (no contention to gate against yet).
/// - `emit_ready`: `true` for async refreshes that the frontend can't otherwise
///   know about (bootstrap, watcher auto-refresh). `false` for user-initiated
///   refreshes — the frontend already knows it issued the command.
/// - `log_label`: prefix for stdout log lines (kept distinct so grep-by-source
///   still works: `look: …` vs `[watcher] …`).
fn spawn_refresh_worker(
    ptrs: WatcherStatePtrs,
    scope: BootstrapScope,
    dirty_snapshot: u64,
    holds_slot: bool,
    emit_ready: bool,
    log_label: &'static str,
) {
    let db_path = default_db_path();
    // SAFETY: `AppState` is owned by Tauri and outlives every worker thread,
    // so the addresses smuggled through `WatcherStatePtrs` stay valid.
    thread::spawn(move || {
        let (engine_lock, change_version, cleared_version, in_progress, last_refresh_at) = unsafe {
            (
                &*(ptrs.engine_lock as *const RwLock<QueryEngine>),
                &*(ptrs.change_version as *const AtomicU64),
                &*(ptrs.cleared_version as *const AtomicU64),
                &*(ptrs.in_progress as *const AtomicBool),
                &*(ptrs.last_refresh_at as *const AtomicU64),
            )
        };
        // RAII slot release. A panic inside the engine/storage layer would
        // otherwise leave `in_progress` stuck `true` and silently disable all
        // further auto-refreshes for the rest of the session.
        let _slot = holds_slot.then(|| RefreshSlotGuard { flag: in_progress });

        let started_at = Instant::now();
        let result = if scope.is_all() {
            QueryEngine::bootstrap_sqlite(&db_path)
        } else {
            QueryEngine::bootstrap_sqlite_scoped(&db_path, scope)
        };
        match result {
            Ok(()) => {
                if let Ok(new_engine) = QueryEngine::from_sqlite(&db_path) {
                    let mut guard = engine_lock
                        .write()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    *guard = new_engine;
                }
                // Only clear the dirty version if no new events landed during
                // the refresh; otherwise the next loop tick picks them up.
                if change_version.load(Ordering::Acquire) == dirty_snapshot {
                    cleared_version.store(dirty_snapshot, Ordering::Release);
                }
                if scope.is_all() {
                    eprintln!(
                        "{log_label} ok elapsed_ms={}",
                        started_at.elapsed().as_millis()
                    );
                } else {
                    eprintln!(
                        "{log_label} ok scope={scope:?} elapsed_ms={}",
                        started_at.elapsed().as_millis()
                    );
                }
                if emit_ready
                    && let Some(handle) = APP_HANDLE.get()
                    && let Some(w) = handle.get_webview_window(crate::consts::MAIN_WINDOW)
                {
                    let _ = w.emit(crate::consts::EVENT_INDEX_READY, ());
                }
            }
            Err(err) => {
                change_version.fetch_add(1, Ordering::AcqRel);
                eprintln!("{log_label} failed: {err}");
            }
        }
        last_refresh_at.store(now_unix_ms(), Ordering::Release);
    });
}

pub const ENV_DB_PATH: &str = "LOOK_DB_PATH";
const APP_DIR: &str = "look";
const DB_FILE: &str = "look.db";

pub fn default_db_path() -> PathBuf {
    if let Ok(custom) = env::var(ENV_DB_PATH) {
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
                return PathBuf::from(trimmed).join(APP_DIR).join(DB_FILE);
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(data_home) = env::var("XDG_DATA_HOME") {
            let trimmed = data_home.trim();
            if !trimmed.is_empty() {
                return PathBuf::from(trimmed).join(APP_DIR).join(DB_FILE);
            }
        }
        if let Ok(home) = env::var("HOME") {
            return PathBuf::from(home)
                .join(".local")
                .join("share")
                .join(APP_DIR)
                .join(DB_FILE);
        }
    }

    // Fallback
    let home = env::var("HOME")
        .or_else(|_| env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".look").join(DB_FILE)
}

fn should_mark_dirty(event: &Event) -> bool {
    if event.paths.is_empty() {
        return false;
    }

    let kind_relevant = matches!(
        event.kind,
        EventKind::Create(_)
            | EventKind::Remove(_)
            | EventKind::Any
            | EventKind::Modify(ModifyKind::Name(_))
    );
    if !kind_relevant {
        return false;
    }

    // Suppress events that only touch noisy synthetic files (vim swaps, browser
    // partial downloads, OS metadata droppings, office lockfiles). These can
    // fire dozens of times per second during normal use and force a needless
    // full reindex without changing anything a user would search for.
    if event.paths.iter().all(|p| is_noisy_path(p)) {
        return false;
    }

    true
}

fn is_noisy_path(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
        return false;
    };
    if NOISY_NAMES.contains(&name) {
        return true;
    }
    if NOISY_PREFIXES.iter().any(|p| name.starts_with(p)) {
        return true;
    }
    let lower = name.to_ascii_lowercase();
    NOISY_SUFFIXES.iter().any(|ext| lower.ends_with(ext))
}

fn path_under_any(path: &Path, roots: &[PathBuf]) -> bool {
    roots
        .iter()
        .any(|root| path == root.as_path() || path.starts_with(root))
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{CreateKind, EventAttributes, ModifyKind, RemoveKind, RenameMode};

    fn ev(kind: EventKind, paths: &[&str]) -> Event {
        Event {
            kind,
            paths: paths.iter().map(PathBuf::from).collect(),
            attrs: EventAttributes::default(),
        }
    }

    #[test]
    fn should_mark_dirty_accepts_create_and_remove_and_rename() {
        assert!(should_mark_dirty(&ev(
            EventKind::Create(CreateKind::File),
            &["/home/u/Documents/report.pdf"],
        )));
        assert!(should_mark_dirty(&ev(
            EventKind::Remove(RemoveKind::File),
            &["/home/u/Downloads/old.zip"],
        )));
        assert!(should_mark_dirty(&ev(
            EventKind::Modify(ModifyKind::Name(RenameMode::To)),
            &["/usr/share/applications/firefox.desktop"],
        )));
    }

    #[test]
    fn should_mark_dirty_rejects_data_and_metadata_modifies() {
        // Pure content edits (e.g. saving a text file in place) must not
        // trigger a reindex — only structural changes do.
        assert!(!should_mark_dirty(&ev(
            EventKind::Modify(ModifyKind::Data(notify::event::DataChange::Content)),
            &["/home/u/Documents/report.pdf"],
        )));
        assert!(!should_mark_dirty(&ev(
            EventKind::Modify(ModifyKind::Metadata(
                notify::event::MetadataKind::Permissions
            )),
            &["/home/u/Documents/report.pdf"],
        )));
    }

    #[test]
    fn should_mark_dirty_suppresses_events_whose_paths_are_all_noise() {
        // Vim's swap file create — must not wake the indexer.
        assert!(!should_mark_dirty(&ev(
            EventKind::Create(CreateKind::File),
            &["/home/u/Documents/.notes.txt.swp"],
        )));
        // Browser partial download — only noisy paths in the event.
        assert!(!should_mark_dirty(&ev(
            EventKind::Create(CreateKind::File),
            &["/home/u/Downloads/big.iso.crdownload"],
        )));
    }

    #[test]
    fn should_mark_dirty_passes_through_mixed_noise_and_real_paths() {
        // A rename pair: from a swap file to the real file (atomic save). We
        // want the indexer to run because the real file changed.
        assert!(should_mark_dirty(&ev(
            EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
            &[
                "/home/u/Documents/.notes.txt.swp",
                "/home/u/Documents/notes.txt",
            ],
        )));
    }

    #[test]
    fn should_mark_dirty_requires_at_least_one_path() {
        assert!(!should_mark_dirty(&ev(
            EventKind::Create(CreateKind::File),
            &[],
        )));
    }

    #[test]
    fn is_noisy_path_recognizes_editor_swaps_and_office_locks() {
        for name in [
            ".notes.txt.swp",
            ".notes.txt.swo",
            ".notes.txt.swx",
            "report.tmp",
            "data.temp",
            "movie.mkv.part",
            "iso.crdownload",
            ".#emacs-lockfile",
            "~$report.docx",
            ".DS_Store",
            "Thumbs.db",
            "desktop.ini",
            ".directory",
        ] {
            let p = PathBuf::from(format!("/home/u/Documents/{name}"));
            assert!(is_noisy_path(&p), "expected noisy: {name}");
        }
    }

    #[test]
    fn is_noisy_path_does_not_match_real_documents() {
        for name in [
            "report.pdf",
            "thesis.docx",
            "photo.jpg",
            "archive.tar.gz",
            "script.sh",
        ] {
            let p = PathBuf::from(format!("/home/u/Documents/{name}"));
            assert!(!is_noisy_path(&p), "must not flag user file: {name}");
        }
    }

    #[test]
    fn is_noisy_path_is_case_insensitive_on_suffixes() {
        // Some apps use upper-case suffixes; we still want them filtered.
        let p = PathBuf::from("/home/u/Documents/notes.SWP");
        assert!(is_noisy_path(&p));
    }

    #[test]
    fn path_under_any_matches_root_and_descendants() {
        let roots = vec![
            PathBuf::from("/home/u/Documents"),
            PathBuf::from("/usr/share/applications"),
        ];
        assert!(path_under_any(
            Path::new("/home/u/Documents/report.pdf"),
            &roots
        ));
        assert!(path_under_any(
            Path::new("/usr/share/applications/firefox.desktop"),
            &roots
        ));
        // The root itself counts as "under" (e.g. an event on the watched dir).
        assert!(path_under_any(Path::new("/home/u/Documents"), &roots));
    }

    #[test]
    fn path_under_any_is_boundary_aware() {
        let roots = vec![PathBuf::from("/home/u/Down")];
        // `/home/u/Downloads` must NOT count as being under `/home/u/Down` —
        // PathBuf::starts_with compares whole components, so this is a property
        // of the helper we explicitly rely on.
        assert!(!path_under_any(
            Path::new("/home/u/Downloads/foo.zip"),
            &roots,
        ));
    }

    #[test]
    fn path_under_any_returns_false_when_no_root_matches() {
        let roots = vec![PathBuf::from("/home/u/Documents")];
        assert!(!path_under_any(Path::new("/tmp/scratch.txt"), &roots));
    }

    #[test]
    fn refresh_slot_guard_releases_flag_on_drop() {
        let flag = AtomicBool::new(true);
        {
            let _guard = RefreshSlotGuard { flag: &flag };
            assert!(flag.load(Ordering::Acquire));
        }
        // After the guard goes out of scope (or after a panic unwinds through
        // it), the slot must be released so the watcher can fire again.
        assert!(!flag.load(Ordering::Acquire));
    }
}
