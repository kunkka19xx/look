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
- [x] command keyboard shortcuts (Cmd+/, Cmd+1/2/3/4, Tab/Shift+Tab, Esc hide)
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

**Rust backend work (DONE):**
- [x] draft Windows shell source structure in `docs/windows-port-plan.md`
- [x] draft Rust platform refactor/change plan in `docs/windows-port-plan.md`
- [x] define Windows v1 parity checklist from current macOS behavior (`README.md`, `docs/user-guide.md`) -> `docs/windows-v1-parity-checklist.md`
- [x] split engine indexing into platform adapters (macOS/Windows) without changing ranking/search core
- [x] implement Windows app discovery sources (Start Menu + install roots fallback)
- [x] implement curated Windows Settings catalog (`ms-settings:` targets)
- [x] add Windows path defaults/normalization for config bootstrap and exclude handling
- [x] verify FFI API stability for multi-shell use and add Windows smoke coverage in CI

**Windows shell UI (DONE - UI mock-first):**
- [x] scaffold native Windows shell (`apps/windows/LauncherApp/`) with WinUI 3
- [x] wire FFI search + result rendering + keyboard navigation in Windows shell (mock-first)
- [x] implement Windows launcher UI screens (search, command mode with 2-column cards, clipboard placeholder, settings, help)
- [x] implement keyboard navigation and shortcuts UI bindings

**Windows real functionality (remaining items):**
- [x] implement Windows action dispatch (open, reveal in Explorer, copy, web handoff)
- [x] implement global hotkey + hide/show/focus lifecycle parity on Windows (Alt+Space toggle, Alt+Shift+Q quit, hide-on-focus-loss auto-dismiss, WS_EX_TOOLWINDOW hides from taskbar + Alt-Tab)
- [x] implement Windows clipboard history mode (`c"`) with listener-first capture strategy (`AddClipboardFormatListener` + `WM_CLIPBOARDUPDATE`, bounded history, persisted to `%LOCALAPPDATA%\look\clipboard-history.json`)
- [x] implement Windows command mode execution (`calc`, `shell`, `kill`, `sys`)
- [x] implement Windows launch-at-login integration (`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`; synced on app start + on Advanced Settings save)
- [x] implement Windows web translate mode (`t"`) with parallel VI/EN/JA fan-out, Enter-to-translate (matches macOS), per-section copy button, and "Open in Google Translate" handoff. `tw"` (Apple Dictionary lookup) deferred — no Windows equivalent of `DCSCopyTextDefinition`.
- [x] suppress console-window flashes on translate calls (`bridge/ffi/src/translate_api.rs` adds `CREATE_NO_WINDOW` 0x08000000 flag on the curl `Command` under `cfg(target_os = "windows")`)
- [x] align reveal shortcut with macOS: rebound `Ctrl+R` → `Ctrl+F` to match `Cmd+F`; moved `Ctrl+F` and `Ctrl+C` handlers to `GlobalKeyDown` so they fire while the search box has focus (previously only worked from the list view)
- [x] extract reusable XAML primitives for duplicated layouts: `KeyChordRowView` (Help + Shortcuts tabs), `LabeledSliderView` (Appearance + Advanced tabs), `CommandCardView` (CommandPanels), `TranslateLanguageSectionView` (TranslatePanel) — net –200+ XAML lines, single source of truth for visual tweaks
- [ ] **copy as real file object on Windows** (parity with macOS `pasteboard.writeObjects([targetURL as NSURL, result.path as NSString])` at `LauncherCommandService.swift:214`). Today `ActionDispatcher.CopyResultPath` only writes a text path via `package.SetText`. Should call `package.SetStorageItems([StorageFile/StorageFolder])` so Ctrl+V in Explorer pastes an actual file/folder, not a path string. Keep text fallback so apps that only accept text still work.
- [ ] **multi-pick result rows on Windows** (parity with macOS commit `74b619c`). Switch `ResultsList.SelectionMode` to `Multiple` (or `Extended`), wire Shift+Up/Down and Shift+Tab range selection, and update Ctrl+C/Ctrl+F/Enter handlers to operate on `ResultsList.SelectedItems` (list of `LauncherRowItem`) when count > 1 — copy multiple as a single `StorageItems` payload, reveal opens Explorer with the parent + multi-select, Enter opens each. Mirror selection-affordance UI (shaded + count badge) from macOS `LauncherSubviews.swift`.
- [ ] **filter file/non-text clipboard entries from history on Windows** (parity with macOS `ClipboardHistoryStore.swift:136 pasteboardCarriesFileReference`). Today `ClipboardHistoryService.cs:103` only checks `Contains(StandardDataFormats.Text)`, but a file copy in Explorer ALSO carries a synthesized text path, so file copies pollute history with raw paths. Add `view.Contains(StandardDataFormats.StorageItems)` short-circuit before the text capture path. Optional follow-up: store file-reference entries as a separate kind so they can be re-pasted as actual files (otherwise just skip them).
- [ ] implement Windows packaging/signing/release pipeline (`.msix`/`.msi`) and documentation
- [ ] run closed beta and fix top reliability/performance parity regressions before GA

