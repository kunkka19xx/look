# Implementation Tasks

This is the execution breakdown for the next backend-focused milestone.

## Priority queue (current)

Now:

- [x] benchmark query latency and indexing throughput
- [x] collect first dated baseline note in `docs/bench-notes/YYYY-MM-DD.md`
- [x] add diagnostics/debug toggles for development

Next:

- [x] keep `.look.config` as canonical user settings source (no backend table migration for user-editable settings)
- [x] finish open Milestone G reliability tasks (error model + user fallback messaging)
- [x] config validation and reload feedback (Cmd+Shift+; shows warnings in orange banner)
- [x] file_scan_depth/file_scan_limit validation (clamp 1-12 / 500-50000 in both Swift and Rust)
- [x] add `/System/Library/CoreServices/Applications` to default app scan roots and add regression test for `Keychain Access` discoverability (`keychain` query)

Recently completed (current optimization cycle):

- [x] move hot-path candidate normalization to load-time precompute
- [x] replace greedy fuzzy base matching with bounded DP scorer
- [x] switch usage boost to logarithmic scaling
- [x] tune settings/query disambiguation (`ingo` noise fix + `sett` prefix intent)
- [x] add compact FFI search payload and switch UI bridge default to compact path
- [x] stream indexing with bounded channels and chunked bootstrap upsert
- [x] add stale candidate cleanup and usage-event retention policy
- [x] migrate `Candidate` read-mostly text fields to `Box<str>`

## Milestone A: Storage foundation (SQLite)

- [x] add `core/storage` SQLite layer and connection manager
- [x] define schema (`candidates`, `usage_events`, `settings`, `index_state`)
- [x] add migration system with schema versioning
- [x] add CRUD APIs for candidates and usage events
- [x] add unit tests for migration and basic read/write

## Milestone B: Unified engine model

- [x] define shared `ActionKind` in core
- [x] refactor engine query output to typed action results (`LaunchResult`, `LaunchResultAction`)
- [x] ensure result payloads can cross FFI boundary cleanly
- [ ] add result pagination/streaming for large result sets (optional)

## Milestone C: Indexing pipeline

- [x] implement app index source (`/Applications`, `/System/Applications`, `~/Applications`)
- [x] implement file/folder index source with root config defaults
- [x] add System Settings source from `.appex`/`.prefPane` discovery
- [x] support exclude paths and hidden file policy (`app_exclude_paths`, `app_exclude_names`, `file_exclude_paths`)
- [x] add startup full scan + snapshot upsert
- [x] persist index snapshots to SQLite
- [x] move indexing to streaming source pipeline with bounded channel backpressure
- [x] add worker panic diagnostics for indexing threads
- [x] switch file traversal to `ignore::WalkBuilder`
- [x] add chunked bootstrap upsert path (no full discovery vector required)

## Milestone D: Bridge and app integration

- [x] finalize FFI API (`init`, `search`, `record_action`, `translate`)
- [x] switch SwiftUI launcher from seed data to engine results
- [x] wire action execution path (open app/path/web, run command)
- [x] translate text via FFI (network-gated)
- [x] command mode with `calc`, `shell`, `kill` commands
- [x] command keyboard shortcuts (Cmd+/, Cmd+1/2/3, Tab, Esc hide, Cmd+Esc)
- [x] keep settings persistence file-based via `.look.config` (user-portable, copyable config)
- [x] add structured error mapping for UI feedback

## Milestone E: Ranking and safety

- [x] log execution events to `usage_events`
- [x] implement recency/frequency score boost
- [x] apply shell safety policy (`sudo` warning, confirm mode option)
- [x] add numeric guardrails for calc
- [x] add tests for scoring and safety rules (scoring coverage added)

## Milestone F: Performance and polish

- [x] benchmark query latency and indexing throughput
- [ ] add in-memory cache for top-N results
- [x] optimize startup path and background index scheduling
- [x] add diagnostics/debug toggles for development
- [x] update docs and user guide for finalized behavior
- [x] remove hot-loop normalization allocations via indexed candidate precompute
- [x] reduce ranking-heap extra string allocations in rerank flow
- [x] optimize FFI search payload size with compact endpoint

## Milestone H: Data lifecycle and retention

- [x] add candidate `indexed_at_unix_s` watermarking in SQLite
- [x] delete stale candidates not refreshed in current index run
- [x] clean legacy `NULL` indexed candidates during stale cleanup
- [x] prune `usage_events` by age window (90 days)
- [x] enforce max `usage_events` cap (50,000 rows)

## Milestone W: Windows port (separate track)

Reference: `docs/windows-port-plan.md`

- [x] draft Windows shell source structure in `docs/windows-port-plan.md`
- [x] draft Rust platform refactor/change plan in `docs/windows-port-plan.md`
- [x] define Windows v1 parity checklist from current macOS behavior (`README.md`, `docs/user-guide.md`) -> `docs/windows-v1-parity-checklist.md`
- [ ] split engine indexing into platform adapters (macOS/Windows) without changing ranking/search core
- [ ] implement Windows app discovery sources (Start Menu + install roots fallback)
- [ ] implement curated Windows Settings catalog (`ms-settings:` targets)
- [ ] add Windows path defaults/normalization for config bootstrap and exclude handling
- [ ] verify FFI API stability for multi-shell use and add Windows smoke coverage in CI
- [ ] scaffold native Windows shell (`apps/windows/LauncherApp/`) with WinUI 3
- [ ] wire FFI search + result rendering + keyboard navigation in Windows shell
- [ ] implement Windows action dispatch (open, reveal in Explorer, copy, web handoff)
- [ ] implement global hotkey + hide/show/focus lifecycle parity on Windows
- [ ] implement Windows clipboard history mode (`c"`) with listener-first capture strategy
- [ ] implement Windows command mode parity (`calc`, `shell`, `kill`, `sys`)
- [ ] implement Windows launch-at-login integration
- [ ] add Windows packaging/signing/release pipeline (`.msix`/`.msi`) and documentation
- [ ] run closed beta and fix top reliability/performance parity regressions before GA

