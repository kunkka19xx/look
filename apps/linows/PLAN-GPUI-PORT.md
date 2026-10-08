# linows on gpui-ce, the port

Decided 2026-10-05. The spike in `gpui-spike/` proved the shell can be drawn
by gpui-ce: layer shell, transparent floating layout, 60 fps entrance with no
one-frame dips, fcitx5 Vietnamese, per-card blur on swayfx and niri, HiDPI,
43 MB resident. Windows builds and renders (DirectX, popup window, region
clip); blur there still needs a GPU machine. Details in `gpui-spike/README.md`.

This file is the plan for turning that spike into the app, one screen at a
time. The Tauri build keeps shipping until the last milestone.

## Rules

- Tauri stays the release until parity. Bug fixes land there. The port grows
  beside it.
- Any fix needed by both goes into a core crate or the shared backend crate
  (below), never copied.
- macOS is the design reference. The Tauri frontend is the behaviour reference
  for everything Linux or Windows specific (focus, blur, launching, IME).
- Stay on the gpui API surface shared with upstream gpui: `div`, `Styled`,
  `uniform_list`, `img`, `svg`, `StyledText`, `with_animation`,
  `EntityInputHandler`, `WindowKind::LayerShell`. All confirmed present in
  gpui-ce 0.2.2.
- Performance first. One quad per card face, `uniform_list` for rows, no
  per-keystroke relayout, blur region committed only when bounds change.
- Mimic is the default, not the ceiling. Where the webview was working
  around WebKitGTK, or gpui offers a plainly better way, note it in
  `PORT-GAINS.md` during the slice: shipped, open, or for macOS.

## Code layout

```
apps/linows/
  src/           Tauri frontend, frozen once M1 lands
  src-tauri/     Tauri binary, thin shim over backend/
  backend/       new: Tauri-free library, every command body
  gpui/          new: the gpui-ce app, grown from gpui-spike/
```

### Backend extraction (do first, on main, no behaviour change)

Of the 59 Rust files under `src-tauri/src`, 36 mention Tauri. The coupling
is thin almost everywhere:

| File | Tauri mentions | What they are |
| --- | --- | --- |
| `main.rs` | 47 | app setup, plugins, hotkey, tray, window events |
| `commands.rs` | 33 | `#[tauri::command]` wrappers, `State<AppState>` |
| `files.rs` | 22 | commands plus `AppHandle` for dialogs |
| `process.rs` | 12 | commands |
| `qactions/mod.rs` | 10 | commands |
| `clipboard.rs` | 8 | commands plus `emit` |
| everything else | 1 to 7 | one command attribute, one `State` |

Every platform module under `platform/linux` and `platform/windows`, all
`qactions/controls`, `highlight/{tokenizer,lang,html}`, `crash.rs` and
`clipimage.rs` are already Tauri-free and move as they are.

The split:

1. Move command bodies into `backend/` as plain functions taking
   `&AppState`. `commands.rs` keeps the attribute and one call per function.
2. Events (`index-ready`, `health-changed`, `config-reload`) become a
   `HostEvents` trait the backend calls. Tauri implements it with `emit`,
   gpui with a channel into the app.
3. Window effects (`set_window_effect`, `set_blur_region`, layer shell
   positioning) stay out of backend; both hosts own their window.

Done when the Tauri build has no logic left in `commands.rs`, clippy and fmt
pass, and the deb and nix builds are byte-for-byte equivalent in behaviour.

## Platform axis

macOS is not part of this. `apps/macos` stays SwiftUI and reaches the core
crates through `bridge/ffi`, as today. The gpui app is linows: Linux and
Windows in one binary, the same pair the Tauri build covers. There is no
third app and no per-platform screen; the split lives in three tiers that
already exist:

| Tier | Platforms | Where | Rule |
| --- | --- | --- | --- |
| core crates | Linux, Windows, macOS | `core/` | engine, matching, answers, qactions catalog, todo, lunar. Anything all three need goes here |
| backend | Linux, Windows | `backend/platform/{linux,windows}` | launching, icons, clipboard, focus return, hotkey, autostart, process, update. `cfg(target_os)` at the module edge, one shared trait surface |
| gpui host | Linux, Windows | `gpui/host/{linux,windows}.rs`, `gpui/blur/{wayland,windows}.rs` | window kind, bounds, decoration, blur region, toggle IPC. Picked with `cfg_attr(path)` as the spike does |

Screens sit above all three and compile once. What differs per platform:

| Concern | Linux | Windows | Tier |
| --- | --- | --- | --- |
| Window | layer shell on wlroots, niri, KDE, Hyprland; xdg toplevel on GNOME; X11 toplevel on i3 and X sessions | `WS_POPUP` topmost tool window, restyled after create | host |
| Position | compositor centres the layer surface; toplevels recentre after show on tiling WMs, before show on GNOME and KDE | `Bounds::centered` | host |
| Blur behind | `ext-background-effect-v1` or KDE blur over the protocol; swayfx via `layer_effects` in the user's config | DWM transient backdrop plus `SetWindowRgn` | host |
| Toggle IPC | D-Bus `Toggle` plus the Unix socket | loopback TCP now, named pipe later | host |
| Hotkey | `wayland_shortcut.rs` (portal, kglobalaccel, Hyprland, niri), X11 grab, GNOME extension | `RegisterHotKey` in `launcher_hotkey.rs` | backend |
| Focus return, paste | `window_focus`, `wlr_focus`, `kde_focus`, `plasma_focus`, `gnome_ext`, `virtual_keyboard` | `window_focus`, `keysynth` | backend, moves as is |
| IME | text-input-v3, fcitx5 verified | TSF through gpui_ce_windows, unverified | gpui |
| Clipboard | reads via arboard, file lists via wl-copy or xclip. Writes use GTK today and must move to gpui's clipboard for text and wl-copy or xclip for files | `windows/clipboard.rs`, moves as is | backend |
| Fonts | fontconfig, Adwaita Sans then the system UI font | DirectWrite, Segoe UI | gpui |
| Packaging | deb, AppImage, nix | nsis | CI |

Four Tauri modules are GTK or WebKit bound and do not move:
`platform/linux/layer_shell.rs`, `gpu.rs`, `blur.rs` and the write half of
`clipboard.rs`. Their gpui replacements are the host tier above plus the
clipboard change. Everything else under `platform/` is already free of both.

