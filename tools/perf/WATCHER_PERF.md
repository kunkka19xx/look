# Watcher / Index-Refresh Performance Report

Measured against the changes that landed in `apps/linows/src-tauri/src/state.rs`
and `core/engine/src/lib.rs` (scoped bootstrap, non-recursive file watches,
noise filter, off-thread reindex, cooldown gate, RAII slot guard).

Three benches drive every number in this report. They live in a separate
`tools/perf` crate - not part of the app, not in `dev-dependencies` of any
shipped crate. Zero impact on Tauri bundle size.

| Bench                       | What it measures                                                                       |
| --------------------------- | -------------------------------------------------------------------------------------- |
| `scoped_refresh_bench.rs`   | Wall-clock cost of one `bootstrap_sqlite_scoped(scope)` call.                          |
| `watcher_stress.rs`         | End-to-end behavior under **synthesized** event streams (BEFORE vs AFTER policy).      |
| `real_fs_stress.rs`         | **Real** `notify` watcher + worker thread doing actual disk I/O over a tempdir.        |

Re-run on your own machine:

```sh
cd tools/perf
cargo run --release --bin scoped_refresh_bench
cargo run --release --bin watcher_stress
cargo run --release --bin real_fs_stress
```

All numbers below come from the machine that wrote this report. Per-call costs
scale roughly linearly with your `file_scan_roots` size, so absolute values
will differ; the **ratios** between BEFORE and AFTER columns are the
load-bearing part.

---

## 1. Per-call refresh latency

