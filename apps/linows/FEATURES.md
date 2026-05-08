# linows — Features & Behavior Reference

Complete feature documentation based on macOS app (source of truth).

---

## Query Prefix System

The search input recognizes special prefixes that change search behavior:

| Prefix | Mode | Description | Example |
|--------|------|-------------|---------|
| (none) | Default search | Fuzzy match all candidates (apps, files, folders, settings) | `firefox` |
| `a"` | App-only | Filter results to apps only | `a"code` |
| `f"` | File-only | Filter results to files only | `f"readme` |
| `d"` | Folder-only | Filter results to directories/folders only | `d"documents` |
| `r"` | Regex | Match title/path/subtitle by regex pattern | `r"^Visual.*Code$` |
| `c"` | Clipboard | Browse clipboard history, optional filter | `c"` or `c"password` |
| `t"` | Translation | Translate text via web (Google Translate) | `t"hello world` |
| `tw"` | Dictionary | Local dictionary lookup with definitions | `tw"ephemeral` |

### Prefix Behavior Details

**Default Search (no prefix):**
- Empty query → browse mode (ranked by frequency + recency)
- Fuzzy matching on title, subtitle, path
- Alias expansion (e.g., "note" matches Notion, Obsidian, etc.)
- Kind bias: Apps +220, Folders 0, Files -20
- Limit: 40 results

**App-only (`a"`):**
- Only shows CandidateKind::App results
- Aliases still apply
- Good for disambiguating when file names match app names

**File-only (`f"`):**
- Only shows CandidateKind::File results
- Aliases do NOT apply (intentional)
- Path matching works (e.g., `f"git/books-pc`)

**Folder-only (`d"`):**
- Only shows CandidateKind::Folder results
- Useful for finding project directories

**Regex (`r"`):**
- Matches against title, path, and subtitle
- Invalid regex returns empty results (no crash)
- Scoring: title+path match > title only > path only > subtitle only

**Clipboard (`c"`):**
- Shows clipboard history (max 10 entries)
- Optional query filters entries by content
- Enter → copy entry back to clipboard
- Delete → remove entry from history

**Translation (`t"`):**
- Sends text to translation service (via Rust FFI)
- Shows result in translation panel with language buttons
- Languages: English, Vietnamese (Tiếng Việt), Japanese (日本語)
- Copy result or open in browser

**Dictionary (`tw"`):**
- Local dictionary lookup (platform dictionary API)
- Shows definitions inline
- Speech playback button (if available)

---

## Multi-Pick System

### Behavior
- **Activate:** Ctrl+P on a focused result (toggles pick on/off)
- **Visual:** Checkmark icon appears on picked items
- **Panel:** Right side shows "Picked Items" panel with list + remove buttons
- **Clear all:** Ctrl+Shift+P clears all picks
- **Clipboard:** Picked items written to clipboard as file paths (one per line)
- **Persistence:** Picks cleared when window hides

### Data Flow
1. User selects result → presses Ctrl+P
2. Item added to picks array (by id + path)
3. Picked items panel shows on right (replaces preview panel)
4. On copy (Ctrl+C with picks): write all paths to clipboard, separated by newlines
5. On clear: empty picks array, hide panel, show preview again

### Pick Limits
- No explicit max (practical limit ~20-30 items based on UI space)
- Same item cannot be picked twice (toggle behavior)

---

## Command Mode (Ctrl+/)

Activated by Ctrl+/, hides search bar, shows sidebar with 5 commands.
Context-sensitive hint bar at the bottom shows available shortcuts per command.

### Calculator (`calc`)

**Operators:** `+`, `-`, `*`, `/`, `%`, `^` (power), parentheses
**Features:**
- Real-time evaluation as you type
- Result auto-copied on Enter
- Handles decimal numbers
- Error display for invalid expressions

**Examples:**
```
2 + 3 * 4        → 14
(2 + 3) * 4      → 20
2 ^ 10            → 1024
100 % 7           → 2
3.14 * 2          → 6.28
```

### Shell (`shell`)

