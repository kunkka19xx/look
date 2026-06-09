//! Models the linows watcher's decision logic against synthesized event
//! streams and reports BEFORE vs AFTER behavior side-by-side. Per-call refresh
//! costs are measured live on the current machine (one warm `ALL`, one warm
//! `APPS_ONLY`, one warm `FILES_ONLY`) so the simulated CPU columns reflect
//! whatever filesystem the bench is run on.
//!
//! Run with:
//!   cargo run --release --example watcher_stress --manifest-path core/Cargo.toml
//!
//! BEFORE policy (state of `state.rs` prior to today's changes):
//!   • no noise filter (all create/remove/rename events pass)
//!   • recursive file watches (deep-tree changes also reach the loop)
//!   • 2 s debounce, no cooldown
//!   • every fired refresh = `bootstrap_sqlite(ALL)`
//!
//! AFTER policy:
//!   • `should_mark_dirty` / `is_noisy_path` filter
//!   • non-recursive file watches (only top-level events arrive)
//!   • 2 s debounce + 10 s cooldown
//!   • scoped refresh: apps-only when only apps dirty, etc.
use look_engine::{BootstrapScope, QueryEngine};
use std::env;
use std::path::{Path, PathBuf};
use std::time::Instant;

const DEBOUNCE_MS: f64 = 2_000.0;
const COOLDOWN_MS: f64 = 10_000.0;
const SCENARIO_SECS: f64 = 60.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    Apps,
    Files,
    DeepFiles, // events from depth > 1 under a file root — only the BEFORE
               // policy sees these (recursive watch); AFTER's non-recursive
               // watch never delivers them.
}

#[derive(Clone)]
struct SimEvent {
    t_ms: f64,
    source: Source,
    /// Kept for trace-level debugging; not read by the simulators themselves.
    #[allow(dead_code)]
    path: PathBuf,
    is_noisy: bool,
}

#[derive(Default, Debug)]
struct PolicyResult {
    events_received: usize,
    events_filtered: usize,
    refreshes_fired: usize,
    refreshes_deferred_by_cooldown: usize,
    cpu_ms: f64,
}

fn main() {
    let db = temp_db_path();
    let _ = std::fs::remove_file(&db);

    // Bring the DB up to "warm" and measure per-scope cost on this machine.
    QueryEngine::bootstrap_sqlite(&db).expect("warm bootstrap");
    let cost_all = measure(&db, BootstrapScope::ALL);
    let cost_apps = measure(&db, BootstrapScope::APPS_ONLY);
    let cost_files = measure(&db, BootstrapScope::FILES_ONLY);

    println!("watcher_stress: measured per-call costs on this machine:");
    println!("  ALL        = {cost_all:>6.2} ms");
    println!("  APPS_ONLY  = {cost_apps:>6.2} ms");
    println!("  FILES_ONLY = {cost_files:>6.2} ms\n");

    let scenarios = vec![
        ("sync-client",           sync_client_scenario()),
        ("active-downloader",     active_downloader_scenario()),
        ("apt-install-burst",     apt_install_burst_scenario()),
        ("vim-edit-storm",        vim_edit_storm_scenario()),
        ("deep-tree-npm-install", deep_tree_scenario()),
    ];

    println!(
        "{:<24}{:<10}{:>10}{:>10}{:>10}{:>12}{:>10}",
        "scenario", "policy", "received", "filtered", "fires", "cool-skip", "cpu_ms"
    );
    println!("{}", "─".repeat(86));

    for (name, events) in &scenarios {
        let before = simulate_before(events, cost_all);
        let after = simulate_after(events, cost_all, cost_apps, cost_files);
        print_row(name, "BEFORE", &before);
        print_row(name, "AFTER",  &after);
        println!();
    }

    let _ = std::fs::remove_file(&db);
}

fn print_row(name: &str, policy: &str, r: &PolicyResult) {
    println!(
        "{:<24}{:<10}{:>10}{:>10}{:>10}{:>12}{:>10.1}",
        name,
        policy,
        r.events_received,
        r.events_filtered,
        r.refreshes_fired,
        r.refreshes_deferred_by_cooldown,
        r.cpu_ms,
    );
}

