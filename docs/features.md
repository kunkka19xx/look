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
- scoped query prefixes: `a"`, `f"`, `d"`, `r"`, and `rc"` (recent files/folders, newest first - blends opened-through-Look with recently added/changed on disk; macOS for now)
- path-fragment friendly matching (slash-biased queries)
- URL-like queries (no prefix): typing a URL offers an **Open in browser** row (structural URLs rank first, a bare `host.tld` after local results); opened URLs return as frecency-ranked **Recently opened** rows
- arithmetic queries (no prefix): typing an expression (`2+2`, `sqrt(16)`, `200*15%`) pins a **Calculator** row above every other result; shape decides whether something counts as math, not spacing, so a date, a resolution, or a ratio (`20-05-2026`, `1920x1080`, `16:9`) is left alone. `Enter` or a click copies the value and hides the launcher; clipboard history shows the worked expression (`2+2 = 4`) but still pastes just the value. Shared `core/calc` engine on macOS, Linux, and Windows
- open with `Enter`, reveal in Finder with `Cmd+F`
- copy selected file/folder path/content handle with `Cmd+C`
- multi-pick files/folders with `Cmd+P` (toggle); picked set is mirrored to the system pasteboard for paste anywhere. `Cmd+Shift+P` clears the set
- move selected file/folder (or all picked items) to the Trash with `Cmd+D` - recoverable, no confirmation (macOS only for now)
- pinned **Trash** quick folder (type `trash`): `Enter` opens it in Finder, its preview shows the item count, and `Cmd+D` empties it via Finder (confirmed, since it's permanent)
- preview pane: text/image file previews, plus folder previews listing the immediate children (folders first, capped at 30, click to open)
- hide the selected app from Look with `Cmd+Shift+H` / `Ctrl+Shift+H` so it stops appearing in results
- run the selected app as administrator with `Ctrl+Shift+Enter` (Windows only, triggers a UAC prompt)

### Clipboard and translation

- clipboard history mode with `c"` prefix
- in-memory clipboard history (recent text clips, size set by `clipboard_history_limit`, default 10, range 10 to 100); file/folder copies are excluded
- remove the selected clipboard history item with `Cmd+D` / `Ctrl+D`
- quick translation with `t"...`
- dictionary lookup panel with `tw"...`

### AI answers and web suggestions (macOS, Linux, Windows)

- optional, **on by default**; toggle with `ai_enabled` in `~/.look.config` or the Settings panel
- **answer card**: a question, an entity with no local match (e.g. `sir alex ferguson`), or an instant-answer pattern (weather/currency/crypto) shows a Spotlight-style card. Sources resolve concurrently and render as they arrive - DuckDuckGo, then Wikipedia (arithmetic no longer answers here; it has its own pinned row above the results - see Core search and launch). On macOS it falls back to a streaming on-device **Apple Intelligence** answer when no web source hits. In the knowledge-lookup view the card sits in a two-column layout with the suggestion list
- **search suggestions**: Google autocomplete rows appear under the results for plain text queries (2+ chars); `Enter` on one runs a web search, as does `Cmd+Enter` on the query
- **model interpretation** *(macOS)*: when a natural-language query finds nothing locally, the model turns the phrasing into a *structured* file query (type, time, place) and re-searches - never a rewritten query string, never overriding results already on screen. Results say when they were interpreted, or when the search had to be relaxed to find near-misses
- **personal answers**: schedule questions ("what's on my calendar this week?") answer from your calendar itself - deterministic, instant, and never sent to a web source
- **platform note**: the web answer card and Google suggestions run on macOS, Linux, and Windows (the latter two via the Tauri `apps/linows/` app), sharing the `look-answers` core engine. The on-device LLM (query rewrite + the Apple Intelligence answer fallback) is **macOS-only** - there is no on-device model on Linux/Windows. The `ai_enabled` key is shared so the same toggle gates web answers on Linux/Windows and both web + on-device features on macOS
- network note: while AI is on, the answer card's web sources and the Google suggestions send the typed query to those services; the on-device model itself makes no network calls. All of it is off when `ai_enabled = false`

### AI actions and chat (macOS)

- **the `>` session**: type `>` to switch the panel into a conversation - actions, questions, and streamed answers stack together. `Esc` leaves, `Cmd+Z` undoes, `Cmd+.` stops a generation without ending the session. Past conversations are listed, searchable, and resumable
- **calendar and reminders**: add, move, cancel, complete, remove, snooze, and block focus time, in plain language ("move my dentist to friday"). Every change previews first and confirms with `Enter`, then `Cmd+Z` undoes it. Adding an event that already sits on that day says so instead of quietly duplicating it
- **`@` for exact times**: `>add lunch @ 1pm` skips the model entirely - instant, deterministic, and works with no capable model configured
- **join a meeting**: typing `join` pins a "Join <meeting>" row for the next Teams, Zoom, Google Meet, Webex, Jitsi, GoToMeeting, or Whereby meeting on your calendar, and `Enter` opens it. Deterministic and model-free: the invite already carries its join link, so Look reads it out of the event rather than calling any API. A meeting already under way beats one starting sooner. Online accounts work through macOS Calendar (System Settings > Internet Accounts); Look itself makes no network call
- **no prefix needed**: typing an instruction in the main bar works too. The plan appears as the first result row and one `Enter` runs it
- **file recall**: "pdfs from last week", "files added to desktop" search your index by type, time, and place
- **text-ops**: "summarize", "translate to german", "make this shorter" transform whatever you copied. Pick a file first (`Cmd+P`), or `@`-mention one while typing, and they transform that file instead. Text files, source code, and PDFs; an oversized file says how much of it was read rather than quietly summarizing the first part. A PDF that is a scan, is password-protected, or decodes to junk is refused by name - summarizing garbage would read exactly like a real answer
- **remembered facts**: "remember I prefer metric" stores a durable fact the assistant sees on every turn. Only you can write these - the model never can
- **requires a capable provider** for the natural-language paths: Ollama with a pulled model (Settings > AI), local by default but usable against a remote host. Apple Intelligence handles answers but is too small to plan actions; the `@` forms keep working regardless
- **privacy**: prompts go to whichever provider you select, so a remote host or cloud-routed model receives them over the network. Your calendar, clipboard, and remembered facts are held to a stricter rule: attached only when inference is on this machine, unless you explicitly allow remote context in Settings

### Command mode

- `Cmd+/` command mode entry, or inline `:cmdid` shortcut from the home screen (e.g. `:calc 2+2`, `:kill chrome`, `:pomo`); space after a known command id triggers a live switch with args pre-filled
- built-in commands: `calc`, `pomo`, `todo`, `speed`, `kill`, `shell`, `sys`
- `pomo`: pomodoro focus timer with editable session list, three timer styles (Modern Ring / Vintage Dial / Minimal Text), shuffled background-music folder, menu-bar mini-timer, 5s standby fade, "ending soon" alert at 10s remaining
- `todo`: daily tasks grouped by date (3 unfinished per day, 3 upcoming groups, past days stay non-editable, unfinished tasks 1-3 days late show an `EXTENDED` badge and can still be completed, tasks more than 3 days late show `OVERDUE`, fuzzy search over tasks and dates, manual save) plus a Stats page: weekly/monthly completion donuts, streak, 30-day trend, GitHub-style year heatmap. Today's done/total shows as a clickable stat in the home hint bar. Stored in the shared `look.db` (`core/todo`), one-year retention
- `speed`: internet speed test on a live dial - download and upload as counter-rotating comets on a log scale (1 Mbps to 1 Gbps), latency at the centre pulsing once per round trip, plus LAN/public addresses (public masked by default, both click-to-copy), ISP, location, and a plain-language read of the result. Measurement is shared (`core/netspeed`): a latency probe plus four parallel curl streams per direction against Cloudflare's keyless endpoints, falling back to the nearest of several public test mirrors when Cloudflare rate-limits the connection. Runs on open (reusing a reading under a minute old) and on `R`, never on a timer
- calc parser (`core/calc`, shared by every shell) supports exponent (`^`), factorial (`!`), constants (`pi`, `e`), math functions (`sqrt`, `abs`, `round`, `floor`, `ceil`), `%` shorthand while keeping modulo, implicit multiplication (`2pi`, `3sqrt(9)`), comma-grouped and scientific-notation input (`1,500`, `1e6`), and aliases `x`/`:`/leading `v` (multiply, divide, `sqrt`) honored wherever they land inside `/calc` (`1920x1080`, `16:9`) - results are limited only by what an `f64` can represent, not an artificial ceiling
- kill flow with explicit confirmation and process-by-port lookup (`:3000` / `port 3000`)
- warning cue when shell input contains `sudo`

### Running apps switcher

- an icon row rendered on the right half of the search bar: when enabled, the search field takes the left half and the running-app icons occupy the right (right-aligned, growing leftward as more apps open). Apps are capped at 9, sorted alphabetically and **stable** - positions don't shuffle when you switch apps
  - **macOS**: from `NSWorkspace.shared.runningApplications`, filtered to regular apps
  - **Linux**: from `/proc` scan, filtered by what GNOME Shell's `Shell.AppSystem.get_running()` considers a windowed app (via Look's GNOME extension on Wayland) or by `wlr-foreign-toplevel` / X11 client-list / desktop-hints on other compositors
  - **Windows**: from running-window enumeration via Win32
