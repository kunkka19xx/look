# look User Guide

`look` is a keyboard-first launcher for macOS, Windows, and Linux focused on fast local actions.

> **Cross-platform shortcut note.** Examples are written with macOS modifiers (`Cmd+...`). On Windows and Linux, read `Cmd` as `Ctrl` - except the launcher toggle, which is `Alt+Space` (since `Win+Space` / `Super+Space` are reserved by the OS or desktop environment).
>
> | macOS           | Windows / Linux  |
> | --------------- | ---------------- |
> | `Cmd+Space`     | `Alt+Space`      |
> | `Cmd+Enter`     | `Ctrl+Enter`     |
> | `Cmd+F`         | `Ctrl+F`         |
> | `Cmd+C`         | `Ctrl+C`         |
> | `Cmd+/`         | `Ctrl+/`         |
> | `Cmd+0`         | `Ctrl+0`         |
> | `Cmd+1`…`Cmd+5` | `Ctrl+1`…`Ctrl+5`|
> | `Cmd+1`…`Cmd+9` (running-apps switcher) | `Alt+1`…`Alt+9` |
> | `Cmd+P`         | `Ctrl+P`         |
> | `Cmd+Shift+P`   | `Ctrl+Shift+P`   |
> | `Cmd+Shift+,`   | `Ctrl+Shift+,`   |
> | `Cmd+Shift+;`   | `Ctrl+Shift+;`   |
>
> "Reveal in Finder" reads as "Reveal in Explorer" on Windows and "Show in Files" on Linux.

## First run

