# Windows Port Plan

This document describes a step-by-step plan to port `look` from the current macOS shell to a Windows-native launcher while preserving UI behavior and core functionality.

## Goal

Ship a Windows launcher with the same product behavior as macOS:

- keyboard-first global launcher UX
- app/file/folder search and launch actions
- clipboard history mode (`c"`)
- command mode (`calc`, `shell`, `kill`, `sys`)
- translation and dictionary flows where supported
- local-first performance profile

Windows v1 parity checklist reference: `docs/windows-v1-parity-checklist.md`

## Current architecture baseline

Today the codebase is split as:

- macOS shell: `apps/macos/LauncherApp/look-app/` (Swift/AppKit)
- shared Rust backend: `core/` (indexing, matching, ranking, storage)
- FFI bridge: `bridge/ffi/`

The Windows port should keep this split:

- add a Windows native shell (recommended: WinUI 3)
- keep Rust engine + storage shared
- keep FFI boundary narrow and stable

## Proposed Windows app source structure

```text
apps/windows/LauncherApp/
├── look-win.sln
├── launcher-app/
│   ├── launcher-app.csproj
│   ├── App.xaml
│   ├── App.xaml.cs
│   ├── app.manifest
│   ├── Assets/
│   ├── Core/
│   │   ├── LauncherState.cs
│   │   ├── QueryParser.cs
│   │   └── ResultSelectionState.cs
│   ├── Bridge/
│   │   ├── FfiBindings.cs
│   │   ├── EngineBridge.cs
│   │   └── BridgeModels.cs
│   ├── Commands/
│   │   ├── CalcCommand.cs
│   │   ├── ShellCommand.cs
│   │   ├── KillCommand.cs
│   │   └── SysCommand.cs
│   ├── Features/
│   │   ├── Clipboard/
│   │   │   ├── ClipboardHistoryStore.cs
│   │   │   └── ClipboardQuery.cs
│   │   ├── HotKey/
│   │   │   ├── GlobalHotKeyManager.cs
│   │   │   └── HotKeySettings.cs
│   │   ├── Search/
│   │   │   ├── LauncherSearchLogic.cs
│   │   │   └── ResultDedupe.cs
│   │   └── Window/
│   │       ├── WindowLifecycle.cs
│   │       └── FocusTracker.cs
│   ├── Services/
│   │   ├── ActionDispatcher.cs
│   │   ├── ShellExecuteService.cs
│   │   ├── ExplorerRevealService.cs
│   │   ├── StartupRegistrationService.cs
│   │   └── ProcessService.cs
│   ├── Theme/
│   │   ├── ThemeSettings.cs
│   │   ├── ThemeStore.cs
│   │   └── Typography.cs
│   ├── Views/
│   │   ├── LauncherWindow.xaml
│   │   ├── LauncherWindow.xaml.cs
│   │   ├── LauncherRowView.xaml
│   │   ├── ResultPreviewView.xaml
│   │   ├── CommandPanels/
│   │   └── Settings/
│   └── Tests/
│       ├── LauncherSearchLogicTests.cs
│       └── QueryParserTests.cs
└── packaging/
    ├── msix/
    └── wix/
```

Shell responsibilities:

- global hotkey registration and launcher toggle
- launcher window lifecycle and keyboard-first interaction
- local action dispatch (open, reveal, copy, web handoff)
- command mode (`calc`, `shell`, `kill`, `sys`)
- clipboard history mode (`c"`)
- theme/settings UI

FFI boundary from Windows shell:

- `look_search_json_compact`
- `look_record_usage_json`
- `look_reload_config`
- `look_translate_json`
- `look_free_cstring`

## Phase 0 - Define parity and constraints

1. Create a Windows parity checklist from current user-visible behavior in `README.md` and `docs/user-guide.md`.
2. Classify features as:
   - parity required for v1 Windows
   - can ship in later Windows patch release
3. Freeze behavior contracts for:
   - query prefixes (`a"`, `f"`, `d"`, `r"`, `c"`)
   - keyboard shortcuts
   - result action semantics (`Enter`, reveal, copy, web handoff)
4. Define performance budgets for Windows release candidate:
   - launcher open latency
   - query p50/p95 latency
   - idle CPU/memory envelope

Exit criteria:

- written parity checklist committed in docs
- agreed v1 scope for Windows shell

Current status:

- parity checklist drafted in `docs/windows-v1-parity-checklist.md`
- next action: convert checklist entries into automated tests as platform modules land