Windows UI delivery note (mock-first):

- [x] use mock search provider by default to unblock UI parity work
- [x] keep FFI search provider wired behind provider abstraction for later backend re-enable

**Windows UI parity tasks (mock-first, match macOS behavior - UI only, NO real execution):**

- [x] define Windows design tokens (color, spacing, radius, typography) mapped from macOS theme semantics
- [x] create shared row component styling (icon, title, meta, selection state, divider)
- [x] define button variants (`primary`, `secondary`, `ghost`, `danger`) and interaction states
- [x] define message/banner system (`success`, `info`, `warning`, `error`) with copy-action support
- [x] declare launcher screens and states: search, command mode, clipboard mode, settings, help
- [x] declare empty/loading/error states for each launcher mode with stable copywriting
- [x] implement keyboard hint/footer style and per-mode hint mapping to match macOS intent
- [x] implement Advanced Settings screen with real .look.config persistence (background image, scan depth/limit, lazy indexing, log level, launch at login)
- [x] implement Shortcuts reference screen (Ctrl+ shortcuts)
- [x] implement Command mode 2-column card layout (calc, shell, kill, sys) with default calc selection
- [x] implement Command mode keyboard shortcuts (Ctrl+/ enter, Up/Down switch, Enter run, Ctrl+1/2/3 quick-select)
- [ ] add preview/right-panel layout parity spec for dictionary/result preview behavior
- [ ] add style documentation with screenshots for side-by-side macOS vs Windows parity QA

---

**Windows REAL functionality tasks (IN PROGRESS):**

- [x] implement FFI search connection (Rust backend working, search returns real results)
- [x] implement IconService for Windows icon extraction (SHGetFileInfo API)
- [x] fix icon display in WinUI 3 (stable icon rendering with shell extraction + cache fallback)
- [x] implement global hotkey + hide/show/focus lifecycle parity on Windows (Alt+Space toggle, Alt+Shift+Q quit, hide-on-focus-loss auto-dismiss, WS_EX_TOOLWINDOW hides from taskbar + Alt-Tab)
- [x] implement Windows clipboard history mode (`c"`) with listener-first capture strategy (`AddClipboardFormatListener` + `WM_CLIPBOARDUPDATE`, bounded history, persisted to `%LOCALAPPDATA%\look\clipboard-history.json`)
- [x] implement Windows command mode execution (`calc`, `shell`, `kill`, `sys`) with in-panel output and keyboard run flow
- [x] implement Windows launch-at-login integration (`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`; synced on app start + on Advanced Settings save)
- [x] implement Windows action dispatch (open, reveal in Explorer, copy, web handoff) with type-aware open handling (app/file/folder/setting/url)
- [x] implement Windows web translate mode (`t"`) — `look_translate_json` FFI binding, `TranslationService` with parallel vi/en/ja fan-out, `TranslatePanelView` with per-section copy + Open-in-Google-Translate handoff, Enter-to-translate (matches macOS); `CREATE_NO_WINDOW` on the curl Command suppresses console flashes

---

Windows immediate execution queue (current):

- [x] PR-1: add Rust platform module scaffold (`core/engine/src/platform/{macos,windows}`) and dispatch from index modules
- [x] PR-1: refactor config defaults/path handling for platform-aware roots and separator/case-safe path matching
- [x] PR-1: keep macOS behavior stable with regression tests for IDs and excludes
- [x] PR-1: add CI Windows Rust build/test lane (`core` + `bridge/ffi`)
- [x] PR-2: implement Windows app discovery adapters (Start Menu + fallback roots) with dedupe
- [x] PR-2: implement curated Windows settings catalog (`ms-settings:`) while keeping `setting:*` ID contract
- [x] PR-2: move platform-specific app discovery into `platform/macos/apps.rs` and `platform/windows/apps.rs`; keep `index/apps.rs` as dispatch only
- [x] PR-2: add Windows adapter unit tests (start-menu entry detection, fallback filtering, dedupe, merged roots, catalog integrity)
- [x] PR-3: reduce Windows app noise (helper executables, System32 guardrails, WindowsApps depth handling)
- [x] PR-3: add Windows fallback dedupe rules for Start Menu/WindowsApps/System32 overlap and keep real app preference
- [x] PR-3: expand Windows Settings catalog coverage and render settings with dedicated UI kind/icon

Windows command-mode parity updates (recent):

- [x] make command screen match macOS split layout (left command list + right content panel)
- [x] implement kill command running-app list parity with selection + confirmation flow
- [x] add kill-by-port query parity (`:3000`, `port 3000`) with empty-state messaging
- [x] improve kill process naming and system-noise filtering for Windows-specific background processes
- [x] align calc parser with macOS advanced features (`^`, `!`, `%`, `pi`, `e`, `sqrt/abs/round/floor/ceil`, aliases)
- [x] add finance-style percent semantics for add/subtract (`200 + 10%`, `200 - 10%`)
- [x] expand and group sys output sections (overview/performance/hardware/network)

