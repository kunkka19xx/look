# Feature Status

This document tracks what `look` supports today and what is planned next.

## Product pillars

- keyboard-first launcher UX
- low-latency local search
- practical ranking and personalization
- focused built-in tools (not plugin-first)
- predictable behavior with clear controls

## Available now

### Core search and launch

- app/file/folder search from one input
- scoped query prefixes: `a"`, `f"`, `d"`, `r"`
- path-fragment friendly matching (slash-biased queries)
- open with `Enter`, reveal in Finder with `Cmd+F`
- copy selected file/folder path/content handle with `Cmd+C`
- multi-pick files/folders with `Cmd+P` (toggle); picked set is mirrored to the system pasteboard for paste anywhere. `Cmd+Shift+P` clears the set
- preview pane: text/image file previews, plus folder previews listing the immediate children (folders first, capped at 30, click to open)

### Clipboard and translation

- clipboard history mode with `c"` prefix
- in-memory clipboard history (latest text clips); file/folder copies are excluded
- quick translation with `t"...`
- dictionary lookup panel with `tw"...`

### Command mode

- `Cmd+/` command mode entry, or inline `:cmdid` shortcut from the home screen (e.g. `:calc 2+2`, `:kill chrome`, `:pomo`); space after a known command id triggers a live switch with args pre-filled
- built-in commands: `calc`, `shell`, `kill`, `sys`, `pomo`
- `pomo`: pomodoro focus timer with editable session list, three timer styles (Modern Ring / Vintage Dial / Minimal Text), shuffled background-music folder, menu-bar mini-timer, 5s standby fade, "ending soon" alert at 10s remaining
- calc parser supports exponent (`^`), factorial (`!`), constants (`pi`, `e`), math functions (`sqrt`, `abs`, `round`, `floor`, `ceil`), and `%` shorthand while keeping modulo
- kill flow with explicit confirmation and process-by-port lookup (`:3000` / `port 3000`)
- warning cue when shell input contains `sudo`

### Running apps switcher (macOS)

- an icon row rendered **inside the search bar**: when enabled, the search field takes the left half of the top bar and the running-app icons occupy the right half (right-aligned, growing leftward as more apps open). Apps come from `NSWorkspace.shared.runningApplications`, filtered to regular apps, capped at 9, sorted alphabetically and **stable** — positions don't shuffle when you switch apps
- on the home screen, `Cmd`+the digit shown on an icon's corner badge activates it (in command mode, `Cmd+1`..`Cmd+5` keep their existing command-catalog semantics)
- badge labels follow an ergonomic outer-first layout: with N running apps we consume the easiest-to-reach keys first (`1, 2, 3, 9, 8` before `4`, then `7`, then `6`, then `5`). 5 running apps → badges `1, 2, 3, 8, 9`; 9 running apps → all of `1`..`9`
- windowless apps (e.g. Finder with no Finder windows open) get a fresh window via a Dock-style reopen instead of the bare activate flash; hidden apps (`Cmd+H`) are unhidden first
- click on an icon also switches; hover shows app name + shortcut tooltip; active app has an accent ring
- toggled on/off via `Settings > Appearance > Running Apps`. Persisted as `running_apps_placement` in `~/.look.config` (`none` = off, any other value = on; legacy `top`/`right`/`bottom` still load as "on"). The window is a single fixed size (860×600, screen-ratio scaled) and never resizes for the row
- off hides the row and disables the `Cmd+number` switching

### Settings and runtime config

- in-app settings panel (`Cmd+Shift+,`)
- local config file `~/.look.config`
- runtime reload (`Cmd+Shift+;`)
- 7 built-in theme presets (Catppuccin, Tokyo Night, Rose Pine, Gruvbox, Dracula, Kanagawa, Custom)
- query alias presets in `~/.look.config` for app + System Settings intent expansion (`alias_note`, `alias_code`, `alias_term`, `alias_chat`, `alias_music`, `alias_brow`)
- in-app config reset (`Settings > Advanced > Create Fresh Config`) with confirmation popup
- semantic color system with auto-derived text colors in Custom mode
- indexing, UI, privacy/logging, launch-at-login controls
- immediate validation feedback for invalid settings input
- advanced extra scan directory controls (`file_scan_extra_roots`) with overlap/risky-root validation

### Backend and persistence

- SQLite-backed candidate + usage storage
- startup/index refresh pipeline for apps/files/settings
- dirty-aware incremental indexing via file-system events (`Cmd+Space` refresh-on-dirty)
- usage-event feedback loop for ranking updates
- Rust core + FFI bridge to Swift app shell

## In progress / near-term

- running apps switcher parity on Linux + Windows (Tauri shell) — see `docs/tasks.md` "linows" section
- better coverage for deeper System Settings pages
- safer shell policy controls (more explicit execution guardrails)
- richer benchmark reporting (p50/p95/p99) for query/index paths
- tighter ranking calibration across title/subtitle/path signals

## Planned direction

- optional extension/plugin injection model (without bloating base UX)
- broader platform support after macOS quality stabilizes (Windows first)

## Out of scope for v1

- cloud-first workflows
- semantic/vector retrieval
- full content indexing of file bodies
- mandatory plugin ecosystem for core workflow