Each milestone builds and runs on both platforms before it closes. The
Windows VM covers screens and input; only the GPU-bound checks (blur, 60 fps
motion) wait for a real machine, and those are M8's job.

## Screen inventory

What the Tauri build has today, with the backend it calls and the gpui piece
that replaces it. Line counts are the current JS, for sizing only.

### Shell, always mounted

| Piece | Tauri source | Backend | gpui |
| --- | --- | --- | --- |
| Window host | `main.rs`, `platform/linux/layer_shell.rs`, `blur_wayland.rs`, `windows/effects.rs` | hotkey, single instance, D-Bus toggle, CLI | spike `main.rs` + `host/{linux,windows}.rs` + `blur/` already do layer shell, popup, region. Add X11 toplevel for GNOME and i3, D-Bus `Toggle`, `--toggle` CLI, hide on focus loss |
| Query field | `search.html`, `smoothcaret.js` 93 | none | spike `search.rs`. Add selection, Ctrl+A/W/Left/Right word jumps, paste, placeholder, breadcrumb for levels |
| Running apps strip | `running-apps.js` 194 | `list_running_apps`, `activate_running_app`, `get_icon` | row of `img` tiles in the top bar, Alt+digit |
| Picked count | `picked.js` 108 | `get_icon` | badge in the top bar |
| Banner, health notice | `banner.js` 53, `health.js` 81 | `get_health_issues`, health event | toast with fade, sticky notice, dismiss persisted in config dir |
| Confirm bar | `confirm.js` 61 | none | inline yes/no row, Y/N keys |
| Hint bar, footers | `app.js` hint strings | none | spike hint chips. Per-screen hint text from one table |
| Layout | `layout.js` 261 | `apply_layout` | classic framed vs floating gap. Spike does floating only; add the gap 0 case (one panel, hairline divider) |
| Theme | `theme.css`, `liquid.css`, `theme-defaults.js` | `get_config` | `theme.rs`: 8 presets + custom + Liquid surface axis, resolved from config at show. Face colours stay one quad each |
| Motion | `motion.css`, `motion.js`, keyframes in seven component sheets | none | `motion.rs` has the numbers and the rules. Entrance done. The rest is listed under Motion inventory below, per milestone |
| Blur region | `blur.js` 92 | `set_blur_region` | done in spike (`mark_cards`, rounded staircase). Fix the frost-before-fade nit by scaling region alpha with the entrance |

### Motion inventory

Everything the webview animates, with where it lands in the port. Only the
entrance is in place; the rest is one pass of its own (M7) once every screen
is ported, since each is an effect on an element that has to exist first.
The rules come from M3: gpui 0.2.2 has no element scale and no transform, so a box can
only be resized, which relays out the text in it every frame and makes labels
shimmer. So in the port things fade, and move by whole pixels on the settle
curve (`motion::rise`), never on the CSS spring: its overshoot in whole pixels
is a one pixel bounce at the end. An `svg` can scale through
`with_transformation`, so a glyph may still bounce or zoom; an `img` cannot.
Durations and delays stay the CSS numbers. Both switches that turn motion
off (`animations_enabled` in the config, and the desktop's reduce-motion
preference on Linux, read the way `platform.js` reads it) collapse every
duration to zero; the caret blink and the AI spinner stay, as the CSS keeps
them.

| Animation | Tauri source | Port | Milestone |
| --- | --- | --- | --- |
| Shell arrive (fade plus scale 0.965) | `motion.css` shell-arrive | fade only | done |
| Bar spawn (fade plus 8 px rise, spring) | shell-spawn | fade plus whole pixel rise, settle curve | done |
| Launchpad cascade (fade, 10 px rise, scale 0.985, 35 ms stagger) | `superactions.css` ctl-tile-in | fade plus rise, stagger kept, no scale | done |
| Glyph bounce riding the cascade | glyph-bounce | `svg` scale through `with_transformation`, same delay as its tile | M7 |
| Tile press pulse (keyboard activation) | ctl-press | opacity dip on the tile face, 180 ms | M7 |
| L slot crossfade when the source changes | ctl-slot-fade | fade of the new body, 240 ms | M7 |
| Pomo progress bar width glide | ctl-slot-bar-fill | width of a plain quad, 900 ms linear; no text in it, so it may resize | M7 |
| Placeholder slide in (spring, delayed) | placeholder-slide-in | fade plus whole pixel slide | M7 |
| Running apps strip slide in, staggered | strip-slide-in | fade plus whole pixel slide per tile, same stagger | M7 |
| Results list ease in when leaving the launchpad | `results.css` results-list-in | fade plus 6 px rise of the card, once per switch, never per keystroke | M7 |
| Selection pill glide (translate and height) and the title shift | `results.css`, motion.css glide | the pill is its own quad under the rows: animate its top and height by whole pixels; the text block moves by whole pixels | M7 |
| Selection gain: icon zoom and pill stretch | row-icon-gain, pill-gain | pill stretch drops (it resizes a quad over text); icon zoom only for glyph icons through `svg` scale, none for pictures | M7 |
| Banner in | `banner.css` banner-in | fade plus rise, 200 ms; the M3 notice chip grows into it | M7 |
| Confirm bar in | `confirm.css` confirm-in | fade plus rise, 180 ms | M7 |
| Smooth caret: glide between positions, blink | `caret.css` | glide by whole pixels, 105 ms; blink is an opacity loop that stays with motion off | M7 |
| AI spinner | `ai-card.css` ai-spin | `svg` rotation through `with_transformation`, stays with motion off | M7 |
| Pomo card, controls, session list fades, chevron rotate, field reject shake | `commands.css` | fades as they are; chevron through `svg` rotation; the shake is a whole pixel nudge | M7 |
| Speed gauge sweep | `speed.js` | drawn each frame from the value, no CSS to port; the dial requests its own frames and stops under reduce motion | done |
| Hide | none, the window hides at once | same | done |
| Motion off switches | `platform.js` data-motion | one `motion::enabled()` read at show, every duration zero when false | M7 |

### 1. Home (empty query)

Launchpad bento from `superactions.js` (1469) + `launchpad-grid.js` +
`qactions.js` (357). Catalog driven from `core/qactions`
(`launchpad_layout`). Tiles: Clock, L slot (Pomo > Todo > Clock with lunar
date), Bluetooth, Wi-Fi, Theme, Keep Awake, Mic, Screensaver, Restart, Shut
Down (press again to confirm), Battery, Weather, Now Playing (transport
buttons, Alt+P).