## Phase 1 - Platform abstraction in Rust engine

1. Isolate macOS-specific indexing logic behind platform adapters in `core/engine/src/index/`.
2. Introduce platform-dispatched modules:
   - app discovery (macOS and Windows variants)
   - settings catalog/discovery (macOS and Windows variants)
   - path normalization helpers (separator and case behavior)
3. Keep search/matching/ranking/storage platform-agnostic.
4. Add tests for adapter selection and stable candidate IDs across platforms.

Key files to refactor first:

- `core/engine/src/index/apps.rs`
- `core/engine/src/index/settings.rs`
- `core/engine/src/config.rs`

Exit criteria:

- engine builds/tests pass with platform-specific index adapters
- no macOS-only assumptions outside platform adapter modules

Current status:

- app discovery split into platform modules (`platform/macos/apps.rs`, `platform/windows/apps.rs`)
- settings catalog split into platform modules (`platform/macos/settings_catalog.rs`, `platform/windows/settings_catalog.rs`)
- `index/apps.rs` now dispatch-only; ranking/search core remains shared

### Rust code change map (detailed)

Proposed structure:

```text
core/engine/src/
├── platform/
│   ├── mod.rs
│   ├── macos/
│   │   ├── apps.rs
│   │   ├── settings.rs
│   │   └── paths.rs
│   └── windows/
│       ├── apps.rs
│       ├── settings.rs
│       └── paths.rs
└── index/
    ├── apps.rs
    ├── settings.rs
    └── files.rs
```

File-by-file plan:

1. `core/engine/src/index/apps.rs`
   - keep `discover_installed_apps(config, tx)` signature
   - dispatch by target platform (`cfg(target_os = "macos"|"windows")`)
   - move macOS `.app` scanning into `platform/macos/apps.rs`
   - add Windows Start Menu + install roots discovery in `platform/windows/apps.rs`

2. `core/engine/src/index/settings.rs`
   - keep `discover_system_settings_entries(tx)` signature
   - move Apple catalog into `platform/macos/settings.rs`
   - add curated `ms-settings:` catalog in `platform/windows/settings.rs`
   - keep candidate id/kind conventions stable (`setting:*`, `CandidateKind::App`)

3. `core/engine/src/config.rs`
   - replace hardcoded platform roots with helper builders
   - add `default_app_scan_roots()` and platform-aware `default_file_scan_roots()`
   - keep existing config keys and parsing semantics
   - update path expansion logic to support Windows absolute paths

4. `core/engine/src/index/files.rs`
   - centralize platform-aware path normalization
   - preserve boundary-aware exclude-path checks across separators/casing
   - keep `ignore::WalkBuilder` traversal and candidate model unchanged

5. `bridge/ffi/src/lib.rs` and `bridge/ffi/*`
   - keep exported symbols and JSON payload contracts stable
   - add Windows CI checks/smoke tests for search, usage, config reload, translate

Rust rollout sequence:

1. Introduce platform module scaffolding with macOS pass-through behavior.
2. Refactor config defaults to platform helper builders.
3. Refactor `index/apps.rs` and `index/settings.rs` into platform dispatch.
4. Add Windows app/settings implementations behind `cfg(target_os = "windows")`.
5. Add Windows-focused tests and CI coverage.

Rust completion criteria:

- Rust workspace builds/tests on macOS and Windows
- no FFI ABI break
- macOS behavior preserved while enabling Windows adapters

## Phase 2 - Windows indexing sources

1. Implement Windows app discovery:
   - Start Menu shortcut locations (per-user and machine)
   - common install roots as fallback
2. Implement Windows settings discovery/catalog:
   - curated `ms-settings:` entries for high-value settings pages
3. Implement Windows file root defaults for config bootstrap:
   - Desktop, Documents, Downloads with Windows path handling
4. Ensure exclude-path and skip-dir behavior matches existing config semantics.
5. Validate candidate quality and dedupe behavior against parity checklist.

Exit criteria:

- index produces high-quality app/settings/file candidates on Windows
- IDs and kinds remain compatible with existing ranking/storage model

Current status:

- Windows app discovery implemented with Start Menu-first scan and lightweight fallback roots
- curated `ms-settings:` catalog implemented with stable `setting:*` candidate IDs
- adapter-level Windows unit tests added for entry detection/filtering/dedupe and catalog integrity

## Phase 3 - FFI hardening for multi-shell support

