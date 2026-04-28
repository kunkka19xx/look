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

## Recent implementation snapshot (2026-04)

- Windows search now uses real Rust FFI results in the WinUI shell.
- Windows app discovery noise filtering and dedupe were tightened:
  - Start Menu + fallback root scanning with helper executable filtering
  - overlap dedupe across Start Menu/WindowsApps/System32 where safe
- Icon pipeline was stabilized for WinUI rendering:
  - shell extraction path for `.exe`/`.lnk`/folder/file
  - cached bitmap fallback for reliable row rendering
  - settings icon mapping for `ms-settings:` entries
- Windows Settings catalog was expanded with broader `ms-settings:` coverage.
- Action dispatch now performs real typed open behavior for app/file/folder/setting/url,
  with reveal/copy/web handoff wired in the shell.
- Command mode now executes real handlers on Windows for `calc`, `shell`, `kill`, and `sys`,
  with command output rendered inside the command panel.
- Windows app project now auto-builds Rust FFI and copies `look_ffi.dll` during build.

## Translate + reusable UI primitives snapshot (2026-04)

Web translate mode (`t"`) shipped — matches macOS Cmd-driven flow:

- `Bridge/FfiBindings.cs`: `look_translate_json(text, targetLang)` P/Invoke and `EngineBridge.Translate(text, lang)` UTF-8 marshal wrapper that frees the cstring symmetrically.
- `Services/TranslationService.cs`: parallel `Task.Run` fan-out across `vi`/`en`/`ja` with `CancellationToken` propagation, mirrors macOS `LauncherTranslationService.fetchNetworkTranslations`.
- `MainWindow.Search.cs`: new `LauncherMode.Translate` branch in `ResolveMode`. Typing only updates the panel header ("Press Enter to translate"); translation fires only on Enter — matches macOS hint "Press Enter after finishing input to translate on web". Version-token + per-fan-out CTS cancellation prevents stale paint after the user backs out of translate mode.
- `Views/TranslatePanelView.xaml`: three `TranslateLanguageSectionView` instances (Tiếng Việt / English / 日本語) plus a footer "Open in Google Translate" button that opens `https://translate.google.com/?sl=auto&tl=en&text=<query>&op=translate` via `ActionDispatcher.OpenUrl`. Each section has a per-language copy affordance that bubbles `CopyTranslatedRequested` up to MainWindow which writes to clipboard and shows the standard banner.
- `bridge/ffi/src/translate_api.rs`: added `CREATE_NO_WINDOW` (0x08000000) creation flag on the `curl` `Command` under `#[cfg(target_os = "windows")]`, so the GUI shell doesn't flash a console window for each translate fan-out call.
- `tw"` (Apple Dictionary lookup) is intentionally not implemented — Windows has no `DCSCopyTextDefinition` equivalent; the parity checklist now classifies it as deferred.

Reusable XAML primitives (extraction pass on duplicated layouts):

- `Views/KeyChordRowView.xaml(.cs)`: `KeyText` + `Description` + `KeyColumnWidth` + `IsPill`. Replaced ~30 inline rows in `HelpScreenView.xaml` (–161 lines) and `Settings/Tabs/ShortcutsSettingsTabView.xaml` (–181 lines).
- `Views/LabeledSliderView.xaml(.cs)`: wraps a `Slider` with `Label` + `LabelColumnWidth` + `Minimum/Maximum/StepFrequency` + pass-through `Value` and a `RangeBaseValueChangedEventHandler ValueChanged` event. Replaced 15 inline grid blocks in `AppearanceSettingsTabView.xaml` (–155 lines) and 2 in `AdvancedSettingsTabView.xaml` (–18 lines). `SetColorSliders` signature updated from `Slider` → `LabeledSliderView`; everything else compiles unchanged because `Value` and `ValueChanged` are pass-throughs.
- `Views/CommandPanels/CommandCardView.cs`: subclasses `ToggleButton` so existing `card == ShellCard`, `Card.IsChecked`, `sender is ToggleButton` keep working; adds `Title` + `Subtitle` DPs. Four card declarations in `CommandPanelsView.xaml` collapsed from 7 lines each to 1 line.
- `Views/TranslateLanguageSectionView.xaml(.cs)`: extracted from `TranslatePanelView` so the three language blocks aren't duplicated.

