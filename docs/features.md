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

### Clipboard and translation

- clipboard history mode with `c"` prefix
- in-memory clipboard history (latest text clips)
- quick translation with `t"...`
- dictionary lookup panel with `tw"...`

### Command mode

- `Cmd+/` command mode entry
- built-in commands: `calc`, `shell`, `kill`, `sys`
- kill flow with explicit confirmation and process-by-port lookup (`:3000` / `port 3000`)
- warning cue when shell input contains `sudo`

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
