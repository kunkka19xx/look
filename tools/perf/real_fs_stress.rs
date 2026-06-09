//! Real-filesystem end-to-end stress test for the watcher path.
//!
//! Unlike `watcher_stress` (which simulates the decision logic against
//! synthesized event timestamps), this binary:
//!   • spins up a real `notify::RecommendedWatcher` against a tempdir,
//!   • runs the same loop policy as `apps/linows/src-tauri/src/state.rs`
//!     (debounce, cooldown, noise filter, scoped refresh, off-thread reindex,
//!     RAII slot guard),
//!   • spawns a producer worker that performs real file create/rename/remove
//!     operations,
//!   • points the engine at a throwaway DB via `LOOK_DB_PATH` and a custom
//!     `LOOK_CONFIG_PATH` so the bench never touches your live index,
//!   • reports counters at the end.
//!
//! Run with:
//!   cargo run --release --bin real_fs_stress --manifest-path tools/perf/Cargo.toml
use look_engine::{BootstrapScope, QueryEngine};
use notify::event::ModifyKind;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::env;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const RUN_SECS: u64 = 30;
const DEBOUNCE_MS: u128 = 2_000;
const COOLDOWN_MS: u64 = 10_000;

fn main() {
    let tempdir = env::temp_dir().join(format!("look-real-fs-stress-{}", std::process::id()));
    let apps_root = tempdir.join("apps");
    let files_root = tempdir.join("files");
    let db_path = tempdir.join("look.db");
    let config_path = tempdir.join("look.config");

    fs::create_dir_all(&apps_root).expect("mkdir apps");
    fs::create_dir_all(&files_root).expect("mkdir files");

    // Point the engine at our tempdir for both config and DB. The config file
    // tells `RuntimeConfig::load()` to use only our tempdir roots, so the
    // bench's bootstrap doesn't crawl the user's real `~/Documents`.
    write_config(&config_path, &apps_root, &files_root);
    unsafe {
        env::set_var("LOOK_CONFIG_PATH", &config_path);
        env::set_var("LOOK_DB_PATH", &db_path);
    }

    // Seed the database with one full bootstrap so subsequent scoped refreshes
    // hit the warm path (matching watcher behavior after app start).
    QueryEngine::bootstrap_sqlite(&db_path).expect("warm bootstrap");

    // Counters observable from main.
    let events_received = Arc::new(AtomicU64::new(0));
    let events_filtered = Arc::new(AtomicU64::new(0));
    let dirty_marks = Arc::new(AtomicU64::new(0));
    let refreshes_fired = Arc::new(AtomicU64::new(0));
    let cooldown_skips = Arc::new(AtomicU64::new(0));
    let in_progress = Arc::new(AtomicBool::new(false));
    let last_refresh_ms = Arc::new(AtomicU64::new(0));
    let stop = Arc::new(AtomicBool::new(false));

    println!(
        "real_fs_stress: tempdir={} pid={} run={}s",
        tempdir.display(),
        std::process::id(),
        RUN_SECS,
    );

    // ─── Watcher thread ───────────────────────────────────────────────────
    let watcher_handle = {
        let apps_root = apps_root.clone();
        let files_root = files_root.clone();
        let db_path = db_path.clone();
        let events_received = events_received.clone();
        let events_filtered = events_filtered.clone();
        let dirty_marks = dirty_marks.clone();
        let refreshes_fired = refreshes_fired.clone();
        let cooldown_skips = cooldown_skips.clone();
        let in_progress = in_progress.clone();
        let last_refresh_ms = last_refresh_ms.clone();
        let stop = stop.clone();

        thread::spawn(move || {
            let (tx, rx) = mpsc::channel::<notify::Result<Event>>();
            let mut watcher = RecommendedWatcher::new(
                move |res| {
                    let _ = tx.send(res);
                },
                notify::Config::default(),
            )
            .expect("create watcher");

            watcher
                .watch(&apps_root, RecursiveMode::Recursive)
                .expect("watch apps");
            watcher
                .watch(&files_root, RecursiveMode::NonRecursive)
                .expect("watch files");

            let mut apps_dirty = false;
            let mut files_dirty = false;
            let mut last_dirty_at: Option<Instant> = None;

            loop {
                if stop.load(Ordering::Acquire) {
                    break;
                }
                match rx.recv_timeout(Duration::from_millis(500)) {
                    Ok(Ok(event)) => {
                        events_received.fetch_add(1, Ordering::AcqRel);
                        if !should_mark_dirty(&event) {
                            events_filtered.fetch_add(1, Ordering::AcqRel);
                            continue;
                        }
                        let mut matched = false;
                        for p in &event.paths {
                            if p.starts_with(&apps_root) {
                                apps_dirty = true;
                                matched = true;
                            }
                            if p.starts_with(&files_root) {
                                files_dirty = true;
                                matched = true;
                            }
                        }
                        if matched {
                            dirty_marks.fetch_add(1, Ordering::AcqRel);
                            last_dirty_at = Some(Instant::now());
                        }
                    }
                    Ok(Err(_)) => {}
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }

                if let Some(t) = last_dirty_at
                    && t.elapsed().as_millis() >= DEBOUNCE_MS
                    && (apps_dirty || files_dirty)
                {
                    let cooldown_ok = {
                        let last = last_refresh_ms.load(Ordering::Acquire);
                        last == 0 || now_unix_ms().saturating_sub(last) >= COOLDOWN_MS
                    };
                    if !cooldown_ok {
                        cooldown_skips.fetch_add(1, Ordering::AcqRel);
                        continue;
                    }
                    if in_progress
                        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                        .is_err()
                    {
                        continue;
                    }
                    let scope = BootstrapScope {
                        apps: apps_dirty,
                        files: files_dirty,
                        settings: false,
                    };
                    apps_dirty = false;
                    files_dirty = false;
                    last_dirty_at = None;
                    let db_for_worker = db_path.clone();
                    let in_progress_for_worker = in_progress.clone();
                    let last_refresh_for_worker = last_refresh_ms.clone();
                    let refreshes_for_worker = refreshes_fired.clone();

                    thread::spawn(move || {
                        // RAII guard: reset in_progress + stamp cooldown even on panic.
                        struct Guard {
                            flag: Arc<AtomicBool>,
                            stamp: Arc<AtomicU64>,
                            counter: Arc<AtomicU64>,
                        }
                        impl Drop for Guard {
                            fn drop(&mut self) {
                                self.stamp.store(now_unix_ms(), Ordering::Release);
                                self.flag.store(false, Ordering::Release);
                                self.counter.fetch_add(1, Ordering::AcqRel);
                            }
                        }
                        let _g = Guard {
                            flag: in_progress_for_worker,
                            stamp: last_refresh_for_worker,
                            counter: refreshes_for_worker,
                        };
                        let _ = QueryEngine::bootstrap_sqlite_scoped(&db_for_worker, scope);
                    });
                }
            }
        })
    };

    // ─── Producer worker ──────────────────────────────────────────────────
    // Mixes realistic noise + legit changes across both root families.
    let producer_handle = {
        let apps_root = apps_root.clone();
        let files_root = files_root.clone();
        let stop = stop.clone();
        thread::spawn(move || {
            let started = Instant::now();
            let mut tick = 0u64;
            while !stop.load(Ordering::Acquire) {
                tick += 1;

                // Every ~3 s: a legit file save in `files_root` (top-level).
                if tick.is_multiple_of(30) {
                    let p = files_root.join(format!("note-{tick}.md"));
                    let _ = fs::write(&p, "hello\n");
                }

                // Every ~1 s: a vim swap dance (noisy create + remove).
                if tick.is_multiple_of(10) {
                    let swap = files_root.join(".buffer.swp");
                    let _ = fs::write(&swap, "swap");
                    let _ = fs::remove_file(&swap);
                }

                // Every ~2 s: write to `.crdownload` (noisy).
                if tick.is_multiple_of(20) {
                    let dl = files_root.join("big.iso.crdownload");
                    let _ = fs::write(&dl, "partial");
                }

                // Every ~8 s: rename `.crdownload` to the final name (legit).
                if tick.is_multiple_of(80) {
                    let from = files_root.join("big.iso.crdownload");
                    let to = files_root.join(format!("big-{tick}.iso"));
                    let _ = fs::rename(&from, &to);
                }

                // Every ~5 s: drop a deep-tree file under files_root. With our
                // non-recursive watch, this MUST NOT generate any event.
                if tick.is_multiple_of(50) {
                    let deep = files_root.join("project/node_modules/foo");
                    let _ = fs::create_dir_all(&deep);
                    let _ = fs::write(deep.join(format!("pkg-{tick}.js")), "x");
                }

                // Every ~6 s: write a `.desktop` into `apps_root` (legit, apps).
                if tick.is_multiple_of(60) {
                    let app = apps_root.join(format!("fake-{tick}.desktop"));
                    let _ = fs::write(
                        &app,
                        format!(
                            "[Desktop Entry]\nType=Application\nName=Fake{tick}\nExec=/bin/true\n"
                        ),
                    );
                }

                if started.elapsed().as_secs() >= RUN_SECS {
                    break;
                }
                thread::sleep(Duration::from_millis(100));
            }
        })
    };

    producer_handle.join().expect("producer");
    // Give the watcher a final debounce + cooldown window to flush.
    thread::sleep(Duration::from_secs(3));
    stop.store(true, Ordering::Release);
    watcher_handle.join().expect("watcher");

    // ─── Report ───────────────────────────────────────────────────────────
    println!();
    println!(
        "{:<28}{:>10}",
        "events received from notify",
        events_received.load(Ordering::Acquire)
    );
    println!(
        "{:<28}{:>10}",
        "events filtered (noise)",
        events_filtered.load(Ordering::Acquire)
    );
    println!(
        "{:<28}{:>10}",
        "dirty marks (matched)",
        dirty_marks.load(Ordering::Acquire)
    );
    println!(
        "{:<28}{:>10}",
        "refreshes fired",
        refreshes_fired.load(Ordering::Acquire)
    );
    println!(
        "{:<28}{:>10}",
        "cooldown skips",
        cooldown_skips.load(Ordering::Acquire)
    );

    let cands = load_count(&db_path);
    println!("{:<28}{:>10}", "candidates in final DB", cands);

    // Cleanup. Best-effort; not a hard error if anything sticks.
    let _ = fs::remove_dir_all(&tempdir);
}