Shortcut alignment with macOS:

- Reveal-in-Explorer rebound from `Ctrl+R` to `Ctrl+F` to match macOS `Cmd+F`. Handler moved to `GlobalKeyDown` (was on `ResultsList_OnKeyDown`) so it fires while typing in the search box, not just when the list view has focus. Same fix applied to `Ctrl+C` copy. Hint text, help screen, and shortcuts settings all updated.

## Launcher polish + UWP parity snapshot (2026-Q2)

Search pipeline:

- search now runs async on `Task.Run` with version-based cancellation, keeping the UI
  thread free during FFI round-trips; debounce bumped from 8 ms to 16 ms
- removed `FileVersionInfo.GetVersionInfo` from the dedup hot path (was blocking the
  first search after launch); rank ties now fall through to FFI score
- junk-path filter added: `$RECYCLE.BIN`, `System Volume Information`, `$WINDOWS.~BT/~WS`,
  `Config.Msi`, `PerfLogs`, `Recovery`, `$GetCurrent`, `$SysReset`, `$INPLACE.~TR`,
  `\WindowsApps\` (the last one kills direct-to-package duplicates that the user can
  neither launch nor read icons from)

Icon pipeline:

- `IShellItemImageFactory` primary path for `shell:` URIs and `.lnk` stubs so UWP
  targets resolve to the real package logo (Weather/Photos/Terminal/Calculator)
- HBITMAP -> managed `Bitmap` conversion preserves alpha via manual DIB copy with
  un-premultiplication and topdown/bottomup handling, plus bounds/stride sanity checks
  and COM-release symmetry on cast-miss
- `.url` internet shortcut support via `IconFile` / `IconIndex` parsing
- icon cache relocated from `%TEMP%\look-icon-cache` to `%LOCALAPPDATA%\look\icon-cache`
  so it survives reboots; same dir now hosts the crash log too
- every previously-silent `catch {}` now logs via `Debug.WriteLine` with a context string

UWP discovery (`Services/UwpAppService.cs`):

- enumerates `shell:AppsFolder` via `Shell.Application` COM on a background `Task.Run`
- filters to AUMID entries only (`PackageFamilyName!AppId`), avoiding overlap with
  Rust's Start Menu indexer
- fuzzy title scoring merged with Rust results in `MergeBackendAndUwpResults`
- `DeduplicatePairedAppEntries` extended with `AppPathCategory.UwpAppsFolder` (rank 3,
  beats InstallExecutable/StartMenuShortcut on matching normalized title)
- `ShellExecuteService` routes any `shell:` target through `explorer.exe` (same pattern
  as `ms-settings:`), so launching a UWP entry invokes the packaged-app activation flow

Appearance parity (Windows <- macOS):

- six built-in theme presets ported: Catppuccin, Tokyo Night, Rose Pine, Gruvbox, Dracula,
  Kanagawa; per-preset tint/text/border/blur values wired into `AppearanceSettingsTabView`
- three-tier text system: `LauncherTextBrush` + new `LauncherSecondaryTextBrush` +
  `LauncherMutedTextBrush`, derived via macOS-equivalent `dimmableColor` (factor 0.82
  secondary / 0.64 muted, luminance-aware) unless the preset overrides with explicit tokens
- search input now inherits launcher colors via local `TextBox.Resources` overrides so
  text/placeholder/caret/selection stay in sync with Appearance sliders and presets

Background & blur rendering (`MainWindow.xaml.cs`):

- `<Image BackgroundImage>` layer rendered under `LauncherSurface`; `ApplyBackgroundImage`
  handles file-based `BitmapImage` for blur==0 and Win2D `CanvasImageSource` +
  `GaussianBlurEffect` for blur>0, with `CanvasBitmap` cached by path so slider drags only
  re-run the blur effect
- startup read of `~/.look.config` re-applies bg image + opacity + blur + mode so the
  setting survives restart
- `Microsoft.Graphics.Win2D` added to `LauncherApp.csproj`
- live preview wired in `AdvancedSettingsTabView` (slider ValueChanged, combo
  SelectionChanged, Choose/Clear button handlers)
- true adjustable backdrop Gaussian blur: composition `SpriteVisual` with
  `GaussianBlurEffect` layered over `CompositionBackdropBrush`; Appearance tab's
  `BlurOpacitySlider` + `SettingsBlurSlider` now drive a real pixel-radius via
  `UpdateBlurRadius` (mapped 0-60 px total) instead of only modulating tint alpha

Advanced settings:

- added Extra Scan Dirs UI (config key `file_scan_roots`) with macOS-style horizontal
  pill strip + inline `x` button per entry, single-row with horizontal scroll
- redundancy check on add: warns if the chosen folder is already covered by an existing
  scan root (home-relative resolved via `ResolveHomeRelativePath`)
- Skip Folders migrated to the same pill strip visual

Window polish:

- global hotkeys wired via `RegisterHotKey` + `SetWindowSubclass`: **Alt+Space** toggles
  the launcher (Hide/Show + `SetForegroundWindow` + focus QueryInput + SelectAll),
  **Alt+Shift+Q** quits via `Application.Current.Exit`; both use `MOD_NOREPEAT`
- window chrome: `SetBorderAndTitleBar(false, false)` removed the thin OS frame; DWM
  border color locked to `DWMWA_COLOR_NONE`
- `ResultsList_OnSelectionChanged` calls `ScrollIntoView` so Tab/Shift+Tab and
  Up/Down navigation past the viewport edge now scrolls to keep selection visible
- crash capture upgraded: log moved to `%LOCALAPPDATA%\look\look-crash.log`; added
  handlers for `AppDomain.CurrentDomain.UnhandledException` and
  `TaskScheduler.UnobservedTaskException` alongside the existing XAML handler, each entry
  tagged with origin (`UI` / `AppDomain` / `TaskScheduler`)

### Calc parity notes (Windows -> macOS follow-up)

Windows `calc` currently includes the following behavior that should be mirrored on macOS:

- numeric guardrail: result magnitude limit is `±1,000,000,000,000` (`1e12`)
- operators: `+`, `-`, `*`, `/`, `%`, `^`, unary `+/-`
- postfix operators: factorial (`!`), percent (`50% -> 0.5`)
- constants: `pi`, `e`
- functions: `sqrt`, `abs`, `round`, `floor`, `ceil`
- aliases: `x`/`X` as `*`, `:` as `/`, `v`/`V` prefix as `sqrt`
- percent semantics:
  - multiplication/division: `20 * 5% == 1`
  - finance style add/subtract: `200 + 10% == 220`, `200 - 10% == 180`

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
├── LauncherApp.slnx
├── LauncherApp.csproj
├── App.xaml
├── App.xaml.cs
├── MainWindow.xaml
├── MainWindow.xaml.cs
├── app.manifest
├── Assets/
├── Core/
│   ├── LauncherState.cs
│   ├── QueryParser.cs
│   └── ResultSelectionState.cs
├── Bridge/
│   ├── FfiBindings.cs
│   ├── EngineBridge.cs
│   └── BridgeModels.cs
├── Commands/
│   ├── CalcCommand.cs
│   ├── ShellCommand.cs
│   ├── KillCommand.cs
│   └── SysCommand.cs
├── Features/
│   ├── Clipboard/
│   │   ├── ClipboardHistoryStore.cs
│   │   └── ClipboardQuery.cs
│   ├── HotKey/
│   │   ├── GlobalHotKeyManager.cs
│   │   └── HotKeySettings.cs
│   ├── Search/
│   │   ├── LauncherSearchLogic.cs
│   │   └── ResultDedupe.cs
│   └── Window/
│       ├── WindowLifecycle.cs
│       └── FocusTracker.cs
├── Services/
│   ├── ActionDispatcher.cs
│   ├── ShellExecuteService.cs
│   ├── ExplorerRevealService.cs
│   ├── StartupRegistrationService.cs
│   └── ProcessService.cs
├── Theme/
│   ├── ThemeSettings.cs
│   ├── ThemeStore.cs
│   └── Typography.cs
├── Views/
│   ├── LauncherWindow.xaml
│   ├── LauncherWindow.xaml.cs
│   ├── LauncherRowView.xaml
│   ├── ResultPreviewView.xaml
│   ├── CommandPanels/
│   └── Settings/
├── Tests/
│   ├── LauncherSearchLogicTests.cs
│   └── QueryParserTests.cs
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
   - `apps/windows/LauncherApp/` (WinUI 3 with WinExe)
2. Build first runnable shell with:
   - launcher window
   - query input
   - result list rendering
   - selected-row highlight and keyboard navigation
3. Load data from FFI search endpoint and render candidate rows.
4. Port theme primitives to preserve visual identity while following Windows conventions.

Current status (FFI connected, command mode/action parity complete, UWP discovery live):

- shell scaffold in place at `apps/windows/LauncherApp/`
- FFI search IS connected and working (Rust backend returns real results)
- Completed UI components:
  - Launcher window with transparent/acrylic/mica backdrop + custom composition backdrop
    Gaussian blur driven by Appearance sliders
  - Borderless window chrome (`SetBorderAndTitleBar(false, false)` + DWM border color
    locked off)
  - Search results list with FFI-backed results, UWP apps merged in via
    `UwpAppService`, auto-scroll on keyboard navigation
  - Icon pipeline: `IShellItemImageFactory` for UWP/.lnk + alpha-preserving HBITMAP
    copy, `ExtractIconExW` / `SHGetFileInfoW` for the rest, `.url` support, PNG cache
    in `%LOCALAPPDATA%\look\icon-cache`
  - Command mode with 2-column card layout (calc, shell, kill, sys)
  - Settings screens: Appearance (six macOS presets + live tint/text/border/blur,
    three-tier semantic text brushes), Advanced (Extra Scan Dirs + Skip Folders pill
    strips with inline remove, live bg image render + opacity + blur), Shortcuts
  - Help screen
  - Keyboard navigation and shortcuts wired for real command interactions

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

**Phase 5 implementation status**:
- Global hotkey toggle: implemented
  - Alt+Space toggles visibility (`AppWindow.Hide`/`.Show` + `SetForegroundWindow` +
    focus QueryInput + SelectAll) via `RegisterHotKey` + `SetWindowSubclass` WndProc
    subclass; `MOD_NOREPEAT` prevents repeat fire on held keys
  - Alt+Shift+Q force-quits via `Application.Current.Exit` (with `Environment.Exit(0)`
    fallback)
  - hide-on-focus-loss auto-dismiss: implemented via `Window.Activated` -> `AppWindow.Hide`; file/folder pickers wrap work in `MainWindow.SuppressAutoHide()` (refcounted `IDisposable` scope) so the launcher doesn't disappear while a picker is open
  - hidden from taskbar and Alt-Tab via `WS_EX_TOOLWINDOW` (and cleared `WS_EX_APPWINDOW`) applied in `ConfigureLauncherWindow` before first `Activate`
- Result actions (open, reveal, copy, web handoff): implemented
- `shell:` URIs routed through `explorer.exe` so UWP `AppsFolder` targets launch cleanly
- Clipboard history mode (`c"`): implemented via `AddClipboardFormatListener` + `WM_CLIPBOARDUPDATE` subclass; entries deduped move-to-front, bounded (10 entries × 30K chars, matches macOS `maxEntries` / `maxStoredCharacters`), persisted to `%LOCALAPPDATA%\look\clipboard-history.json`; `Services/ClipboardHistoryService.cs` owns the listener and surfaces change events to `MainWindow` which rebuilds `LauncherMode.Clipboard` rows
- Command mode (`calc`, `shell`, `kill`, `sys`): implemented with in-panel execution and preview
  - `kill` parity: running-app list, keyboard/mouse selection, confirm bar, `:port` lookup (`:3000`, `port 3000`)
  - `calc` parity: advanced parser (`^`, `!`, `%`, constants/functions) plus finance-style percent for add/subtract
  - `sys`: grouped sections and live CPU/memory/network/battery/GPU/top-memory summary