Backend: `launchpad_layout`, `launchpad_tile_values`, `launchpad_warnings`,
`quick_action_state`, `quick_action_apply`, `weather_current`,
`now_playing_current`, `now_playing_command`, `lunar_date`, `todo_list`.

gpui: spike `bento()` draws eleven static tiles with the stagger. Replace with
catalog-driven tiles, one `Tile` entity per control holding its state, a 1 s
clock timer, 2 s Now Playing poll while shown, mnemonics as key bindings.

### 2. Search results and preview

The daily path. `results.js` (578), `preview.js` (1122), `search.js` (609),
`levels.js` (151), `rowactions.js` (268), `actionmenu.js` (301),
`sourceblocks.js` (380), `ai-answer.js` (263), `ai-answer-card.js` (137).

Row anatomy: icon or thumbnail, title, path, kind pill, pick check, source
row variant, web suggestion variant, instant answer variant.

Preview kinds: app meta (version, last used), file meta, folder listing,
highlighted text, clipboard text, clipboard image, process (live CPU), source
block, web suggestion, calc, web URL.

Backend: `search`, `record_usage`, `open_path`, `open_elevated`,
`reveal_path`, `get_icon`, `get_file_meta`, `get_app_version`, `list_folder`,
`highlight_file_cmd`, `highlight_shell_cmd`, `source_rows`,
`source_preview`, `source_block(s)`, `perform_block`, `tool_actions`,
`perform_tool_action`, `copy_files_to_clipboard`, `trash_paths`,
`classify_url`, `recent_urls`, `record_url_hit`, `web_suggestions`,
`calc_inline`, `instant_*`, `duckduckgo_answer`, `wikipedia_answer`,
`definitional_entity`.

gpui: `uniform_list` for rows (icons via `img` from the path
`platform::icons` already resolves, thumbnails from `Arc<Image>` bytes),
preview as a second card with its own scroll handle, highlighted code as
`StyledText` with `TextRun`s. The highlight crate emits HTML today; add a
runs output next to `highlight/html.rs` so both frontends share the
tokenizer. Row actions and action menu as `anchored` + `deferred` popovers.
Levels reuse the breadcrumb slot in the field.

### 3. Prefix modes

Catalogued in `catalog.js`, spelled in step with `core/engine modes.rs`:
`a"` apps, `f"` files, `d"` folders, `r"` regex, `rc"` recent, `ps"`
processes, `c"` clipboard, `ci"` clipboard images, `t"` translate, plus `"`
and `:` discovery menus and the quick folders. Translate card is
`translate.js` (159).

Backend: `search` with mode, `get_clipboard_history`, `get_clipboard_images`,
`clipboard_image_data_url`, `delete_clipboard_*`, `copy_clipboard_image`,
`paste_into_focused_app`, `clipboard_paste_blocker`, `search_processes`,
`kill_process`, `process_detail`, `process_cpu`, `translate`.

gpui: same list and preview with different row and preview variants. Mode
state lives in one enum on the launcher, not scattered booleans.

### 4. Command screens (`/`)

`screens/commands/`: index (list of commands), calc (48), shell (38), sys
(54), kill (276), todo (956), pomo (959, with the rodio music player), speed
(724, gauge animation). Hints per screen in `app.js`.

Backend: `eval_calc`, `run_shell_command`, `get_system_info`,
`search_kill_targets`, `kill_process`, `todo_list`, `todo_save`,
`scan_music_folder`, `pick_folder`, `music_*`, `speed_test`, `local_ipv4`,
`system_uptime`.

gpui: one `Screen` enum, each screen its own entity rendered in the results
slot. Todo needs a multi-line editor with undo (gpui `EntityInputHandler`
again, with a history stack). Speed gauge is an arc drawn with `svg` or a
`canvas` style element; check what 0.2.2 offers before choosing. Pomo keeps
the music thread as is, it is already Tauri-free.

### 5. Settings, help, update

`settings.js` (1636) + `settings.html` (788): tabs Appearance, Shortcuts,
Advanced. `shortcutrecorder.js` (148), `update_widget.js` (308),
`help.html` (181).

Backend: `get_config`, `set_config`, `reset_config`, `list_fonts`,
`pick_folder`, `pick_image`, `set_autostart`, `get_autostart`,
`set_cli_path`, `get_cli_path`, `list_candidate_drives`,
`force_index_refresh`, `hotkey_check`, `launcher_hotkey_state`,
`launcher_hotkey_set_active`, `get_install_method`, `get_lookapp_version`,
`start_windows_update`.

gpui: a settings screen with its own scroll handle, controls built once
(toggle, select, slider, text field, folder list, colour swatch). Help is a
static overlay from the same catalog the key bindings come from, so it
cannot go stale.

## Milestones

Ordered so the port is usable as a daily launcher after M1 and gains screens
without ever losing one.

### M0 Foundation