fn measure(db: &Path, scope: BootstrapScope) -> f64 {
    // One warm-up + three timed runs; report the median.
    let _ = QueryEngine::bootstrap_sqlite_scoped(db, scope);
    let mut samples = [0.0f64; 3];
    for slot in samples.iter_mut() {
        let t = Instant::now();
        let _ = QueryEngine::bootstrap_sqlite_scoped(db, scope);
        *slot = t.elapsed().as_secs_f64() * 1000.0;
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    samples[1]
}

// ── Policy simulators ───────────────────────────────────────────────────────

/// BEFORE: every received event (incl. deep + noisy) marks dirty; debounce only;
/// every fire is a full `ALL` bootstrap.
fn simulate_before(events: &[SimEvent], cost_all_ms: f64) -> PolicyResult {
    let mut r = PolicyResult::default();
    let mut dirty_since: Option<f64> = None;

    // Drive the loop minute-by-100ms-tick. At each tick, drain events up to
    // `now`, then check if debounce expired.
    let mut idx = 0;
    let mut tick = 0.0f64;
    while tick <= SCENARIO_SECS * 1000.0 {
        while idx < events.len() && events[idx].t_ms <= tick {
            // BEFORE policy: no path filter, deep events also count.
            r.events_received += 1;
            dirty_since = Some(tick);
            idx += 1;
        }
        if let Some(d) = dirty_since
            && tick - d >= DEBOUNCE_MS
        {
            r.refreshes_fired += 1;
            r.cpu_ms += cost_all_ms;
            dirty_since = None;
        }
        tick += 100.0;
    }
    // Tail flush: process any remaining dirtiness.
    if dirty_since.is_some() {
        r.refreshes_fired += 1;
        r.cpu_ms += cost_all_ms;
    }
    r
}

/// AFTER: noise filter + non-recursive (deep events dropped before they arrive)
/// + debounce + 10 s cooldown + scope-aware refresh.
fn simulate_after(
    events: &[SimEvent],
    cost_all_ms: f64,
    cost_apps_ms: f64,
    cost_files_ms: f64,
) -> PolicyResult {
    let mut r = PolicyResult::default();
    let mut dirty_since: Option<f64> = None;
    let mut apps_dirty = false;
    let mut files_dirty = false;
    let mut last_refresh_at: Option<f64> = None;

    let mut idx = 0;
    let mut tick = 0.0f64;
    while tick <= SCENARIO_SECS * 1000.0 {
        while idx < events.len() && events[idx].t_ms <= tick {
            let ev = &events[idx];
            idx += 1;

            // Non-recursive: the kernel never delivers deep-tree events to us.
            if matches!(ev.source, Source::DeepFiles) {
                continue;
            }
            r.events_received += 1;
            // Noise filter: vim swaps, .crdownload, ~$lockfiles etc.
            if ev.is_noisy {
                r.events_filtered += 1;
                continue;
            }
            match ev.source {
                Source::Apps => apps_dirty = true,
                Source::Files => files_dirty = true,
                Source::DeepFiles => unreachable!(),
            }
            dirty_since = Some(tick);
        }

        if let Some(d) = dirty_since
            && tick - d >= DEBOUNCE_MS
            && (apps_dirty || files_dirty)
        {
            let cooldown_ok = last_refresh_at.is_none_or(|last| tick - last >= COOLDOWN_MS);
            if cooldown_ok {
                let scope = BootstrapScope {
                    apps: apps_dirty,
                    files: files_dirty,
                    settings: false,
                };
                let cost = if scope.is_all() {
                    cost_all_ms
                } else if scope.apps && !scope.files {
                    cost_apps_ms
                } else if scope.files && !scope.apps {
                    cost_files_ms
                } else {
                    cost_all_ms
                };
                r.refreshes_fired += 1;
                r.cpu_ms += cost;
                last_refresh_at = Some(tick);
                apps_dirty = false;
                files_dirty = false;
                dirty_since = None;
            } else {
                r.refreshes_deferred_by_cooldown += 1;
                // Stay dirty; we'll try again on the next tick.
            }
        }
        tick += 100.0;
    }
    if dirty_since.is_some() && (apps_dirty || files_dirty) {
        // Tail flush past simulation horizon.
        r.refreshes_fired += 1;
        r.cpu_ms += cost_all_ms;
    }
    r
}

// ── Scenarios ───────────────────────────────────────────────────────────────

/// Cloud-sync client (Dropbox-style) doing periodic top-level writes in
/// `~/Documents`. Each "save" is an atomic write: a `.tmp` create, then a
/// rename to the final file. ~20 saves over the scenario.
fn sync_client_scenario() -> Vec<SimEvent> {
    let mut out = Vec::new();
    let mut t = 1_000.0;
    let docs = PathBuf::from("/home/u/Documents");
    while t < SCENARIO_SECS * 1000.0 {
        out.push(SimEvent {
            t_ms: t,
            source: Source::Files,
            path: docs.join("sync.tmp"),
            is_noisy: true,
        });
        out.push(SimEvent {
            t_ms: t + 50.0,
            source: Source::Files,
            path: docs.join("note.md"),
            is_noisy: false,
        });
        t += 3_000.0;
    }
    out
}

/// Browser progressively writing a big download. The `.crdownload` writes are
/// noisy; the final rename to the real filename is not.
fn active_downloader_scenario() -> Vec<SimEvent> {
    let mut out = Vec::new();
    let dl = PathBuf::from("/home/u/Downloads");
    // Browsers don't fire a per-byte Create, but they do fire renames each
    // time the file is opened/closed. Model ~12 such events plus 1 final.
    for i in 0..12 {
        out.push(SimEvent {
            t_ms: 1_000.0 + (i as f64) * 4_000.0,
            source: Source::Files,
            path: dl.join("big.iso.crdownload"),
            is_noisy: true,
        });
    }
    // Final rename to the real file 50 s in.
    out.push(SimEvent {
        t_ms: 50_000.0,
        source: Source::Files,
        path: dl.join("big.iso"),
        is_noisy: false,
    });
    out
}

/// `apt install firefox`: a burst of ~50 writes to `/usr/share/applications`
/// in a few hundred ms, then quiet for the rest of the scenario.
fn apt_install_burst_scenario() -> Vec<SimEvent> {
    let mut out = Vec::new();
    let apps = PathBuf::from("/usr/share/applications");
    for i in 0..50 {
        out.push(SimEvent {
            t_ms: 5_000.0 + (i as f64) * 8.0,
            source: Source::Apps,
            path: apps.join(format!("firefox-{i}.desktop")),
            is_noisy: false,
        });
    }
    // Quiet otherwise.
    out
}

/// Heavy vim session: every 5 s vim writes a `.swp`, edits in place, then on
/// `:w` performs an atomic rename (`.swp` → final). Only the rename is
/// non-noisy. Files root.
fn vim_edit_storm_scenario() -> Vec<SimEvent> {
    let mut out = Vec::new();
    let docs = PathBuf::from("/home/u/Documents");
    let mut t = 1_000.0;
    while t < SCENARIO_SECS * 1000.0 {
        out.push(SimEvent {
            t_ms: t,
            source: Source::Files,
            path: docs.join(".notes.txt.swp"),
            is_noisy: true,
        });
        // vim writes lots of tick events; model 5 swap-modifies per save
        for k in 1..5 {
            out.push(SimEvent {
                t_ms: t + (k as f64) * 200.0,
                source: Source::Files,
                path: docs.join(".notes.txt.swp"),
                is_noisy: true,
            });
        }
        // The :w rename:
        out.push(SimEvent {
            t_ms: t + 1_500.0,
            source: Source::Files,
            path: docs.join("notes.txt"),
            is_noisy: false,
        });
        t += 5_000.0;
    }
    out
}

/// `npm install` running inside `~/Documents/project/`: thousands of writes
/// into `node_modules`. Under BEFORE policy these all reach the watcher.
/// Under AFTER policy the non-recursive watch never delivers them — so they
/// are tagged `DeepFiles` and the simulator drops them in the AFTER pass.
fn deep_tree_scenario() -> Vec<SimEvent> {
    let mut out = Vec::new();
    let nm = PathBuf::from("/home/u/Documents/project/node_modules");
    // 500 events evenly over 30 seconds.
    for i in 0..500 {
        out.push(SimEvent {
            t_ms: 1_000.0 + (i as f64) * 60.0,
            source: Source::DeepFiles,
            path: nm.join(format!("pkg-{i}/index.js")),
            is_noisy: false,
        });
    }
    out
}

fn temp_db_path() -> PathBuf {
    env::temp_dir().join(format!("look-watcher-stress-{}.db", std::process::id()))
}