Install with Homebrew (see [README](../README.md#install) for alternatives):

```bash
brew tap kunkka19xx/tap
brew install --cask look
```

On first launch, Look will index your apps, files, and folders in the background. You can start using it immediately - results appear as indexing completes.

To bind `Cmd+Space` to Look, disable Spotlight's default shortcut: `System Settings > Keyboard > Keyboard Shortcuts > Spotlight`.

## Permissions

Look is designed to need as few macOS permissions as possible:

- **No Accessibility permission** is required.
- **No Full Disk Access** is required. Look indexes standard user directories (`~`, `/Applications`, `~/Documents`, `~/Downloads`, etc.). To index a directory outside those defaults, add it via `file_scan_extra_roots` in `~/.look.config`.
- **No Screen Recording** is required.
- **Network access** is used for explicit actions - `t"` translation, `tw"` dictionary lookup, and `Cmd+Enter` web search — and, when **AI features** are enabled (macOS, on by default), for live Google search suggestions and the DuckDuckGo/Wikipedia answer card as you type. The on-device Apple Intelligence model runs locally and makes no network calls of its own. Turn the AI/web features off by setting `ai_enabled = false` in `~/.look.config` (or via Settings). Local search and indexing never make network calls.
- **Finder Automation** is requested only when you empty the Trash (`Cmd+D` on the pinned Trash folder). The Trash is protected by macOS, so Look asks Finder to empty it; macOS prompts once, and you can manage it under `System Settings > Privacy & Security > Automation`. Moving individual files to the Trash needs no permission.

If macOS prompts for permission during an action you didn't trigger, that's a bug - please [file an issue](https://github.com/kunkka19xx/look/issues).

## Core workflow

In the main input, type to search and press `Enter` to open.

Default search sources:

- installed apps
- local files/folders (from configured roots)
- curated System Settings entries

Useful actions:

- `Cmd+F`: reveal selected app/file/folder in Finder
- `Cmd+C`: copy selected file/folder
- `Cmd+P`: toggle pick on the selected file/folder (multi-select); the picked set is written to the system pasteboard so you can paste them anywhere in Finder
- `Cmd+Shift+P`: clear all picked items
- `Cmd+D`: move the selected file/folder - or all picked items — to the Trash (macOS only for now). Like Finder's `Cmd+Delete`, this is immediate and unconfirmed because it's recoverable: the items go to the Trash, not permanent deletion. The rows disappear from results right away.
- `Cmd+Enter`: web search current query (Google)

When at least one item is picked, the right panel switches to the **Picked** list - each row has an `X` to remove a single item, plus a **Clear all** button. File/folder copies (both `Cmd+C` and `Cmd+P`) are excluded from clipboard history.

**Trash.** Type `trash` to pin the Trash quick folder; `Enter` opens it in Finder. With the Trash folder selected, its preview shows the item count and `Cmd+D` **empties** the Trash. Emptying is permanent, so it asks you to confirm (`Y`/`Enter` to empty, `N`/`Esc` to cancel). Look empties the Trash through Finder, so the first time you do this macOS asks for permission to control Finder (see [Permissions](#permissions)).

## AI answers and web suggestions (macOS, Linux, Windows)

Look can answer questions and look things up without leaving the launcher. These features are **on by default** on macOS, Linux, and Windows. Toggle them in Settings or with `ai_enabled` in `~/.look.config`.

- **Answer card.** A question, an entity that has no local match (e.g. `sir alex ferguson`), or an instant-answer pattern (weather, currency, crypto) shows a Spotlight-style card above the results. Sources resolve independently and each appears as it lands - local **Calculator** first, then **DuckDuckGo** and **Wikipedia**. On macOS, when no web source has an answer it falls back to a streaming on-device **Apple Intelligence** answer. Click a source label to open it; the copy button copies that block.
- **Search suggestions.** For plain text queries (2+ characters), Google autocomplete rows appear under the results. `Enter` on a suggestion (or `Cmd+Enter` on your query) runs a web search in your default browser.
- **Query rewrite** *(macOS only)*. When a natural-language query finds nothing locally, the on-device model rewrites it into Look's prefix grammar and searches again. It never overrides results you can already see - it only runs when the raw query came up empty.

**Platform note.** The web answer card and Google suggestions are available on macOS, Linux, and Windows. The on-device LLM - query rewrite and the Apple Intelligence answer fallback - is **macOS-only**; there is no on-device model on Linux/Windows, so there the card uses web sources (DuckDuckGo, Wikipedia, currency/weather/crypto) only. The `ai_enabled` toggle is shared across platforms.

**Network note.** While AI features are on, the answer card's web sources and the Google suggestions send your typed query to those services (DuckDuckGo, Wikipedia, Google). The on-device model makes no network calls of its own. Set `ai_enabled = false` to disable all of it and run fully offline.

## Query prefixes

Don't remember the prefixes? Type a single `"` to open a menu listing every prefix with a short description - pick one (click or `↑`/`↓` then `Enter`) to drop it into the search field, ready for your term.

- `a"term` -> apps only
- `f"term` -> files only
- `d"term` -> folders only
- `rc"term` -> recent files/folders, newest activity first (optional filter; `rc"` alone lists all). Blends what you've opened through Look with what recently appeared/changed on disk (downloads, screenshots). macOS for now.
- `r"pattern` -> regex search (case-insensitive)
- `c"term` -> clipboard history search
- `t"text` -> quick translation panel
- `tw"text` -> dictionary lookup panel

Path-like queries (for example `git/project/readme`) are also supported and bias path matches.

## Clipboard and translation

Clipboard mode (`c"`):

- stores recent text clips for the running app session,
- `Enter` on a clipboard row copies that content back to clipboard.

Translation mode (`t"`/`tw"`):

- supports EN/VI/JA result sections,
- translation uses network requests.

## Command mode

Enter command mode with `Cmd+/`, or jump straight to a specific command from the home screen with the `:` prefix:

- `:calc` then `Enter` - open `/calc` with empty input
- `:calc 2+2` - opens `/calc` with `2+2` already typed (the space after the command id is the trigger; you can keep typing without pressing Enter)
- Same pattern for `:shell`, `:kill`, `:sys`

The `:` prefix only triggers when the word right after it is a known command id (`calc`, `shell`, `kill`, `sys`); anything else (`:foo`, `:Users/me/...`) stays in normal search.

Built-in commands:

- `calc`: evaluate expressions (supports `^`, `!`, constants `pi`/`e`, functions `sqrt`/`abs`/`round`/`floor`/`ceil`, plus `%` shorthand)
- `shell`: run shell command text
- `kill`: force-kill a running app/process (with confirmation), supports port queries like `:3000` or `port 3000`
- `sys`: show system information
- `pomo`: pomodoro focus timer with editable session list, three timer styles (Modern Ring / Vintage Dial / Minimal Text), background-music folder, menu-bar mini-timer, and a 5-second standby fade

`calc` quick examples:

- `2^3` -> `8`
- `-2^2` -> `-4`
- `4!` -> `24`
- `2*pi` -> `6.2832`
- `200*15%` -> `30`
- `10%3` -> `1` (`%` remains modulo when used between operands)

`pomo` quick reference:

- Edit the **Session List** to plan focus + break blocks; the timer auto-advances through them and loops the music folder while running
- `Space` start/pause the active session • `R` reset • `P` toggle music play/pause
- Pick a folder of audio files (mp3/m4a/wav/aac/flac/ogg/aiff/alac); tracks are played one at a time, shuffled per launch
- A "session ending soon" alert fires 10s before each block ends - both as a menu-bar popover and (when granted) a macOS notification with chime
- Menu-bar mini-timer shows remaining time even when the launcher is hidden; click to jump back into `/pomo`

Behavior:

- `Escape`: leave command mode
- `Shift+Escape`: hide launcher
- `Tab` / `Shift+Tab`: switch commands while staying in command mode
- `Cmd+1` / `Cmd+2` / `Cmd+3` / `Cmd+4` / `Cmd+5`: jump to specific command (`shell`, `calc`, `kill`, `sys`, `pomo`)
- `Up` / `Down`: in `kill`, navigate process/app results
- shell text containing `sudo` shows an orange warning cue

## Settings and config

Open settings with `Cmd+Shift+,`.

### Appearance / Themes

The Appearance tab controls:

- **Tint Color** - accent color for UI highlights (RGB + opacity)
- **Blur** - blur material and opacity for the launcher window
- **Font** - name and size for launcher text
- **Font Color** - text color (RGB + opacity)
- **Border** - border thickness and color

Built-in theme presets are available:

| Theme       | Description                       |
| ----------- | --------------------------------- |
| Catppuccin  | Warm pastels (Mocha variant)      |
| Tokyo Night | Dark with vibrant accents         |
| Rose Pine   | Soft pink-tinted dark theme       |
| Gruvbox     | Retro warm tones                  |
| Dracula     | Classic purple-accented dark      |
| Kanagawa    | Japanese-inspired dark theme      |
| Custom      | Your own colors derived from tint |

Theme is saved as `ui_theme=<name>` in config.

**Running Apps**: a switch that shows running-app icons in the right half of the search bar. When on, the search field shrinks to the left half and the running apps fill the right half (right-aligned, growing leftward as more apps open). Each icon has a corner number badge; pressing the modifier + the badge digit on the home screen activates that app - `Cmd+1`..`Cmd+9` on macOS, `Alt+1`..`Alt+9` on Linux and Windows. When off, the search bar spans the full width and the switcher shortcut is disabled. The launcher window stays the same size either way.

Behavior:

- **Stable** - icons sit in alphabetical order and don't shuffle when you switch apps. The activation digit for a given app stays the same until you launch or quit something.
- **Ergonomic badge keys** - easier-to-reach keys are assigned first. With 5 running apps the badges are `1, 2, 3, 8, 9` (skipping the harder middle keys); `5/6/7` only get used when you have 7+ apps running.
- **Linux focus** - Look's GNOME Shell extension activates the app's most-recent window on Wayland; X11 uses `_NET_ACTIVE_WINDOW` via x11rb; sway/Hyprland use `wlr-foreign-toplevel-management`; i3 uses `i3-msg`.
- **Windowless apps** (Finder with no Finder windows, etc.) get a fresh window via a Dock-style "reopen" so you don't see an empty flash.

Saved as `running_apps_placement=<value>` in `~/.look.config` (`none` = off, any other value = on; legacy `top`/`right`/`bottom` values still load as "on"). New keys are auto-appended to existing config files on next Save Config.

### Indexing Settings

Default values:

- **File Scan Depth**: 4 (range: 1-12)
- **File Scan Limit**: 4000 (range: 500-50000)
- **Lazy indexing**: On

Advanced controls:

- **Extra Scan Dirs**: add user-specific directories to index on top of default roots
- overlap and risky-root validation is enforced for extra scan dirs

These control how deeply and how many files are indexed for search.

Lazy indexing behavior:

- when **On**, Look listens for file/app create/remove/rename events and marks the index dirty,
- pressing `Cmd+Space` triggers background reindex only when dirty,
- when **Off**, pressing `Cmd+Space` always triggers background reindex.

### Other Settings

- settings-only blur multiplier (`Settings Blur`) for readability when settings is open
- translation privacy and backend log level
- launch at login

Runtime config file:

- path: `~/.look.config`
- optional override: `LOOK_CONFIG_PATH=/path/to/config`
- reload after manual edits: `Cmd+Shift+;`
- reset to fresh defaults from UI: `Settings -> Advanced -> Create Fresh Config` (confirmation popup)

Backend-related keys:

- `app_scan_roots`, `app_scan_depth`, `app_exclude_paths`, `app_exclude_names`
- `file_scan_roots`, `file_scan_extra_roots`, `file_scan_depth`, `file_scan_limit`, `file_exclude_paths`
- `lazy_indexing_enabled`
- `skip_dir_names`
- `alias_<keyword>` (for app + System Settings query aliases, for example `alias_note=Notion|Obsidian|Notes|Apple Notes|Bear|Logseq`)
- `backend_log_level`, `launch_at_login`

Alias note:

- aliases do not create synthetic results; they only boost existing indexed app/System Settings entries
- if an aliased app is not installed, there is no error and no result is added
- keep alias lists short (around 5-10 targets per keyword) to avoid noisy ranking

Default alias presets (fresh config files):

- `alias_note=Notion|Obsidian|Notes|Apple Notes|Bear|Logseq`
- `alias_code=Visual Studio Code|VSCode|Cursor|Windsurf|IntelliJ IDEA|PyCharm|WebStorm|Neovim|Xcode|Zed`
- `alias_term=Terminal|iTerm|iTerm2|Ghostty|WezTerm|Alacritty|Kitty|Warp`
- `alias_chat=Slack|Discord|Telegram|Messages`
- `alias_music=Spotify|Apple Music|Music`
- `alias_brow=Safari|Arc|Google Chrome|Chrome|Firefox|Brave`

Preset update behavior:

- presets are written automatically only when `~/.look.config` is created for the first time
- app updates do not rewrite an existing config file, so existing users should add new `alias_*` keys manually

Fresh config reset behavior:

- `Create Fresh Config` replaces the current config file with the latest default template
- reset uses the active config path (`LOOK_CONFIG_PATH` when set, otherwise `~/.look.config`)
- existing custom values are replaced during this reset flow (use manual edit + `Cmd+Shift+;` if you only want partial changes)

UI-related keys include the `ui_*` group (tint/blur/font/border values).

Note: `Settings Blur` is stored as local app UI state (UserDefaults) and is not written to `~/.look.config`.

## Keyboard shortcuts (quick reference)

- `Enter`: open selected result / run command
- `Tab` / `Shift+Tab`: next/previous result (app list) or command (command mode)
- `Up` / `Down`: move selection (and in `kill`, move process selection)
- `Cmd+/`: command mode
- `:cmd` (e.g. `:calc 2+2`, `:kill chrome`, `:sys`, `:pomo`): jump to a command directly from the home screen
- `Cmd+1` / `Cmd+2` / `Cmd+3` / `Cmd+4` / `Cmd+5`: in command mode, direct command switch (`shell`, `calc`, `kill`, `sys`, `pomo`)
- `Cmd+1`..`Cmd+9` (macOS) / `Alt+1`..`Alt+9` (Linux, Windows): on the home screen, activate the running-app whose badge shows that digit, when `Running Apps` is on. Badge labels are ergonomic, not strictly positional - see Settings → Appearance → Running Apps
- `Space` / `R` / `P` (inside `/pomo`): start/pause session, reset, toggle music play/pause
- `Escape`: back/close (context dependent)
- `Shift+Escape`: hide launcher
- `Cmd+Enter`: web search
- `Cmd+F`: reveal in Finder
- `Cmd+C`: copy selected file/folder
- `Cmd+P` / `Cmd+Shift+P`: toggle pick / clear picked set
- `Cmd+D`: move selected file/folder (or picked items) to Trash; on the pinned Trash folder, empty the Trash (macOS only for now)
- `Cmd+Shift+,`: toggle settings panel
- `Cmd+Shift+;`: reload config
- `Cmd+-`, `Cmd+=`, `Cmd+0`: temporary UI zoom out/in/reset

## Troubleshooting

**Results seem stale or a newly installed app is missing.**

- reload config with `Cmd+Shift+;`
- if lazy indexing is Off, Look reindexes on every launcher open; if On, it reindexes only when filesystem changes are detected
- check scan roots, depth, and limits in `~/.look.config`
- add user-specific directories via `file_scan_extra_roots`

**`Cmd+Space` does not open Look.**

- confirm Spotlight's `Cmd+Space` is disabled or rebound (`System Settings > Keyboard > Keyboard Shortcuts > Spotlight`)
- relaunch Look (`open "/Applications/Look.app"`) after changing the Spotlight binding
- if you previously ran a dev/side-by-side build, make sure only one Look instance is running

**The launcher opens behind another window.**

- this is usually a focus-handoff timing issue; hide the launcher (`Escape`) and open it again
- if it reproduces consistently, please file an issue with your macOS version

**High CPU or slow first launch.**

- the initial index scan is a one-time cost on first run; subsequent launches use the cached SQLite index
- you can lower `file_scan_depth` and `file_scan_limit` in `~/.look.config` if you have very large user directories

**A config change was ignored.**

- Look reads `~/.look.config` at launch. After editing manually, reload with `Cmd+Shift+;` or restart Look.
- confirm you edited the active config path (`LOOK_CONFIG_PATH` overrides `~/.look.config` when set)

**Translation (`t"` / `tw"`) returns no results.**

- translation requires network; check connectivity and retry
- corporate proxies and VPNs can block the translation endpoint

**Linux only - ghost slider trails or overlapping popovers in Settings.**

- observed on Arch GNOME 50 + webkit2gtk 2.52.3; Ubuntu 26.04 and NixOS 2.50.6 on identical webkit are unaffected, so this is a stack-interaction bug we can't auto-detect
- open **Settings > Advanced > Arch** and flip one toggle:
  - **Disable GPU compositing** - keeps blur, fixes the ghost. Requires restart.
  - **Disable blur effect** - drops blur, keeps tint. Takes effect immediately.

**I want to reset everything to defaults.**

- `Settings > Advanced > Create Fresh Config` rewrites `~/.look.config` from the latest defaults (with a confirmation prompt)

## Uninstall

Homebrew:

```bash
brew uninstall --cask look
brew untap kunkka19xx/tap   # optional
```

Manual install:

```bash
rm -rf "/Applications/Look.app"
```

Remove local state (optional - includes config, index, and usage history):

```bash
rm -f "$HOME/.look.config"
rm -rf "$HOME/Library/Application Support/look"
```

## Related docs

- Architecture guide: `docs/architecture.md`
- Feature status: `docs/features.md`
- Backend contributor guide: `docs/backend-guide.md`
- Tech blog (EN): `docs/tech-blog-core-algorithms.md`
- Tech blog (VI): `docs/tech-blog-core-algorithms.vi.md`