fn write_config(path: &Path, apps_root: &Path, files_root: &Path) {
    // Minimal config: point both root families at our tempdir, set tight
    // limits so the bootstrap is fast and bounded.
    let contents = format!(
        "app_scan_roots={}\n\
         app_scan_depth=2\n\
         file_scan_roots={}\n\
         file_scan_extra_roots=\n\
         file_scan_depth=2\n\
         file_scan_limit=5000\n\
         lazy_indexing_enabled=true\n",
        apps_root.display(),
        files_root.display(),
    );
    fs::write(path, contents).expect("write config");
}

fn load_count(db_path: &Path) -> usize {
    look_storage::SqliteStore::open(db_path)
        .and_then(|s| s.load_candidates(None))
        .map(|v| v.len())
        .unwrap_or(0)
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// Mirrors `apps/linows/src-tauri/src/state.rs` exactly.
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
    if event.paths.iter().all(|p| is_noisy_path(p)) {
        return false;
    }
    true
}

fn is_noisy_path(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
        return false;
    };
    if matches!(
        name,
        ".DS_Store" | "Thumbs.db" | "desktop.ini" | ".directory"
    ) {
        return true;
    }
    if name.starts_with("~$") || name.starts_with(".~") || name.starts_with(".#") {
        return true;
    }
    let lower = name.to_ascii_lowercase();
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
    NOISY_SUFFIXES.iter().any(|ext| lower.ends_with(ext))
}