**Behavior:**
- Runs command via system shell (cmd.exe on Windows, /bin/sh on Linux)
- Captures stdout (max 800 characters displayed)
- Shows "running..." while executing
- No interactive commands (no stdin)
- Timeout: reasonable limit to prevent hangs

**Examples:**
```
echo hello          → hello
dir                 → (directory listing)
whoami              → username
date                → current date/time
```

### Kill (`kill`)

**Flow:**
1. Type process name, PID, or port number
2. Fuzzy-match against running processes
3. Show matching processes with PID, name, memory usage
4. Select target → confirmation bar appears at bottom
5. Confirm → terminate process
6. Cancel → dismiss confirmation

**Matching:**
- By name: fuzzy match against process names
- By PID: exact match
- By port: find process listening on port (netstat/ss)

### Pomodoro (`pomo`)

**Features:**
- Configurable session list (focus/break, name, duration) — editable inline
- 3 timer styles: Modern Ring, Vintage Dial, Minimal Text
- Start/Pause/Resume with keyboard shortcut hint on button
- Skip and Reset controls
- Session List toggle (collapsible, shows count)
- Idle standby mode: after 5s of no interaction, fades UI and expands timer
- Desktop notifications for phase transitions and ending soon (10s warning)

**Background Music:**
- Folder-based music player (Choose folder → scan for audio files)
- Supported formats: mp3, m4a, wav, aac, flac, ogg, aiff, alac
- Shuffle on load, auto-advance to next track
- Play/Pause/Prev/Next controls
- Audio playback via Rust `rodio` crate (not HTML5 Audio — WebKitGTK has issues)
- Folder picker via `tauri-plugin-dialog` (native cross-platform)
- Persists selected folder in localStorage

**Keyboard:**
| Key | Action |
|-----|--------|
| Space | Start/Pause/Resume timer |
| R | Reset timer |
| P | Toggle music play/pause |

### System Info (`sys`)

**Displays:**
| Field | Source |
|-------|--------|
| CPU | Model, core count, usage % |
| Memory | Total, used, available |
| Disk | Total, used, free per volume |
| GPU | Model, VRAM (if available) |
| Network | IP address, active interface |
| Uptime | System uptime |
| OS | Version string |

**Updates:** Every 2 seconds while panel is visible.

---

## Quick Folders

Appear in results when query matches folder name or when browsing:

| Folder | Path (Windows) | Path (Linux) |
|--------|---------------|--------------|
| Desktop | `%USERPROFILE%\Desktop` | `~/Desktop` |
| Documents | `%USERPROFILE%\Documents` | `~/Documents` |
| Downloads | `%USERPROFILE%\Downloads` | `~/Downloads` |
| Pictures | `%USERPROFILE%\Pictures` | `~/Pictures` |
| Videos | `%USERPROFILE%\Videos` | `~/Videos` |
| Music | `%USERPROFILE%\Music` | `~/Music` |

**OneDrive awareness (Windows):** If OneDrive is syncing, use OneDrive paths instead.

---

## App Discovery & Indexing

### Windows Sources
| Source | Method |
|--------|--------|
| Start Menu shortcuts | Scan `%ProgramData%\Microsoft\Windows\Start Menu\Programs` |
| User Start Menu | Scan `%APPDATA%\Microsoft\Windows\Start Menu\Programs` |
| UWP/Packaged apps | PowerShell enumeration of `shell:AppsFolder` → `seed_uwp_apps` |
| Windows Settings | Hardcoded `ms-settings:` URIs with metadata |

### Linux Sources
| Source | Method |
|--------|--------|
| System applications | Scan `/usr/share/applications/*.desktop` |
| User applications | Scan `~/.local/share/applications/*.desktop` |
| Flatpak | `/var/lib/flatpak/exports/share/applications/` |
| Snap | `/var/lib/snapd/desktop/applications/` |

### File Indexing (both platforms)
- Scans configured `file_scan_roots` (default: ~/Desktop, ~/Documents, ~/Downloads, etc.)
- Respects `file_scan_depth` (default 4, range 1-12)
- Respects `file_scan_limit` (default 8000, range 500-50000)
- Skips directories in `skip_dir_names` (node_modules, target, build, dist, etc.)
- Lazy indexing: file watchers mark index dirty on create/rename/delete
- Background refresh on window toggle (if dirty)

