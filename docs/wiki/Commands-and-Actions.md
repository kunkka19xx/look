# Commands & System Actions

Look provides two layers of command execution:
1. **Inline Quick Actions** (`:command` syntax) for instant system controls.
2. **Command Mode** (`Cmd+/` or `:cmdid`) for interactive built-in utility applications.

---

## 1. Inline Quick System Commands

Type a colon `:` followed by the command name to trigger instantaneous actions without opening menus:

* **`:sleep`**: Puts displays and the operating system into sleep mode immediately.
* **`:lock`**: Instantly locks the user session screen.
* **`:mute`**: Toggles system audio output between muted and unmuted states.
* **`:theme`**: Cycles system appearance dynamically between Dark and Light mode.
* **`:caff`**: Toggles macOS `caffeinate` background sleep prevention.

---

## 2. Command Mode (`Cmd+/`)

Press `Cmd+/` to enter the full-screen Command Mode catalog. You can also trigger a specific tool directly by typing its mnemonic (e.g. `:pomo`, `:calc 2+2`, `:kill 3000`).

### Built-in Applications

#### 🍅 Pomodoro Focus Timer (`:pomo`)
* **Focus Engine**: Configurable intervals (e.g. 25m focus, 5m break) with interactive audio alerts.
* **Timer Styles**: Switch between Modern Ring, Vintage Dial, and Minimalist Text dials.
* **Ambient Sound**: Point to any local folder of audio tracks for background focus music.
* **Menu Bar Indicator**: Mini-timer persists in the macOS status menu bar during sessions.

#### ✅ Daily Tasks (`:todo`)
* **Daily Kanban**: Structured daily agenda grouping (3 daily focal tasks, overdue badges, completed tasks).
* **Fuzzy Filtering**: Search your task history dynamically across descriptions and target dates.
* **Productivity Stats**: Completion streaks, monthly heatmaps, and weekly velocity charts.
* **Undo History**: 50-step in-memory undo/redo via `Cmd+Z` / `Cmd+Shift+Z`.

#### ⚡ Speed Test (`:speed`)
* **Live Visual Dial**: Real-time download/upload dials measuring from 1 Mbps to 1 Gbps with latency jitter tracking.
* **Network Inspection**: Displays internal LAN IPv4, external WAN IP (click-to-copy or click-to-mask), and ISP identification.
* **Engine**: Benchmarked against Cloudflare CDN edge nodes without external binary dependencies (`core/netspeed`).

#### 🧮 Calculator & Converter (`:calc`, or just type an expression)
Backed by `core/calc` and the macOS `QuickMathService`:
* **Math**: `2+2`, `sqrt(16)`, `200*15%`, `2^8`, `5!`, constants `pi` / `e`. A pinned Calculator row appears; `Enter` copies the value.
* **Unit conversion**: `<amount> <unit> (to|in|->|=) <unit>`. Includes digital units (b, KB, MB, GB, TB, PB).
* **Currency conversion**: same pattern with currency codes.
* **Timestamps**: `timestamp` / `timestamp now`, or `timestamp <epoch>`, converts to a readable date.

> Exact unit and currency coverage lives in `QuickMathService.swift`; check it for the full list.

#### 😀 Emoji Search (`:emoji`)
* **Search** by name or keyword (offline, bundled Unicode CLDR data in `core/emoji`, shared by macOS and Linux/Windows).
* `Enter` copies the selected emoji; `Shift+Enter` types it into the app you came from, falling back to copy with a notice when insertion isn't permitted (e.g. macOS Accessibility denied).
* An empty query shows your last 20 emoji, most recent first, kept across restarts.

#### 🛑 Process & Port Killer (`:kill` / `ps"`)
* **Kill by Name**: Fuzzy search across running processes and terminate with confirmation.
* **Kill by Port**: Target stuck local web servers (e.g. `:kill 3000` or `ps"8080`) to immediately release locked network sockets.

#### 🐚 Shell Executor (`:shell`)
* Execute ad-hoc terminal snippets directly from the launcher.
* **Safety Cue**: Displays an orange visual warning when input contains `sudo` or privileged commands.