- on the home screen, activation: `Cmd`+badge digit (macOS) / `Alt`+badge digit (Linux, Windows). In command mode, `Cmd+1`..`Cmd+7` / `Ctrl+1`..`Ctrl+7` keep their existing command-catalog semantics
- badge labels follow an ergonomic outer-first layout: with N running apps we consume the easiest-to-reach keys first (`1, 2, 3, 9, 8` before `4`, then `7`, then `6`, then `5`). 5 running apps → badges `1, 2, 3, 8, 9`; 9 running apps → all of `1`..`9`
- focus paths: macOS = `NSRunningApplication.activate()` with Dock-style reopen for windowless apps; Linux = GNOME Shell extension D-Bus on GNOME Wayland, `wlr-foreign-toplevel-management` on sway/Hyprland, niri IPC on niri (focuses and scrolls to the window's workspace), `i3-msg` on i3, `_NET_ACTIVE_WINDOW` (x11rb) on other X11 WMs; Windows = `SetForegroundWindow` via window handle
- click on an icon also switches; hover shows app name + shortcut tooltip; active app has an accent ring
- toggled on/off via `Settings > Appearance > Running Apps`. Persisted as `running_apps_placement` in `~/.look.config` (`none` = off, any other value = on; legacy `top`/`right`/`bottom` still load as "on"). The window is a single fixed size and never resizes for the row
- off hides the row and disables the activation shortcut
- AI mode (`>`) hides the row too, and hands `Cmd+1`..`Cmd+9` plus `Cmd+0` to the conversation list: the digit opens the session carrying that chip. Ten chips is the ceiling (a `Cmd` chord is one keypress), so the list shows ten and older sessions are found by typing

### Super actions

- a control strip on the empty home screen (no query typed) with system toggles, one-shot actions and read-only info tiles
- the tile set, order, sizes and mnemonics come from the shared `core/qactions` catalog, so macOS, Linux and Windows render the same strip; only the native state reads and control paths differ. On Linux the strip works across GNOME, KDE and tiling WMs including i3
- tiles: L slot (Pomodoro session > remaining todos > clock), Bluetooth, Wi-Fi, Battery, Theme, Keep Awake, Screensaver, Weather, Mic, Restart, Shut Down, Now Playing
- activation: click a tile, or press the platform modifier + its highlighted letter - `Cmd` (macOS) / `Alt` (Linux, Windows): `B` Bluetooth, `W` Wi-Fi, `T` Theme, `K` Keep Awake, `S` Screensaver, `M` Mic, `R` Restart, `D` Shut Down, `P` Now Playing play/pause
- Restart and Shut Down arm on the first press and fire on the second; `Esc` (macOS) or the auto-disarm timeout cancels
- Battery, Weather and the L slot are read-only
- toggled on/off via `Settings > Appearance > Super Actions`. Persisted as `super_actions_enabled` in `~/.look.config`
- off hides the strip and disables its mnemonics

### Settings and runtime config

- in-app settings panel (`Cmd+Shift+,`)
- local config file `~/.look.config`
- runtime reload (`Cmd+Shift+;`)
- 9 built-in theme presets (Catppuccin, Tokyo Night, Rose Pine, Gruvbox, Dracula, Kanagawa, Kindle, Liquid, Custom)
- Behind-window blur requested from the compositor where it exists (macOS material; KDE / Hyprland / Niri on Linux), clear glass everywhere else
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
- deeper per-desktop-environment polish across Linux compositors

## Out of scope for v1

- cloud-first workflows
- semantic/vector retrieval
- full content indexing of file bodies
- mandatory plugin ecosystem for core workflow