### Candidate Kinds
| Kind | ID prefix | Examples |
|------|-----------|----------|
| App | `app:` | `app:firefox`, `app:vscode` |
| File | `file:` or `file.` | `file:readme`, `file.notes` |
| Folder | `folder:` or `folder.` | `folder:docs`, `folder.projects` |
| Setting | `setting:` | `setting:display`, `setting:network` |

---

## Settings (.look.config)

INI-style config file. Shared format with macOS app.

**Location:**
- Custom: `LOOK_CONFIG_PATH` env var
- Windows: `%USERPROFILE%\.look.config`
- Linux: `~/.look.config`

### Backend/Indexing Keys

| Key | Default | Description |
|-----|---------|-------------|
| `app_scan_roots` | (platform-specific) | Comma-separated app directories |
| `app_scan_depth` | `3` | How deep to scan app directories |
| `app_exclude_paths` | (empty) | Comma-separated paths to skip |
| `app_exclude_names` | (empty) | Comma-separated app names to hide |
| `file_scan_roots` | Desktop,Documents,Downloads,... | Comma-separated file directories |
| `file_scan_extra_roots` | (empty) | Additional file directories |
| `file_scan_depth` | `4` | Depth limit (1-12) |
| `file_scan_limit` | `8000` | Max files to index (500-50000) |
| `file_exclude_paths` | (empty) | Comma-separated paths to skip |
| `skip_dir_names` | node_modules,target,build,... | Directories to never enter |
| `lazy_indexing_enabled` | `true` | Only re-index when watchers detect changes |

### UI Theme Keys

| Key | Default | Description |
|-----|---------|-------------|
| `ui_tint_red` | `0.08` | Background tint R (0.0-1.0) |
| `ui_tint_green` | `0.10` | Background tint G |
| `ui_tint_blue` | `0.12` | Background tint B |
| `ui_tint_opacity` | `0.55` | Background tint opacity |
| `ui_blur_material` | `hudWindow` | Blur style (hudWindow/sidebar/menu/underWindowBackground) |
| `ui_blur_opacity` | `0.95` | Blur backdrop opacity |
| `ui_font_name` | `SF Pro Text` | Font family name |
| `ui_font_size` | `14` | Base font size in px |
| `ui_font_red` | `0.96` | Font color R |
| `ui_font_green` | `0.96` | Font color G |
| `ui_font_blue` | `0.98` | Font color B |
| `ui_font_opacity` | `0.96` | Font color opacity |
| `ui_border_thickness` | `1.0` | Border width in px |
| `ui_border_red` | `1.0` | Border color R |
| `ui_border_green` | `1.0` | Border color G |
| `ui_border_blue` | `1.0` | Border color B |
| `ui_border_opacity` | `0.12` | Border color opacity |

### Search Alias Keys

Format: `alias_<keyword>=Term1|Term2|Term3`

| Key | Default targets |
|-----|----------------|
| `alias_note` | Notion, Obsidian, Notes, Apple Notes, Bear, Logseq, OneNote, Sticky Notes, Joplin |
| `alias_code` | Visual Studio Code, VSCode, Cursor, Windsurf, IntelliJ IDEA, PyCharm, WebStorm, Neovim, Xcode, Zed, Notepad++, Sublime Text |
| `alias_term` | Terminal, iTerm2, Ghostty, WezTerm, Alacritty, Kitty, Warp, Windows Terminal, PowerShell, Command Prompt, wsl |
| `alias_chat` | Slack, Discord, Telegram, Messages, Microsoft Teams, WhatsApp, Signal, Zoom |
| `alias_music` | Spotify, Apple Music, YouTube Music, VLC, Windows Media Player, foobar2000 |
| `alias_brow` | Safari, Arc, Google Chrome, Firefox, Brave, Microsoft Edge, Vivaldi, Opera |

Set `alias_<keyword>=` (empty value) to remove a default alias.

---

## Keyboard Shortcuts

### Global (system-wide)
| Shortcut | Action |
|----------|--------|
| Alt+Space | Toggle window visibility |
| Alt+Shift+Q | Quit application |

