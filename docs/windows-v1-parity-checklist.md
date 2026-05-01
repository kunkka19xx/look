# Windows v1 Parity Checklist

This checklist defines the required user-visible behavior for the first Windows release.
Scope is derived from current macOS behavior in `README.md` and `docs/user-guide.md`.

## 1) Release scope split

Parity required for Windows v1:

- global hotkey launcher toggle and keyboard-first flow
- app/file/folder search from indexed local sources
- query prefixes: `a"`, `f"`, `d"`, `r"`, `c"`
- core actions: open target (`Enter`), reveal in Explorer, copy selected path/content, web handoff (`Cmd/Ctrl+Enter` equivalent)
- command mode: `calc`, `shell`, `kill`, `sys` (note: `pomo` is macOS-only at v1 — see "Can ship after Windows v1" below)
- clipboard history mode (`c"`) with session-local history
- config load/reload parity for indexing and ranking behavior
- stable candidate ID conventions (`app:*`, `file:*`, `folder:*`, `setting:*`)
- web translate mode (`t"`) with parallel VI/EN/JA fan-out, per-language copy, and "Open in Google Translate" handoff

Can ship after Windows v1 (patch release):

- dictionary lookup mode (`tw"`) — depends on a Windows-side equivalent of Apple's `DCSCopyTextDefinition` and is deferred until that's available
- complete visual/theme parity with every macOS preset variant
- advanced UX polish items that do not change core search/action semantics
- `pomo` command (Pomodoro focus timer) — depends on three macOS-specific frameworks that need Windows equivalents:
  - `AVFoundation` (`AVPlayer` / `AVPlayerItem`) — Windows uses `Windows.Media.Playback.MediaPlayer` from WinRT, or `MediaElement` (XAML) for the streaming-one-track-at-a-time playback model
  - `NSStatusItem` mini-timer in the menu bar — Windows uses a `NotifyIcon` (system tray icon) with optional balloon/popover
  - `UNUserNotifications` — Windows uses `Windows.UI.Notifications.ToastNotification`; macOS-only the foreground-delivery delegate is unnecessary
  - `NSSound` chime fallback — Windows uses `System.Media.SystemSounds` or a packaged `.wav`
  - Persistence (`pomo_sessions`, `pomo_timer_style`, `pomo_music_folder` keys in `.look.config`) is platform-neutral and can ship as-is on Windows once the playback/notification layers exist
  - Behavior contracts to preserve: editable session list with focus/break types + per-item duration; auto-advance with end-of-list looping; "ending soon" alert at 10s remaining; `Space` start/pause, `R` reset, `P` music toggle; 5-second standby fade with sidebar collapse on idle

## 2) Behavior contracts (must not drift)

### Query behavior

- `a"term` filters to apps only
- `f"term` filters to files only
- `d"term` filters to folders only
- `r"pattern` enables regex search
- `c"term` switches to clipboard history search space
- `t"text` switches to web translate mode; translation only fires on `Enter` (not per-keystroke), three sections render parallel VI/EN/JA results, per-section copy button, and an "Open in Google Translate" handoff button
- non-prefixed query keeps blended ranking behavior

### Action semantics

- `Enter`: execute selected result action (in `t"` mode: trigger translation)
- web handoff: open browser search URL using current query (`Ctrl+Enter`)
- reveal action opens parent location and selects target in Explorer (`Ctrl+F`, matches macOS `Cmd+F`)
- copy action writes selected path/content to clipboard (`Ctrl+C`); for app/file/folder rows, the clipboard payload should be a real file reference so Ctrl+V in Explorer pastes the file (not just a path string) — parity with macOS `pasteboard.writeObjects([NSURL, NSString])`
- multi-selection: Shift+Up/Down and Shift+Tab extend selection; Ctrl+C / Ctrl+F / Enter operate on the full selection — parity with macOS multi-pick (commit `74b619c`)
- clipboard history must skip non-text entries (file references, images) so file-copy-in-Explorer does not pollute history with synthesized path text — parity with macOS `pasteboardCarriesFileReference`

### Keyboard model

- selection navigation via `Up`/`Down` and `Tab`/`Shift+Tab`
- mode transitions preserve keyboard-only flow (search <-> command <-> clipboard)
- hide/close behavior mirrors launcher expectations on focus loss and explicit dismiss

## 3) Windows-specific mapping notes

- system settings candidates must use `ms-settings:` targets with `setting:*` IDs
- file path handling must support separators (`/` and `\\`) and case-insensitive comparisons where appropriate
- app discovery should prioritize Start Menu entries, with install-root fallback and dedupe

## 4) Performance budgets for Windows release candidate

The project priority is speed. Windows work is accepted only if these targets remain healthy.

- launcher open latency (hot): p50 <= 120ms, p95 <= 180ms
- query latency (local index): p50 <= 35ms, p95 <= 90ms
- startup to first usable result: <= 900ms
- idle CPU: <= 2%
- idle memory envelope: <= 260MB

Notes:

- measure on representative non-dev hardware and real user datasets
- fail parity QA if p95 query latency regresses >10% week-over-week

## 5) Validation checklist

- run fixed smoke query set for apps/files/folders/settings and check top-5 relevance
- validate each query prefix contract with unit/integration tests
- validate result action behavior from keyboard-only flow
- verify duplicate-candidate suppression across app discovery sources
- verify no FFI ABI breaks for `look_search_json_compact`, `look_record_usage_json`, `look_reload_config`, `look_translate_json`, `look_free_cstring`

### Command mode calc parity smoke cases

- `/calc 2+3*4` -> `Result: 14.0000`
- `/calc (2+3)*4` -> `Result: 20.0000`
- `/calc 10%3` -> `Result: 1.0000`
- `/calc 2^8` -> `Result: 256.0000`
- `/calc 3!` -> `Result: 6.0000`
- `/calc 50%` -> `Result: 0.5000`
- `/calc 200+10%` -> `Result: 220.0000`
- `/calc 200-10%` -> `Result: 180.0000`
- `/calc v9` -> `Result: 3.0000`
- `/calc sqrt(16+9)` -> `Result: 5.0000`
- `/calc abs(-5.2)` -> `Result: 5.2000`
- `/calc floor(3.9)` -> `Result: 3.0000`
- `/calc ceil(3.1)` -> `Result: 4.0000`
- `/calc round(2.6)` -> `Result: 3.0000`
- `/calc pi*2` -> `Result: 6.2832`
- `/calc 10:4` -> `Result: 2.5000`
- `/calc 12x3` -> `Result: 36.0000`
- `/calc 1/0` -> `Error: division by zero`
- `/calc (2+3` -> `Invalid expression`
- `/calc 9999999999999*9` -> `Error: result out of range (+/-1,000,000,000,000)`