Backend extraction above (landed as #546). Move `gpui-spike/` to `gpui/`,
drop the `LOOK_SPIKE_*` knobs in favour of config, pin gpui-ce (decision 1
below), load `.look.config`, resolve theme and layout from it, D-Bus
`Toggle`, `--toggle` and `--hidden` CLI, single instance, global hotkey
through the existing `wayland_shortcut.rs` and `launcher_hotkey.rs`, hide on
focus loss, crash hook. Keep `tools/capture.sh` and `tools/pace.sh`.

Status 2026-10-05: `gpui/` is a workspace member (not a default one, built
with `-p look-gpui`), wired to the backend through `Host` and
`LauncherWindow`. Engine search, open, usage, index refresh on show, theme
and layout from config, the Wayland hotkey and D-Bus service, `--toggle`
over that service, the crash hook: done and seen running on sway. Still on
gpui-ce 0.2.2. Left for later milestones: hide on focus loss and the hotkey
on Windows (M8), query retention (M1), autostart registration (M8).

Done when: `lookapp-gpui` starts hidden on login, Alt+Space summons it with
the configured theme, Esc hides it, pacing stays at 16.7 ms, on niri and in
the Windows VM.

### M1 Search and launch

Wire `look-engine` (`search_scored`, usage recording, index refresh on show),
real rows with icons, open and reveal, hint bar, prefix modes `a" f" d" r"`,
quick folders, web URL rows, calc inline. No preview yet.

Done when: the user can replace the Tauri build for app and file launching
for a day. This is the first dogfood point and the moment `src/` freezes.

Status 2026-10-05: built. Rows come from one query function (engine with the
scopes, pinned folders, calc, live and remembered URLs, merged as the
webview merges them), icons resolve off the UI thread through the backend's
cache, the list is a `uniform_list` with the selection kept in view, the
field has selection, word moves and paste, text copies own the clipboard
through gpui, and the query survives a short dismissal. Verified on sway
with screenshots of a plain query, the three scopes, arithmetic, a URL and
a pinned folder. Awaiting the day of dogfooding.

### M2 Preview column

All preview kinds from screen 2, the highlight runs output, folder listing,
clipboard image rendering, process live view.

Status 2026-10-05: built for every row kind M1 produces. `highlight_runs`
sits beside the HTML output in the backend; the gpui `Preview` entity
reads the backend after a 120 ms dwell, caches per row, and draws the app,
file (image or code), folder, calculator and URL panels beside the list in
the split layout. Verified with screenshots on sway. The clipboard, image
and process previews arrive with their modes in M4, since no row of those
kinds exists before then.

### M3 Home launchpad

Screen 1 in full, including confirm-to-press and Now Playing transport.

Status 2026-10-06: built. `gpui/src/launchpad.rs` is one `Launchpad` entity
drawn from the backend's `launchpad_layout` (the user's super-actions.toml or
the catalog default), each tile placed by its own cell and sized by
`ROW_H` (the macOS 76). Roles: the L slot (Todo over Clock, lunar date in the
corner, task rotation), toggles with the accent wash, Battery with the uptime
fallback, Weather, action tiles with the mnemonic tint, Mic as a mute flip,
Restart and Shut Down armed on the first press and fired on the second (3 s
disarm), Now Playing from MPRIS polled every 2 s while shown with the
previous, play or pause, next transport, and user tiles with their readings,
lines, state wash and inlined icon (an SVG is served through the asset
source so it takes the tile colour). Adapter reads and presses run on the
background executor behind a token so a late read never undoes a press. Alt
plus a letter fires the tile's mnemonic from the launcher. Outcomes, adapter
reasons and drawing warnings arrive as a `Notice` event the launcher shows
as a chip over the bottom edge; M4's banner takes that event over. The
`super_actions_enabled` setting gates the bento as the webview does.
Verified with screenshots on sway against the user's drawing and the catalog
default. Waiting on M5: the Pomo slot and the internal music player in Now
Playing, which need the command screens.

Motion note, same day: the tiles shimmered while landing. A 60 fps recording
showed every label shifting half a pixel from frame to frame, because the
tile box was being resized to stand in for the CSS scale and the arrive
animation inset the whole window, so the text relaid out each frame. gpui
0.2.2 has no element scale, so the port drops the scales: tiles keep their
final size and rise by whole pixels (`motion::rise`), the arrive is a fade.
The CSS spring went too: its overshoot in whole pixels was a one pixel
bounce as each tile landed, so the bar and the tiles use the settle curve.
Measured per frame after the change: the label descends and stays put.

### M4 Modes and row tooling

Screen 3, picked multi-select, row actions, action menu, levels, source
blocks, AI and instant answers, running apps strip, banner and health,
confirm bar.