### Search Mode
| Shortcut | Action |
|----------|--------|
| Enter | Open selected item |
| Escape | Hide window |
| Arrow Up/Down | Navigate results |
| Ctrl+F | Reveal selected in file manager |
| Ctrl+C | Copy selected item's path |
| Ctrl+P | Toggle pick on selected item |
| Ctrl+Shift+P | Clear all picks |
| Ctrl+/ | Enter command mode |
| Ctrl+Shift+, | Open settings |
| Ctrl+H | Show help screen |

### Command Mode
| Shortcut | Action |
|----------|--------|
| Ctrl+1 | Switch to calc |
| Ctrl+2 | Switch to pomo |
| Ctrl+3 | Switch to kill |
| Ctrl+4 | Switch to shell |
| Ctrl+5 | Switch to sys |
| Tab | Cycle to next command |
| Escape | Exit command mode → search mode |
| Enter | Execute command / confirm kill |

### Clipboard Mode (c" prefix)
| Shortcut | Action |
|----------|--------|
| Enter | Copy entry to clipboard |
| Delete | Remove entry from history |
| Escape | Clear prefix, return to search |

### Settings / Help
| Shortcut | Action |
|----------|--------|
| Escape | Close panel, return to search |

---

## Clipboard History

### Behavior
- **Capture:** Monitors system clipboard in background
- **Polling:** 0.35s (foreground), 0.9s (background), 0.08s (burst after paste)
- **Storage:** In-memory ring buffer (max 10 entries, 30KB per entry)
- **Persistence:** Saved to disk at `{data_dir}/clipboard.json`
- **Access:** Type `c"` to browse, `c"query` to filter
- **Deduplication:** Same content not stored twice (moves to top)

### Entry Display
- First line of text (truncated)
- Relative time (2m ago, 1h ago, etc.)
- Character count + line count
- Delete button (per entry)

---

## Banner Notifications

Toast-style messages that appear below the search bar:

| Trigger | Message | Has copy button |
|---------|---------|----------------|
| Item opened | "Opened {title}" | No |
| Path copied | "Copied to clipboard" | No |
| Picks copied | "Copied {n} items" | No |
| Calc result | "= {result}" | Yes |
| Shell output | First line of output | Yes |
| Config reloaded | "Config reloaded" | No |
| Index refreshed | "Index refreshed ({n} items)" | No |
| Error | Error message | No |

**Animation:** Slide in from top + fade, auto-dismiss after 2s.

---

## Result Row Layout

Each result row displays:

```
[22px icon]  Title Text                [kind badge]
             /path/or/subtitle/text
```

### Kind Badges
| Kind | Badge text | Color |
|------|-----------|-------|
| App | `APP` | accent |
| File | `FILE` | success (green) |
| Folder | `DIR` | warning (orange) |
| Setting | `SYS` | info (blue) |

### Icon Sources
- **Apps (Windows):** Extracted from .exe/.lnk via shell APIs
- **Apps (Linux):** From .desktop file Icon= field, looked up in icon theme
- **Files:** MIME-type based generic icons
- **Folders:** Generic folder icon
- **Settings:** Fluent/system icons per setting type
- **Fallback:** First letter of title in a colored circle

---

## Search Scoring & Ranking

### Score Components
1. **Fuzzy title match** — primary signal
2. **Contains match** (title: 1200, subtitle: 900)
3. **Token match** (all query words found: 850)
4. **Regex match** (title+path: 1500, title: 1300, path: 1100, subtitle: 1000)
5. **Alias match** — promotes aliased apps
6. **Path depth penalty** — deeper files scored lower
7. **Kind bias** — Apps +220, Folders 0, Files -20
8. **Settings query boost** — settings hints get +420

### Browse Mode (empty query)
Ranked by combined frequency + recency:
- Higher use_count → higher score
- More recent last_used_at → higher score (exponential decay)
- Apps > Folders > Files in tiebreakers

### Reranking
Top 80 candidates from fast retrieval are reranked by the `look_ranking` module
for final ordering (production rank_score algorithm).
