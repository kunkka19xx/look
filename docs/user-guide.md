# look User Guide

`look` is a keyboard-first launcher for macOS focused on fast local actions.

## First run

Install with Homebrew (see [README](../README.md#install) for alternatives):

```bash
brew tap kunkka19xx/tap
brew install --cask look
```

On first launch, Look will index your apps, files, and folders in the background. You can start using it immediately — results appear as indexing completes.

To bind `Cmd+Space` to Look, disable Spotlight's default shortcut: `System Settings > Keyboard > Keyboard Shortcuts > Spotlight`.

## Permissions

Look is designed to need as few macOS permissions as possible:

- **No Accessibility permission** is required.
- **No Full Disk Access** is required. Look indexes standard user directories (`~`, `/Applications`, `~/Documents`, `~/Downloads`, etc.). To index a directory outside those defaults, add it via `file_scan_extra_roots` in `~/.look.config`.
- **No Screen Recording** is required.
- **Network access** is used only for explicit actions: `t"` translation, `tw"` dictionary lookup, and `Cmd+Enter` web search. The local search and indexing paths make no network calls.

If macOS prompts for permission during an action you didn't trigger, that's a bug — please [file an issue](https://github.com/kunkka19xx/look/issues).

## Core workflow

In the main input, type to search and press `Enter` to open.

Default search sources:

- installed apps
- local files/folders (from configured roots)
- curated System Settings entries

Useful actions:

- `Cmd+F`: reveal selected app/file/folder in Finder
- `Cmd+C`: copy selected file/folder
- `Cmd+Enter`: web search current query (Google)

## Query prefixes

- `a"term` -> apps only
- `f"term` -> files only
- `d"term` -> folders only
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

Enter command mode with `Cmd+/`.

Built-in commands:

- `calc`: evaluate expressions (supports `^`, `!`, constants `pi`/`e`, functions `sqrt`/`abs`/`round`/`floor`/`ceil`, plus `%` shorthand)
- `shell`: run shell command text
- `kill`: force-kill a running app/process (with confirmation), supports port queries like `:3000` or `port 3000`
- `sys`: show system information

`calc` quick examples:

- `2^3` -> `8`
- `-2^2` -> `-4`
- `4!` -> `24`
- `2*pi` -> `6.2832`
- `200*15%` -> `30`
- `10%3` -> `1` (`%` remains modulo when used between operands)

Behavior:

- `Escape`: leave command mode
- `Shift+Escape`: hide launcher
- `Tab` / `Shift+Tab`: switch commands while staying in command mode
- `Cmd+1` / `Cmd+2` / `Cmd+3` / `Cmd+4`: jump to specific command (`shell`, `calc`, `kill`, `sys`)
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

| Theme | Description |
|-------|-------------|
| Catppuccin | Warm pastels (Mocha variant) |
| Tokyo Night | Dark with vibrant accents |
| Rose Pine | Soft pink-tinted dark theme |
| Gruvbox | Retro warm tones |
| Dracula | Classic purple-accented dark |
| Kanagawa | Japanese-inspired dark theme |
| Custom | Your own colors derived from tint |

Theme is saved as `ui_theme=<name>` in config.

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
- `Cmd+1` / `Cmd+2` / `Cmd+3` / `Cmd+4`: direct command switch
- `Escape`: back/close (context dependent)
- `Shift+Escape`: hide launcher
- `Cmd+Enter`: web search
- `Cmd+F`: reveal in Finder
- `Cmd+C`: copy selected file/folder
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

Remove local state (optional — includes config, index, and usage history):

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