- Launch at login: implemented via `Services/StartupRegistration.cs`, writing the `LookLauncher` value under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` with `"<exe>"`; synced at `App.OnLaunched` from the `launch_at_login` config key and again from `AdvancedSettingsTabView.SaveToConfig` when the toggle is changed. MSIX `StartupTask` is a follow-up once packaging lands.
- Web translate mode (`t"`): implemented via `Services/TranslationService.cs` (parallel `Task.Run` fan-out for vi/en/ja with `CancellationToken`) + `Views/TranslatePanelView.xaml` (three-section panel + footer "Open in Google Translate" button) + `Views/TranslateLanguageSectionView.xaml(.cs)` (per-language section with copy button). Translation fires only on Enter (matches macOS), `bridge/ffi/src/translate_api.rs` adds `CREATE_NO_WINDOW` flag on the `curl` `Command` under `cfg(target_os = "windows")` to suppress console flashes. `tw"` (Apple Dictionary lookup) is deferred — no Windows equivalent of `DCSCopyTextDefinition`.
- Reveal-in-Explorer shortcut rebound from `Ctrl+R` to `Ctrl+F` to match macOS `Cmd+F`. `Ctrl+F` and `Ctrl+C` reveal/copy handlers moved to `GlobalKeyDown` so they fire while typing (previously only worked when `ResultsList` had focus).

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

Implementation checklist for UI system parity (mock-first first, backend-agnostic):

- establish Windows design tokens that map to current macOS theme semantics (text, muted text, border, selection, background)
- standardize launcher row visuals (icon, title, meta, selected/hover/focus state, separators)
- define reusable button styles (`primary`, `secondary`, `ghost`, `danger`) with keyboard focus visuals
- define message/banner components (`success`, `info`, `warning`, `error`) and optional copy-action affordance
- declare mode-specific screens and states (search, command, clipboard, settings, help)
- define empty/loading/error states per mode with aligned user-facing copy tone
- define footer hint model and keymap presentation consistent with macOS launcher intent
- document parity references with screenshots and accepted deviations for Windows-native conventions

Current status:

- Windows shell now has mock-first mode screens for search, command, clipboard, settings, and help
- shared UI token/styles are defined in `App.xaml` (buttons, surfaces, banners, typography)
- keyboard hints and banner copy flow are wired for parity iteration before backend-final UX
- theme preset parity: Catppuccin / Tokyo Night / Rose Pine / Gruvbox / Dracula / Kanagawa
  ported from macOS with explicit tint/secondary/muted tokens + `dimmableColor`-style
  fallback for custom themes
- live background-image rendering + adjustable composition backdrop Gaussian blur
  (Win2D) gives Windows the same frosted-glass / bg-image experience as macOS
- search input inherits launcher palette via local `TextBox.Resources` overrides that
  alias to `LauncherTextBrush` / `LauncherMutedTextBrush`, so it re-colors with theme changes

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
