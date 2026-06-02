//! Times `bootstrap_sqlite_scoped` in each mode (ALL / APPS_ONLY / FILES_ONLY)
//! against the current machine's real file/app layout. This is the actual win
//! the watcher delivers: a watcher-triggered refresh that touches one source
//! should be measurably faster than rescanning everything.
//!
//! Run with:
//!   cargo run --release --bin scoped_refresh_bench --manifest-path tools/perf/Cargo.toml
//!
//! Uses a throwaway temp database (does not touch your real `~/.local/share/look/look.db`).
//! Each mode is run twice — the first run includes the SQLite WAL bootstrap and
//! initial inserts, the second is the "warm" path the watcher actually exercises.
use look_engine::{BootstrapScope, QueryEngine};
use look_storage::SqliteStore;
use std::env;
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn main() {
    let db_path = temp_db_path();
    // Make sure we start from a clean slate so the first ALL run isn't biased
    // by leftover state from a previous bench invocation.
    let _ = std::fs::remove_file(&db_path);

    println!(
        "scoped_refresh_bench: db={} pid={}\n",
        db_path.display(),
        std::process::id()
    );

    // First pass: pay the cold-start cost on each scope and report it.
    println!("# cold pass (first call writes WAL, inserts every row)");
    bench("ALL  ", BootstrapScope::ALL, &db_path);
    bench("APPS ", BootstrapScope::APPS_ONLY, &db_path);
    bench("FILES", BootstrapScope::FILES_ONLY, &db_path);

    println!("\n# warm pass (representative of watcher-triggered refreshes)");
    for _ in 0..2 {
        bench("ALL  ", BootstrapScope::ALL, &db_path);
        bench("APPS ", BootstrapScope::APPS_ONLY, &db_path);
        bench("FILES", BootstrapScope::FILES_ONLY, &db_path);
    }

    if let Ok(store) = SqliteStore::open(&db_path)
        && let Ok(cands) = store.load_candidates(None)
    {
        let apps = cands.iter().filter(|c| c.id.starts_with("app:")).count();
        let files = cands.iter().filter(|c| c.id.starts_with("file:")).count();
        let folders = cands.iter().filter(|c| c.id.starts_with("folder:")).count();
        let settings = cands.iter().filter(|c| c.id.starts_with("setting:")).count();
        println!(
            "\nfinal row count: total={}  apps={apps}  files={files}  folders={folders}  settings={settings}",
            cands.len()
        );
    }

    // Tidy up — only one bench process should ever be touching this file.
    let _ = std::fs::remove_file(&db_path);
}

fn bench(label: &str, scope: BootstrapScope, db_path: &PathBuf) {
    let started = Instant::now();
    match QueryEngine::bootstrap_sqlite_scoped(db_path, scope) {
        Ok(()) => {
            let elapsed = started.elapsed();
            println!("  {label} elapsed={}", fmt(elapsed));
        }
        Err(err) => {
            println!("  {label} FAILED: {err}");
        }
    }
}

fn fmt(d: Duration) -> String {
    let ms = d.as_secs_f64() * 1000.0;
    if ms >= 1.0 {
        format!("{ms:>8.2} ms")
    } else {
        format!("{:>8.0} µs", d.as_micros())
    }
}

fn temp_db_path() -> PathBuf {
    let base = env::temp_dir();
    base.join(format!("look-scoped-bench-{}.db", std::process::id()))
}
