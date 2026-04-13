# look User Guide

`look` is a keyboard-first launcher for macOS focused on fast local actions.

## 1) Install and first run

Compatibility:

- macOS 15.0+

Homebrew:

```bash
brew tap kunkka19xx/tap
brew install --cask look
```

If Spotlight and look both use `Cmd+Space`, disable or rebind Spotlight:

- `System Settings` -> `Keyboard` -> `Keyboard Shortcuts...` -> `Spotlight`

Release builds are signed + notarized. Normal first launch should not require `Open Anyway`.

## 2) Core workflow

In the main input, type to search and press `Enter` to open.

Default search sources:

- installed apps
- local files/folders (from configured roots)
- curated System Settings entries

Useful actions:

- `Cmd+F`: reveal selected app/file/folder in Finder
- `Cmd+C`: copy selected file/folder
- `Cmd+Enter`: web search current query (Google)

## 3) Query prefixes

- `a"term` -> apps only
- `f"term` -> files only
- `d"term` -> folders only
- `r"pattern` -> regex search (case-insensitive)
- `c"term` -> clipboard history search
- `t"text` -> quick translation panel
- `tw"text` -> dictionary lookup panel

Path-like queries (for example `git/project/readme`) are also supported and bias path matches.

## 4) Clipboard and translation

Clipboard mode (`c"`):

- stores recent text clips for the running app session,
- `Enter` on a clipboard row copies that content back to clipboard.

Translation mode (`t"`/`tw"`):

- supports EN/VI/JA result sections,
- translation uses network requests.

## 5) Command mode

Enter command mode with `Cmd+/`.

Built-in commands:

- `calc`: evaluate expressions
- `shell`: run shell command text
- `kill`: force-kill a running app (with confirmation)
- `sys`: show system information

Behavior:

- `Escape`: leave command mode
- `Shift+Escape`: hide launcher
- shell text containing `sudo` shows an orange warning cue

## 6) Settings and config

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
- `file_scan_roots`, `file_scan_depth`, `file_scan_limit`, `file_exclude_paths`
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

## 7) Keyboard shortcuts (quick reference)

- `Enter`: open selected result / run command
- `Tab` / `Shift+Tab`: next/previous result or command
- `Up` / `Down`: move selection
- `Cmd+/`: command mode
- `Escape`: back/close (context dependent)
- `Shift+Escape`: hide launcher
- `Cmd+Enter`: web search
- `Cmd+F`: reveal in Finder
- `Cmd+C`: copy selected file/folder
- `Cmd+Shift+,`: toggle settings panel
- `Cmd+Shift+;`: reload config
- `Cmd+-`, `Cmd+=`, `Cmd+0`: temporary UI zoom out/in/reset

## 8) Troubleshooting

If results seem stale:

- reload config with `Cmd+Shift+;`
- check scan roots/depth/limits in `~/.look.config`

If hotkey does not work:

- verify Spotlight shortcut conflict
- relaunch the app and test again

If translation does not return results:

- check connectivity and retry

## 9) Related docs

- Architecture guide: `docs/architecture.md`
- Feature status: `docs/features.md`
- Backend contributor guide: `docs/backend-guide.md`
- Tech blog (EN): `docs/tech-blog-core-algorithms.md`
- Tech blog (VI): `docs/tech-blog-core-algorithms.vi.md`