Status 2026-10-06: built. One `Mode` enum (`modes.rs`) replaces the
webview's booleans: `"` and `:` menus, `t"`, `c"`, `ci"`, `rc"`, `ps"`, each
with its rows, keys, hint and empty state; `query::run` answers every mode
off the UI thread. New previews for clips, copied images (decoded from the
backend's data URL), processes (facts at once, CPU on Enter) and the
clipboard tips. The translate panel takes the whole content row. The banner
(`banner.rs`) is a card above the bar: toasts, and the sticky health notice
with dismissals kept in the state directory (`health.rs`); the launchpad's
notice chip became a banner call. Picks (`picked.rs`): Ctrl+P, Shift+Enter,
Ctrl+Shift+P, the panel in the preview column, the count badge in compact,
files copied to the clipboard. Row actions (`actions.rs`): the Ctrl+K menu
with tool names resolved by core, a block's `then` targets and the actions
declared with `applies`, the question asked in the menu, Ctrl+E, Ctrl+T,
Ctrl+F through core's tools. The confirm bar (`confirm.rs`) backs emptying
the trash and hiding an app. Levels (`levels.rs`) descend from a target that
lists, with the breadcrumb in the bar, Escape back with the query and
selection restored, and `{parent.*}` through the ancestors the backend
spells. Source rows wear their block's name and declared icon
(`blocks.rs`). The running apps strip (`running.rs`) sits at the bar's right
end with Alt+digit. Web suggestions append below the local rows and the
answer card (`answers.rs`) streams DuckDuckGo, Wikipedia and the instant
providers over the list. Every backend call goes through `bg::fetch`; the
clipboard in particular deadlocks the main loop if called on it. The probe
socket takes `key <keystroke>` so screenshots can reach what a chord
reaches. Verified with screenshots on sway for each mode, the menu, picks,
the confirm bar, the strip, the answer card and a Branches level.

Not in this milestone: the `:cmd` rows and `:cmd <args>` trigger wait for
the command screens (M5), as does the internal music player in Now Playing;
the Quick Actions section of a settings row's preview (`qactions.js`) and
Ctrl+O; answer images (gpui's image-from-URL needs an HTTP client); the
AI two-column layout, since the card sits over one list here; Ctrl+Shift+Enter
elevated launch (Windows); Ctrl+H help, Ctrl+Shift+, settings and
Ctrl+Shift+; reload (M6).

### M5 Command screens

Screen 4, in the slices issue #552 draws, one PR each: shell and kill
(with the frame), calc and sys, pomo, todo, speed.

Status 2026-10-06, slice 1 (`port/m5-shell-kill`): the frame and the first
two panels. `commands/mod.rs` holds the frame and each command is a file beside it (`shell.rs`, `kill.rs`): the catalog sidebar with
Ctrl+1 to 7, Tab and Shift+Tab, the active panel beside it in one framed
card with the hint as its footer, and the panel's own field, which the
keys edit while the screen is up (`field()` picks the box, focus follows in
render). Entry points: Ctrl+/, the `:` menu's Enter, the `:cmd <args>`
inline trigger with the args prefilled, so `lookapp shell ls` lands the
same way. Shell runs on the background executor and prints the output or
the error in the feedback line. Kill lists the apps on entry, filters the
loaded names at once and replaces them with the backend's fuzzy result
after 140 ms (ports and pids included), confirms with Y and N in the bar
pinned under the list, and reloads after the banner. The five panels still
to come show their name and "arrives in a later PR" in place of a body.
Verified with screenshots on sway.

Slice 2 (calc and sys, same working tree, 2026-10-06): `commands/calc.rs`
evaluates as it is typed through core's calculator, an error leaving the
last result standing, and Enter copies the result with a banner;
`commands/sys.rs` loads the backend's sections on entry under the read-only
header bar and lays them out as label and mono value rows with a gap
between sections. Three panels left: pomo, todo, speed.

Slice 3 (pomo, 2026-10-06): the timer, the plan and the music live in
`gpui/src/pomo.rs` for the process, since the window is rebuilt per summon;
the plan persists in `~/.look/config` under the keys macOS writes
(`pomo_sessions` as `type:minutes:name`, `pomo_timer_style`,
`pomo_music_folder`), so both shells share one. `commands/pomo.rs` is the
panel: header, the face drawn each frame on a `canvas` (gpui 0.2.2 has one,
the earlier note was wrong) as a ring, a dial or digits over a bar, the
state-coloured controls, the style row behind the gear, the session list
with inline name and minute fields (own `SearchInput` per open field, Enter
or Tab commits, a blank name or a minute count outside 1 to 120 is refused
with a banner), and the music card with its transport and a folder picked
through the desktop portal. Not through gpui's path prompt: that exports the
focused surface as the dialog's parent over xdg_foreign, and a layer-shell
surface is no xdg toplevel, so the compositor answers `zxdg_exporter_v2`
error 0 and the Wayland connection dies (found on first use, 2026-10-06).
`pick.rs` calls the portal's file chooser directly with no parent on Linux
and keeps gpui's prompt elsewhere; the fix for upstream is a
`window_identifier` that returns `None` for layer-shell windows. Space, R
and P as the webview binds them. Five idle seconds while running fade the
panel to the ring alone, the sidebar with it; any key, click or scroll
restores it. A thread ticks the running session each second, window or no
window, and sends the phase notifications through `notify-send`; the
panel and the slot read the clock projected to the instant, so a summon
shows the true time at once (first cut ticked from the panel, so the clock
sat still while hidden and showed stale for the first half second). A
music thread advances the track when one ends, panel open or not. A hide
from a command screen keeps that screen, with its box text, for the next
summon however long the launcher was away (the user's call, 2026-10-06:
a running timer must be where it was left; the macOS source drops the
screen with the query once retention expires, so macOS may want the same
change); the query alone obeys the retention window. The launchpad's slot now shows a
running session first (countdown, phase and session, progress bar), and
Now Playing shows the internal player while it plays, driving it from the
tile. Verified with screenshots on sway: idle, running, faded, all three
faces, the music card playing, the slot.

Slice 4 (todo, 2026-10-06): the data lives in `gpui/src/todo.rs` for the
process (tasks by day, undo and redo to 50 steps, the saved revision), as
the webview keeps its edits across a hide; `commands/todo.rs` is the
screen. Tasks page: the search bar (the frame's box, folded with
`look_matching::normalize_for_search` and matched as a subsequence over
task names and the card titles), the toolbar (done today, Tasks | Stats,
Add date + N, Save with its dirty dot), a card per day newest first with
the progress ring, the title, the relative phrase, complete-all and
clear-all, rows with the checkbox, EXTENDED and OVERDUE tags, the remove
mark on hover, and the dashed add row (box open on click, Enter keeps it
open until the day holds three unfinished, Esc closes). Double-click a
name to rename it. Ctrl+Z and Ctrl+Shift+Z step the history, Ctrl+N flips
the page, Ctrl+S saves the whole set back and says so in the capsule at
the foot (a failure keeps its Retry). Stats page: the week and month
donuts, the streak with its seven dots, the thirty-day trend line, the
year heatmap and the four insight tiles, all drawn on `canvas`; the page
scrolls when taller than the panel, as the webview's content box does.
Ids come from `uuid` v4 (already in the tree through gpui). Verified with
screenshots on sway against a seeded scratch database (`LOOK_DB_PATH`):
tasks, a search, stats. The results footer carries the webview's
`.hint-todo` widget in the hint's last slot while a query shows (the
resting home hides the hint bar in the webview too): the tally, the
unfinished tasks in a bubble on hover, the screen on click. The slot tile
sets the pomo line heights on its tally and next task, which had been
clipping the task. Heatmap cells show the webview's data-tip bubble on
hover (`Sun, Oct 4: 2/2 done`): the canvas inserts a hitbox and a mouse
move handler that stores the hovered cell on the panel, and the bubble is
a `deferred` window-anchored element so the scrolling stats column cannot
clip it. Hint strings differ from the webview on purpose until
M6: Search says `Ctrl+F: Reveal` and Translate `Esc: Clear` where the
webview says `Ctrl+H: Help`, since the help screen is not there yet.

Slice 5 (speed, 2026-10-06): `gpui/src/speed.rs` holds what outlives the
window, as the webview's module state and the macOS controller do: the
last reading, the run in flight on a `look-speed` thread (the measurement
has no cancel, so a second one is never started beside it and the next
open rejoins it), the LAN address, and the dial's motion. The reading
persists through the backend (`netspeed::remember`/`last_reading`, a JSON
file under the state dir, public address dropped), where the webview used
localStorage and macOS UserDefaults. `commands/speed.rs` is the panel:
addresses line with click-to-copy chips, the masked WAN address and its
eye toggle (`E`), the gauge, the legend, the verdict and the carrier line,
`R` to rerun. The gauge is one `canvas` drawing the macOS SpeedGaugeView's
orbit: dashed rings, ticks and labels, the latency pulse, two comets whose
tails are stroked arcs with a dot at each end for the round caps gpui's
paths cannot set. Each paint advances the motion by the wall clock and
asks for the next frame with `request_animation_frame`, so the dial runs
only while the panel is on screen; under `cx.reduce_motion()` it snaps to
the reading and draws once. The elapsed count ticks once a second. The
canvas text and circle helpers moved out of the todo panel into
`commands/draw.rs`; the pomo's `Face::text` still has its own copy to fold
in later. Reduce motion is gpui's flag; wiring `animations_enabled` to it
is M7's.

Size, same day: the release binary was 33.7 MB. cargo-bloat put the text at
26.5 MiB with std, zbus, gpui and its Linux platform, naga and wgpu as the
largest. A separate `release-gpui` profile (`opt-level = "s"`,
`codegen-units = 1`, inheriting release) brings it to 22.7 MB with the
pace probe still at 16.7 ms per frame; `opt-level = "z"` with
`panic = "abort"` reached 18.0 MB but the backend re-raises a thread's
panic in `dbus.rs`, so unwinding stays. Build with
`cargo build -p look-gpui --profile release-gpui`; the tools default to that
path. The Nix package picks the profile up in M8.

### M6 Settings, help, update

Screen 5, shortcut recorder, update widget. Sliced like M5, one PR each:

1. Settings shell, the controls, the Appearance tab, Ctrl+Shift+; reload.
2. Advanced tab: web answers, background image (an image picker beside
   `pick::folder`), indexing fields and folder lists, log level, startup,
   fresh config behind the confirm bar, the Linux rendering switches that
   still mean something without WebKitGTK.
3. Shortcuts tab: a key catalog with labels and descriptions that the hint
   strings and the help overlay read from, the shortcut recorder for
   `launcher_hotkey` (`hotkey_check`, `launcher_hotkey_state`,
   `launcher_hotkey_set_active`), the pending notice.
4. Help overlay on Ctrl+H from the same catalog, and the update widget
   (the release check moves into the backend, where the webview fetched
   GitHub from JS).

Slice 1 (2026-10-07): `gpui/src/settings.rs` is the screen, a sibling of
`command_mode` on the launcher rather than a `COMMAND_ENTRIES` entry, so
it is in no sidebar and on no Ctrl+digit. Ctrl+Shift+, toggles it from
anywhere (a command screen is left first, as the webview does), Esc
leaves it. It draws in the command screen's frame, which became
`Launcher::screen(body, hint)` for both; the card keeps the blur region
and the hint footer. While it is up and no box edits, a plain
`FocusHandle` holds the keys, so typing cannot reach the hidden search
field through the input handler. `gpui/src/controls.rs` is the control
set, built once: section header, divider, labelled row, switch, slider,
select, segmented, text box, button, hint and readout, at the sizes of
`settings.css`. The slider is a `canvas` with a hitbox; its drag lives
on the window's mouse events so the thumb follows a pointer that leaves
the track, and the drag state sits on the screen so a release anywhere
ends it. The select's list and the font box's suggestions are
`deferred` absolute children, so the scrolling body cannot clip them,
and a press outside closes them.

The screen runs the way the macOS ThemeStore runs, not the way the
webview did: every control edits a working copy of the config that the
theme follows on the same frame (`Theme::from_entries` resolves a key
map, `theme::install` makes it the one every render reads), nothing
touches the file until Save Config writes every key the screen owns in
one `set_config`, and Esc discards back to the file. The webview wrote
each key as it changed; on this shell those writes ran on separate
threads and raced each other, one read-modify-write losing the other's
key, which was the first thing dogfood hit. Picking a preset copies its
triplets into the keys and keeps its name, so the sliders tune it under
its own palette, as `style.apply(to:)` does; only Custom drops the name,
and the palette then derives from the user's colours the way
`ThemeStore+Appearance` falls back (text tones dimmed from the font
colour, neutral fills, the font colour as the accent, the ink on it by
luminance). Reading the file follows the same store: the named preset is
the base and the `ui_*` keys win over it, where the webview ignored them
under a preset. Kindle and Liquid still bring their opacities, Blur
Style its opacity. Font Name is a box over `SearchInput` with an
`fc-list` suggestion list (`list_fonts`, fetched once per open), Enter or
a click away commits, empty means `system-ui`, and `fonts::ensure_family`
loads what was picked. `settings_blur_multiplier` scales the tint alpha
of the frame while the screen is up. Running Apps and Super Actions show
in the split layout only; Animations is saved and read by M7. The
Ctrl+Shift+; reloads the
theme, the fonts, the engine and, if up, the working copy, with the
webview's banner.

Dogfood on the seated layout, same day: the preview column scrolled as a
whole, header included, since each body had the webview's fixed cap and
the column is a hint bar shorter there. The panel no longer scrolls and
every body shrinks to the leftover height before anything else moves,
the webview's `.preview-slot` and the macOS ScrollView. The launchpad's
seams are macOS `Launchpad.gap`, a fixed 8 under its 8 of air, not the
inner gap. The hint band has the webview's 1.2 leading in place of
gpui's phi, and sits on the card edge as macOS's card footer does; the
seated bar has the hint bar's 4 14 2. Preview info rows elide in the
middle (`elide.rs`, a shaped line probed by binary search), the macOS
`.truncationMode(.middle)`, so a path keeps its root and its file name.

The slider takes its release in the frame of the press as well as while
dragging: the release listener used to exist only once the state said
dragging, which is the frame after the press, so a quick click never
got its release and the slider stayed armed, following the pointer to
whatever was clicked next. The probe socket grew `click <x> <y>` and
`drag <x1> <y1> <x2> <y2>`, dispatched through `Window::dispatch_event`,
which is how that was reproduced and checked without touching the
user's pointer.

The Inner Gap slider at 0 exposed that the shell drew
floating tiles only (the Platform axis row above asked for the gap 0
case): `Theme::floating` now gates it, and at 0 with a query the home is
the classic framed panel of `.launcher-window`, the bar seated as its
header over a hairline (`Theme::divider`), the list and the preview
split by one, the hint and copyright in a full-width bar under them;
the resting home keeps the frosted bar in both modes, as the spec says.
Verified with screenshots on niri through the
probe socket: the Appearance tab, the tab cycle, Esc back to home, the
reload banner, gap 0 with results and with a translate query, and the
preset, slider, toggle, Save sequence with the file diffed before and
after. The clicks (sliders, selects, the font box) wait for
dogfood: the socket sends keys only.

Slice 2 (2026-10-08): `gpui/src/settings/advanced.rs` is the tab, the
screen now a module directory. Web answers (`ai_enabled`, which the
answer card reads on Save, no reload), Background, Indexing, Privacy &
Logs, Startup, Config File, and on Linux Rendering with the one switch
that still means something here, `ui_compositor_blur`: the frost region
goes to the compositor only while it is on, and the card shadows come
back while it is off, as the macOS material has one or the other. The
WebKitGTK switches (`disable_gpu_compositing`, `disable_blur_effect`)
are not on this shell. Launch at login and, on Windows, Add to PATH read
the OS on open and go back to it on Save; the drive chips are Windows
only too. The number boxes are `SearchInput` fields like the font box
(`FieldKind`), Enter, Tab or a click away snapping the value into range,
a blank to the default. The folder lists are chips with a remove mark,
through `config_list` (the CSV contract moved out of the launcher, where
hiding an app used it). Create Fresh Config asks in the confirm bar,
which now draws on the screen frame too and takes the keys before the
settings screen does; on yes the defaults are written, the engine, the
theme, the fonts and the working copy read the file again.

The background picture: `bgimage.rs` decodes `ui_bg_image` off the UI
thread, scales it to the size it is drawn at, blurs it once with the
`image` crate (gpui has no image filter) and keeps that one bitmap; every
card and the seated panel paint their own slice of it through
`paint_image(card, picture, radius)`, the card as the clip and the whole
picture as the placement, so the floating tiles read as one picture cut
apart by the gaps, as the webview's per-tile `::before` did from a
ResizeObserver. Opacity is the layer's `opacity()`. While the exact
bitmap is still being built the last one for the file stands in, so a
blur drag does not blank the cards. Image Layout is the webview's four
(`BgLayout`). `pick::image` is the portal file chooser with an image
filter, beside the folder one. Verified with screenshots on sway through
the probe socket, which grew `wheel <x> <y> <lines>` for the scrolling
body: the tab top to bottom, the picture across the two cards and across
the seated panel, blur dragged to 15, the number box, the frost switch
and its hint, the fresh config question and the reset file diffed.
Picking a file through the portal waits for dogfood, as does typing in
the number boxes: the socket's keys carry no character.

Slices 3 and 4, help half (2026-10-08): `gpui/src/shortcuts.rs` is the
key catalog, the macOS `ShortcutCatalog` with the webview's wording:
groups by topic (Main, Prefixes, Command), each entry an id, its keys
and what it does; the prefix rows derive from the `"` menu's entries and
the Ctrl+N row from `COMMAND_ENTRIES`, so neither can go stale. Every
footer hint is built from it (`shortcuts::hint` over `Piece`s: a catalog
id with a short label, a bare key, or a phrase), held in `LazyLock`
statics so the `&'static str` signatures stay; the Search hint says
`Ctrl+H: Help` again now that help exists. Settings > Shortcuts
(`settings/shortcuts.rs`) is the catalog flat with the two footers, the
launcher key's row a capsule that records on click where the key is
rebindable (Windows, `hotkey::CONFIGURABLE`): the next chord goes
through `hotkey_check`, a refusal shows after the row, Esc stops, Reset
shows while off the default, the pending notice sits at the top and Save
writes `launcher_hotkey` with the rest. Nothing releases or rebinds the
key around the recording yet: the gpui binary binds no hotkey on
Windows until M8, and Linux is not rebindable. `help.rs` is the help
screen in the command screen's frame on Ctrl+H from anywhere but
settings: the title row carries the All / Main / Prefixes / Command
capsules as the macOS screen does, Tab and Shift+Tab step them, the
body scrolls, Esc or Ctrl+H closes, and a plain `FocusHandle` holds the
keys. Verified with screenshots on sway: the Search footer, help on All
and on Prefixes, scrolled, back to the results with the query kept, and
the Shortcuts tab top and middle.

Update widget, same day: `backend/src/update.rs` is the release check,
GitHub's latest release through the answers crate's HTTP path (curl, 15 s)
against the running version, draft and prerelease ignored, the notes
address kept only when it is GitHub's own, a dismissed version remembered
in the state dir and overridden by a manual check; the webview fetched
this from JS and kept the dismissal in localStorage. `gpui/src/update.rs`
is the block, the macOS `AppUpdateStatusView` in one state for both
places: the version line with the status after it and the Check for
Updates pill, then the banner with Update (Windows installer builds,
through `start_windows_update`, the process leaving on success), Release
Notes and Dismiss, and the hint for the install method (Scoop's command
with a Copy pill, the installer note, or the README link). It sits under
the help title row and as About at the foot of the Advanced tab. The
version is `LOOK_VERSION`, read by `gpui/build.rs` from
`tauri.conf.json`, the file the release tag is checked against, until the
Tauri shell retires. Verified on sway: the help header, a live check
answering the latest version, and the About section.

Settings header dropped, same day, the user's call ahead of macOS: no
title row, the tabs at the top edge, Save Config and its Saved pill
floating at the card's bottom right (`Settings::save_float`, a
`deferred` absolute element the frame adds as the card's own child,
since gpui anchors an absolute child to its parent; the body keeps
clearance under its last row), the footer only where a tab has a hint
and without the copyright (the frame takes an optional trailing element
in the copyright's place), the close hint gone. The tabs take the
control fill, no border: the panel fill had read as the card. The
update block's pills are the app's buttons, and About flows the check
button after the version rather than at the row's end, where it stacked
on the floating Save; the help header keeps it at the end.

### M7 Motion pass

The Motion inventory above, in one go: every effect lands on an element that
exists by then, under the whole-pixel rules. Closes with a frame strip per
animation, as M3's entrance was checked.

Status 2026-10-08: built, on three gpui mechanisms rather than ported
keyframes. `with_animation` keyed by an element id carries every
entrance, restarting when the id first appears: the results card rising
in keyed by a switch count (once per launchpad-to-results, never per
keystroke), the banner by its sequence, the confirm bar by its asking,
the running apps strip per tile with the slide stagger, the launchpad
glyph bounce per tile (an `svg` scaling through its transformation,
same delay as its tile), the L slot's crossfade keyed by its source, the
selected row's glyph zoom keyed by the row, the pomo chevron's quarter
turn keyed by open, and a refused pomo field's danger flash keyed by a
count. `Transition` carries every value that glides to a goal: the
selection pill, now one quad painted under the rows from a `canvas`
that reads the list's scroll offset, its top gliding on the settle
curve and jumping when the rows are new (the webview's `.results-selection`
with `is-instant`); the caret's x; the pomo bar's fill between ticks.
`request_animation_frame` from a custom element drives the clock-based
effects: the placeholder's slide in, the caret's blink (solid for half a
second after a move, then the soft dip), the answer spinner (a loader
glyph turned by the wall clock while streaming), and the pressed tile's
dip, which multiplies into the cascade's opacity. The switches are one
switch: `motion::sync` hands `animations_enabled` and the desktop's
GNOME animations setting (`platform::reduce_motion`, read once at
start) to `App::set_reduce_motion`, which every `with_animation` already
honours by landing on its last frame; transitions get a 1 ms length; the
caret blink and the spinner keep going by design. `LOOK_MOTION_SCALE=8`
stretches every duration for a frame strip taken with grim, which is too
slow for a 300 ms glide otherwise. Verified that way on sway: the pill
between two rows mid-glide, the summon with the bar first, tiles fading
and rising in order, strip icons sliding in and the placeholder fading
in after them, the banner and the confirm bar. Still only built, not
seen: the glyph bounce, the press dip, the slot crossfade, the bar
glide, the chevron and the reject flash, since each needs a state the
probe cannot reach without typing or a running timer. Not ported: the
pill stretch, dropped by the rules. Found on the way: the floating Save
button painted over the confirm bar; it now steps aside while a
question is up.

Dogfood, same day: the picture icons had no zoom (only glyphs were
wired) and stepped through whole-pixel sizes once they had one, which
read as a glitch; a picture now resizes on fractional pixels inside its
fixed chip, since an image has no text to shimmer, while a glyph keeps
the svg transform, which gpui applies as a sprite matrix without
rasterising again. A held Tab retargeted the 300 ms glide every repeat
and a wrap sent the pill across the whole list; the pill now never
starts more than one row from its goal, so it trails a held key by one
row and lands within the glide of the last repeat. The macOS
`titleShift` came in after all, as an offset rather than padding: the
selected row's text sits 4 px right, sliding out on the glide as the
row takes the selection and back as the selection leaves (`shifted`,
keyed on the row and a change count packed into one id), so nothing
reflows and the row the selection left animates too. The icon zoom
then felt quick and sharp against macOS: its hump peaked and fell
straight off the reversed settle curve. It is now the macOS pair,
`motion::zoom`: a cubic ease-out to 1.24 over 110 ms, then a damped
spring back (0.3 s response, 0.6 damping, the SwiftUI numbers) that
undershoots a touch and settles over the rest of 710 ms.

Frost-before-fade, the nit from the motion table, same day: the
compositor frosts a region the moment it is set, so a card sat frosted
before it had faded in. The regions now wait for the entrances: the
bar's, the cards' and the panels' until the bar's spawn has landed, the
tiles' until the cascade has, with frames asked for until both have
(`arm_frost`, a frame flag the marking reads; reduce motion lands them
at once). Built and compiled only: this sway session speaks no blur
protocol, so niri is where it shows. With that, M7 is complete.

swayfx, same day: it frosts through its own `layer_effects` on the whole
layer surface, not a protocol, so the region the app sends means nothing
there. The spike's recipe holds (`blur enable`, then `layer_effects
"lookapp"` with `blur enable`, `blur_ignore_transparent enable` and
`shadows disable`, one effect per `swaymsg` call), but the port drew card
shadows whenever no blur protocol answered, and swayfx frosted their
alpha into a halo around every card. `wm::is_swayfx` (the version reply
carries `sway_original_version`) now counts as able to frost, so with
Blur behind window on the shadows go, as on niri and KWin, and the
Advanced hint names the swayfx effects. Verified on the user's swayfx
0.6: per-card frost, sharp gaps, no halo.
Then the summon read as tiles going black before turning normal: with
`blur_ignore_transparent`, swayfx frosts a pixel only once its alpha
passes a threshold, so a card fading in from nothing sat dark over the
sharp desktop until the frost popped in. Under swayfx frost
(`blur::surfaces_fade`) the shell arrive, the bar's spawn, the tile
cascade and the results lift keep their motion at full opacity, so the
frost is there from the first frame. Other compositors keep the fades.

Banner placement, same day, the user's call on the recommendation: a
toast no longer sits above the bar, where every one moved the bar down
and back. On the home and results it floats over the body just under
the bar, `deferred` and absolute, with an opaque face so the row under
it cannot read through, and its rise in; the bar and the rows stay put.
The sticky health notice stays until dismissed, so it takes a place in
the flow between the bar and the body instead. The command and settings
screens have no bar and keep the banner above their card. Verified on
sway for the toast over results; the sticky notice's fake-issue hook is
debug-only, so it waits for a debug run.

### M8 Parity and flip

Windows on a GPU machine (blur, 60 fps motion). X11 toplevel on GNOME
and i3, KDE and Hyprland blur through the protocol module (already in
`blur/wayland.rs`), autostart, Nix package, deb, AppImage, CI. Screenshot
parity against the reference set (below) on every screen. Then rename the
binary to `lookapp`, retire `src/` and `src-tauri/`, update the wiki and
`docs/architecture.md`.

## Reference screenshots

Before `src/` freezes, capture every screen of the Tauri build on niri into
`gpui/refs/`: home, results with preview for each preview kind, each prefix
mode, each command screen, each settings tab, help. Use the D-Bus toggle and
`query` over the socket, never synthesized keys. Each milestone closes by
putting its gpui shot next to the reference.

## Decisions to take

1. gpui-ce pin. 0.2.2 on crates.io has no `inactive_frame_interval`, so sway
   needs exclusive keyboard focus and niri on-demand. Git main has it plus
   the Parley text move. Pinning a git rev works with `cargoLock.lockFile`
   in Nix. Suggest: pin git main now, move to the next release when it
   comes.
2. Directory names `backend/` and `gpui/`.
3. Markdown in AI answers and translate. Either a small inline renderer
   (headings, bold, code, lists) or plain text. Suggest plain text first,
   renderer in M4 if it hurts.
4. GNOME Wayland without layer shell: xdg toplevel plus the GNOME extension
   for focus, as the Tauri build does today. No new work, but it must be
   tested in the GNOME VM at M8.
5. Linux clipboard writes. GTK is gone, so text goes through gpui's
   clipboard and file lists through wl-copy or xclip with the
   `x-special/gnome-copied-files` MIME. Needed by M1 for copy path.

## Risks

- Fork bus factor on gpui-ce. Accepted on 2026-10-05.
- Text field features the spike lacks: selection, word jumps, paste, undo.
  Needed by M1 for the field and by M5 for todo.
- Lists: hover, wheel scroll, scrollbar, keep-selected-visible. gpui has
  `uniform_list` and `ScrollHandle`; the behaviour still has to be built.
- Fonts off GNOME: Adwaita Sans is not everywhere. Fall back through
  fontconfig to the system UI font, same list the CSS had.
- Windows blur is unverified until a GPU machine runs it.
- Two frontends for several weeks. The backend crate is what keeps that from
  doubling every bug fix.