1. Keep existing exported API in `bridge/ffi/src/lib.rs` stable.
2. Audit FFI payloads to ensure shell-agnostic data contracts.
3. Add Windows-focused FFI smoke tests (search, usage record, config reload, translate).
4. Validate allocator and string lifetime safety under Windows runtime.

Exit criteria:

- FFI crate compiles and tests on Windows CI
- no shell-specific assumptions in FFI JSON models

Current status:

- FFI exported symbol set unchanged (`look_search_json_compact`, `look_record_usage_json`, `look_reload_config`, `look_request_index_refresh`, `look_translate_json`, `look_free_cstring`)
- ffi smoke coverage expanded to include reload/refresh flow and translate error payload contracts
- CI matrix already runs `bridge/ffi` build/tests on `windows-latest` and `macos-latest`
- current non-Windows fallback paths remain macOS-shaped defaults; when Linux support is added, add an explicit Linux branch before fallback

## Phase 4 - Windows native shell scaffold (WinUI 3)

1. Create Windows app shell directory:
   - `apps/windows/LauncherApp/` (proposed)
2. Build first runnable shell with:
   - launcher window
   - query input
   - result list rendering
   - selected-row highlight and keyboard navigation
3. Load data from FFI search endpoint and render candidate rows.
4. Port theme primitives to preserve visual identity while following Windows conventions.

Exit criteria:

- Windows shell can open, query via FFI, and display interactive results

## Phase 5 - Action parity and OS integration

1. Implement global hotkey toggle (Windows equivalent of `Cmd+Space`).
2. Implement result actions:
   - open app/file/folder/settings target
   - reveal in Explorer
   - copy path/content
3. Implement clipboard history mode with robust capture strategy.
4. Implement command mode actions:
   - `calc`
   - `shell`
   - `kill`
   - `sys`
5. Implement startup behavior (launch at login) for Windows.

Exit criteria:

- all v1 parity-required actions work from keyboard-only flow

## Phase 6 - UX parity polish

1. Match interaction details from macOS shell:
   - focus management
   - hide-on-focus-loss behavior
   - selection reset and command transitions
2. Match error and fallback messaging to existing user-facing patterns.
3. Validate visual parity for:
   - row layout and metadata readability
   - preview/dictionary panel behavior
   - settings/theme controls that are platform-appropriate
4. Run side-by-side parity QA using the checklist from Phase 0.

Exit criteria:

- parity checklist passes for all required behaviors
- no major UX regressions vs macOS baseline

## Phase 7 - Packaging, signing, and release pipeline

1. Choose packaging target (`.msix` or `.msi`) and document installer behavior.
2. Add Windows build jobs to CI:
   - Rust workspace checks
   - FFI checks/tests
   - Windows shell build
3. Add release artifact generation and checksums.
4. Add code signing/notarization equivalent for Windows release trust.
5. Add Windows installation and update instructions to docs.

Exit criteria:

- repeatable signed Windows release artifacts produced by CI

## Phase 8 - Beta rollout and stabilization

1. Ship private beta builds to a small tester group.
2. Collect telemetry and feedback (crash, latency, relevance misses).
3. Fix top reliability and performance issues.
4. Gate general availability on:
   - stability targets met
   - parity checklist pass rate
   - acceptable performance budgets

Exit criteria:

- Windows version ready for public release with clear support scope

## Work breakdown by repository area

- `apps/windows/` (new): Windows shell UI, hotkey, action dispatch, settings UI
- `core/engine/`: platform adapters for indexing + config defaults
- `bridge/ffi/`: stable ABI for both macOS and Windows shells
- `docs/`: platform-specific install guide, keymap notes, known limitations
- `.github/workflows/`: Windows CI build/test/release jobs

## Risk list and mitigations

- Global hotkey conflicts on Windows
  - Mitigation: configurable hotkey + conflict messaging
- App discovery noise from shortcut targets
  - Mitigation: canonicalization + filtering + dedupe rules
- Clipboard capture edge cases
  - Mitigation: listener-first approach with bounded polling fallback
- UI drift between macOS and Windows
  - Mitigation: explicit parity checklist and side-by-side QA pass
- Packaging/signing friction
  - Mitigation: automate release pipeline early (before beta)

## Suggested delivery milestones

- M1: Rust platform adapters + Windows indexers
- M2: Windows shell scaffold + FFI search integration
- M3: Full action parity + command mode parity
- M4: Packaging/signing + closed beta
- M5: Public Windows release