Windows immediate execution queue (current):

- [ ] PR-1: add Rust platform module scaffold (`core/engine/src/platform/{macos,windows}`) and dispatch from index modules
- [ ] PR-1: refactor config defaults/path handling for platform-aware roots and separator/case-safe path matching
- [ ] PR-1: keep macOS behavior stable with regression tests for IDs and excludes
- [ ] PR-1: add CI Windows Rust build/test lane (`core` + `bridge/ffi`)
- [ ] PR-2: implement Windows app discovery adapters (Start Menu + fallback roots) with dedupe
- [ ] PR-2: implement curated Windows settings catalog (`ms-settings:`) while keeping `setting:*` ID contract

## Milestone G: Reliability (errors, tests, logs)

- [x] complete phase-2 structured error model across engine/storage/ffi boundaries
- [x] add safe user-facing fallback messages for action failures
- [x] add unit tests for search scoring and empty-query top-picks behavior
- [ ] add unit tests for curated settings catalog integrity (id/title/target validity)
- [x] add storage tests for usage-event writes and candidate upsert semantics
- [x] add ffi-level smoke tests for `look_search_json` and `look_record_usage`
- [x] add debug logging hooks (startup indexing summary, query timing, action execution outcome)
- [x] add a log-level toggle (`error`/`info`/`debug`) via env var for local troubleshooting
- [ ] add optional persistent backend log file with size-based rotation and retention cap

## Backlog: UI Enhancements

- [x] **App list preview**: 2-column layout with icon/name on left, info/preview on right (image preview, app info)
- [x] **System info command**: Add `sys` command mode screen for model, memory, CPU usage, battery, uptime, and disk
- [x] **Command list 2-column**: Make command list 2-column layout for better visibility
- [x] **Theme presets foundation** ([#54](https://github.com/kunkka19xx/look/issues/54)): built-in theme preset system with Catppuccin, Tokyo Night, Rose Pine, Gruvbox, Dracula, Kanagawa; semantic colors auto-derive from main text in Custom mode; theme persisted to `ui_theme` config
- [x] **Add preset: Catppuccin** ([#54](https://github.com/kunkka19xx/look/issues/54))
- [x] **Add preset: Tokyo Night** ([#54](https://github.com/kunkka19xx/look/issues/54))
- [x] **Add preset: Rose Pine** ([#54](https://github.com/kunkka19xx/look/issues/54))
- [x] **Add preset: Gruvbox** ([#54](https://github.com/kunkka19xx/look/issues/54))
- [x] **Add preset: Dracula** ([#54](https://github.com/kunkka19xx/look/issues/54))
- [x] **Add preset: Kanagawa** ([#54](https://github.com/kunkka19xx/look/issues/54))
- [x] **Config validation**: validate config file on reload (Cmd+Shift+;), show warnings for invalid values with orange banner + copy button; clamp file_scan_depth (1-12) and file_scan_limit (500-50000) in both Swift and Rust
- [ ] **Theme codegen pipeline (future)**: define a shared theme token source and generate typed platform code at build time
- [ ] **Homebrew release**: Package app for homebrew installation
- [x] **Build script**: Create release build and curl installer script

## Evergreen: Search quality and performance (always-on)

- [ ] **Indexing improvement loop**: continually refine scan roots, excludes, and incremental refresh strategy
- [ ] **Matching improvement loop**: improve typo tolerance, tokenization, and relevance scoring for mixed app/file queries
- [ ] **Optimization loop**: keep reducing query latency, startup cost, and memory use as regular maintenance

## Weekly checklist: quality + performance

Run this checklist at least once per week (or before release cut):

- [ ] collect baseline metrics from the same sample dataset and keep results in a dated note (`docs/bench-notes/YYYY-MM-DD.md`)
- [ ] measure query latency (`p50`, `p95`) for empty query, short query (2-4 chars), and long query (8+ chars)
- [ ] measure startup time (app launch -> first usable search result)
- [ ] compare index size and memory usage versus last baseline
- [ ] verify top-5 relevance for a fixed smoke query set (apps, files, folders, settings)
- [ ] review at least 3 recent user-reported misses and convert into matching/indexing improvements
- [ ] add/update at least one test for any ranking/matching/indexing behavior change

Suggested guardrails (adjust as project evolves):

- `query latency p50`: <= 30ms
- `query latency p95`: <= 80ms
- `startup to first result`: <= 700ms
- `peak memory (idle window)`: <= 220MB
- `relevance smoke pass rate`: >= 90% in top-5

Escalation rule:

- if any guardrail regresses by >10% week-over-week, open a focused perf/quality issue before merging unrelated polish work