Windows launcher polish (2026-Q2):

Search pipeline:

- [x] async search via `Task.Run` with version-based cancellation; debounce bumped 8 -> 16 ms
- [x] drop `FileVersionInfo.GetVersionInfo` from dedup hot path; Score breaks InstallExecutable ties
- [x] junk-path filter: `$RECYCLE.BIN`, `System Volume Information`, `$WINDOWS.~BT/~WS`, `Config.Msi`, `PerfLogs`, `Recovery`, `$GetCurrent`, `$SysReset`, `$INPLACE.~TR`, `\WindowsApps\`

Icon pipeline:

- [x] `IShellItemImageFactory` primary path for `shell:` URIs and `.lnk` stubs so UWP targets resolve to real tile logos
- [x] HBITMAP -> managed `Bitmap` with alpha preservation (manual DIB copy + un-premultiplication, topdown/bottomup handling, bounds checks)
- [x] `.url` internet shortcut parsing (`IconFile` / `IconIndex`)
- [x] relocate icon cache to `%LOCALAPPDATA%\look\icon-cache` so it survives reboots
- [x] log every previously-silent catch via `Debug.WriteLine` with context

UWP discovery + routing:

- [x] `Services/UwpAppService.cs` enumerates `shell:AppsFolder` via Shell.Application COM, filters to AUMID entries, fuzzy-merges with Rust results
- [x] dedup extended with `AppPathCategory.UwpAppsFolder` (rank 3); pairing generalized so UWP beats Start Menu `.lnk` and Program-Files exe on matching title
- [x] `ShellExecuteService` routes `shell:AppsFolder\<AUMID>` through `explorer.exe` so UWP entries launch cleanly

Appearance parity (Windows <- macOS):

- [x] port six macOS presets to Windows: Catppuccin, Tokyo Night, Rose Pine, Gruvbox, Dracula, Kanagawa
- [x] add three-tier text system (primary / secondary / muted) with `dimmableColor` (0.82 / 0.64) fallback and per-preset secondary/muted overrides
- [x] align search input theming: local `TextBox.Resources` aliases to `LauncherTextBrush` / `LauncherMutedTextBrush` / `LauncherAccentBrush` so it follows preset changes

Background + backdrop rendering:

- [x] Win2D pipeline for bg image: `CanvasBitmap` + `CanvasImageSource` with `GaussianBlurEffect`; live preview on slider drags; startup re-apply from config
- [x] composition `SpriteVisual` with `CompositionBackdropBrush` + `GaussianBlurEffect` gives Appearance blur sliders a real pixel-radius instead of only tint alpha

Advanced settings:

- [x] add Extra Scan Dirs UI (`file_scan_roots`) as macOS-style horizontal pill strip with inline `x` remove
- [x] redundancy check on add (home-relative path resolution + covering-entry detection, notice below Add button)
- [x] Skip Folders migrated to matching pill strip

Window + reliability:

- [x] borderless window chrome (`SetBorderAndTitleBar(false, false)` + `DWMWA_COLOR_NONE`)
- [x] `ResultsList_OnSelectionChanged` calls `ScrollIntoView` so Tab/Up/Down navigate off-viewport correctly
- [x] global hotkeys: Alt+Space toggle, Alt+Shift+Q quit, via `RegisterHotKey` + `SetWindowSubclass`
- [x] crash log moved to `%LOCALAPPDATA%\look\look-crash.log`; added `AppDomain.UnhandledException` + `TaskScheduler.UnobservedTaskException` handlers alongside the XAML one; each entry tagged with origin
- [x] hide-on-focus-loss auto-dismiss (Spotlight-style; `Window.Activated` -> `AppWindow.Hide` with `SuppressAutoHide()` scope for file/folder pickers)
- [x] hide launcher from taskbar and Alt-Tab (`WS_EX_TOOLWINDOW` + cleared `WS_EX_APPWINDOW`)
- [ ] Windows packaging / signing / release pipeline (`.msix` or `.msi`)

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
- [ ] **Quick Look file previews** *(pending perf validation, ships off by default)*: replace icon-only preview pane with macOS Quick Look rendering (PDF, text, code, audio/video poster, archives, office docs, etc.); requires debounced two-tier rendering, falls back to icon + metadata for unsupported/unreadable. Internal spec: `specs/quicklook-file-preview.md` (maintainer-only, gitignored)
- [ ] **Pomodoro command** (`/pomo`): focus timer in command mode with start/pause/stop, completion notification, and active-session indicator. Internal spec: `specs/pomodoro.md` (maintainer-only, gitignored)
- [ ] **Browser bookmark + history suggestions**: index the user's default browser (Safari/Chrome/Arc/Brave/Edge/Firefox) and surface bookmarks + recent pages as ranked launcher results. Internal spec: `specs/browser-suggestions.md` (maintainer-only, gitignored)
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