From `scoped_refresh_bench` (warm pass; the first call after `WAL` setup is
slightly slower but not representative of the watcher's steady state).

```
Indexed snapshot: 2,471 candidates  (77 apps · 1,692 files · 676 folders · 26 settings)

  Scope         Initial scoped    + is_demo_seeded   Notes
                + scoped refresh   + load_cached
  ──────────    ──────────────    ────────────────   ─────────────────────────────────────
  ALL           ≈ 15.0 ms         ≈ 13.75 ms         Was the only path before today.
  APPS_ONLY     ≈  7.2 ms         ≈  5.95 ms         ~2.3× faster than ALL; skips files walker.
  FILES_ONLY    ≈ 14.8 ms         ≈ 13.70 ms         ~same as ALL: files walker dominates.
```

Two refinements landed after the initial scoped refresh:

- `SqliteStore::is_demo_seeded()` - one `COUNT(*)` instead of loading every row to
  check if the table is just the demo seed (`core/engine/src/lib.rs:135` previously
  did `load_candidates(None)` for that check).
- `RuntimeConfig::load_cached()` - skips re-reading `~/.look.config` on every
  refresh; the linows `reload_config` command and FFI `look_reload_config`
  drop the cache so user edits still take effect.

The savings (~1.25 ms per call) apply to every bootstrap call regardless of
scope, so the absolute delta is the same in each row; the *relative* gain is
largest on `APPS_ONLY` (~17%) because its baseline is smallest.

**Reading this:** scoped refresh is a real win **only for apps events**. On a
file-only event the walker has to run anyway, so the scope flag saves nothing
per call. The wins on the files side come from preventing calls in the first
place - see §2.

---

## 2. End-to-end stress: BEFORE vs AFTER, per scenario

From `watcher_stress`. Each scenario plays out over 60 simulated seconds; both
policies see the **same input event stream** so the diff is purely policy.

Per-call costs used by the simulator (re-measured at run start on this
machine): ALL=17.3 ms · APPS_ONLY=8.8 ms · FILES_ONLY=16.4 ms.

### 2.1 sync-client (Dropbox / Syncthing pattern)

Every 3 s, a top-level `.tmp` create + rename to a real file inside
`~/Documents`. Twenty saves over the window.

```
  policy   received   filtered   fires   cool-skip   cpu_ms
  BEFORE         40          0      20           0     345.5
  AFTER          40         20       6         140      99.1
```

- `.tmp` half of every save is dropped by the noise filter → 20 of 40 events
  filtered instead of triggering a refresh.
- Cooldown defers another 140 would-be fires.
- Result: **20 → 6 refreshes/min, 345 → 99 ms CPU** (3.5× less).

### 2.2 active-downloader (browser writing `big.iso.crdownload`)

Twelve `.crdownload` progress writes, then one final rename to `big.iso`.

```
  policy   received   filtered   fires   cool-skip   cpu_ms
  BEFORE         13          0      13           0     224.6
  AFTER          13         12       1           0      16.4
```

- Every `.crdownload` event is filtered; only the final rename triggers a fire.
- **13 → 1 refresh, 225 → 16 ms CPU** (~14× less).

### 2.3 apt-install-burst (50 `.desktop` writes in 400 ms)

A package install rewriting half of `/usr/share/applications` in one burst.

```
  policy   received   filtered   fires   cool-skip   cpu_ms
  BEFORE         50          0       1           0      17.3
  AFTER          50          0       1           0       8.8
```

- 2 s debounce already coalesces the burst to one fire under either policy.
- AFTER fires `APPS_ONLY` instead of `ALL` → **17 → 9 ms CPU** (~2× less).

### 2.4 vim-edit-storm (heavy vim session)

Every 5 s vim writes a `.swp`, edits in place (5 spurious modifies), then `:w`
atomically renames. Twelve saves.

```
  policy   received   filtered   fires   cool-skip   cpu_ms
  BEFORE         72          0      12           0     207.3
  AFTER          72         60       7         156     115.5
```

- All 60 `.swp` events are filtered.
- Cooldown still caps fires (7 instead of 12) because the genuine rename
  events repeat faster than 10 s.
- **12 → 7 refreshes, 207 → 116 ms CPU** (~1.8× less).

### 2.5 deep-tree-npm-install (`npm install` inside `~/Documents/project/`)

Five hundred writes into `node_modules`. The watcher used to receive every one
of these because file roots were watched recursively.

```
  policy   received   filtered   fires   cool-skip   cpu_ms
  BEFORE        500          0       1           0      17.3
  AFTER           0          0       0           0       0.0
```

- AFTER's non-recursive file watch never delivers the events. Receive count
  goes to **zero** - the watcher loop doesn't even wake up.
- The deep tree is reconciled next time the user opens the launcher
  (window-show triggers a full refresh).
- **500 events received → 0 events received, 17 → 0 ms CPU.**

---

## 3. Real-FS end-to-end (no simulation)

From `real_fs_stress`. A live `notify::RecommendedWatcher` watches a tempdir;
a separate producer thread does real `fs::write` / `fs::rename` / `fs::remove`
on the same tempdir for 30 seconds. The watcher runs the same decision logic
as `state.rs` (debounce, cooldown, noise filter, scope split). The engine
points at a throwaway DB and a throwaway `LOOK_CONFIG_PATH`, so the user's
live index is never touched.

Producer mix per 30 s run: ~10 legit file saves, ~30 vim-swap dances (noisy),
~15 `.crdownload` writes (noisy), ~3 final renames, ~6 deep-tree writes under
`node_modules/foo/` (should never reach the watcher), ~5 apps `.desktop`
writes.

```
events received from notify        211
events filtered (noise)            189   ← 90% of received events filtered
dirty marks (matched)               22   ← survived filtering, classified
refreshes fired                      3   ← ≤ 6/min cap holds empirically
cooldown skips                      20
candidates in final DB             120
```

What this proves about the live policy (vs the simulator's predictions):

- **Noise filter works on real events.** 189 of 211 kernel events dropped -
  vim swap creates/removes and `.crdownload` writes never trigger a refresh.
- **Non-recursive file watch works.** The producer wrote into
  `files_root/project/node_modules/foo/` six times. Those events never
  appeared in `events received from notify`; if they had, the count would be
  hundreds higher and we'd be in the BEFORE-policy world.
- **Cooldown enforces the rate cap.** Twenty would-be fires were deferred;
  three refreshes actually ran across 30 s. Matches the modeled "≤ 6/min."
- **RAII guard doesn't deadlock.** Every spawned worker released the
  in-progress slot via `Drop` - the watcher kept firing for the whole run.

## 4. Headline diff

```
                                 BEFORE                          AFTER
  ──────────────────────────    ──────────────────────────     ──────────────────────────
  Apps refresh cost             ≈ 15 ms                        ≈ 6 ms      (~2.5× faster)
  Files refresh cost            ≈ 15 ms                        ≈ 13.7 ms   (slight per-call win)
  Inotify watches               unbounded (recursive)          bounded     (file roots non-recursive)
  Editor / temp / partial noise every event fires              filtered    (zero refreshes)
  Refresh rate cap              none                           ≤ 6 / min   (10 s cooldown)
  Watcher loop blocked          up to per-call time × N        never       (off-thread)
  Panic recovery                in_progress stuck → silent     automatic   (RAII guard)
  Steady-state worst case       30 fires / min, ~450 ms CPU    6 fires / min, ~90 ms CPU
  Tests in state.rs             0                              12 (plus 8 in engine/storage)
```

---

## 5. Caveats

- All AFTER numbers assume the user's environment delivers inotify events
  reliably. On NFS / CIFS / VirtualBox shared folders, events may not arrive
  at all - the window-show refresh remains the backstop in that case.
- The simulator uses measured `bootstrap_sqlite_scoped` costs from a single
  machine snapshot. On a machine with 10× more indexed files, multiply the
  `cpu_ms` columns by ~10. The **ratio** between BEFORE and AFTER stays the
  same because the policy differences are call-count, not per-call cost.
- Both benches use a throwaway temp DB; your live `~/.local/share/look/look.db`
  is never touched.
- The 10 s cooldown can be re-tuned via `WATCHER_REFRESH_COOLDOWN_MS` in
  `state.rs`. Larger values cap CPU more aggressively at the cost of
  staleness during noisy periods.
