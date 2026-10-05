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
motion) wait for a real machine, and those are M7's job.

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
| Motion | `motion.css`, `motion.js` | none | `motion.rs` already has the numbers. Entrance done. Add row stagger, screen crossfade, banner fade, hide fade. Honour the animations-off setting |
| Blur region | `blur.js` 92 | `set_blur_region` | done in spike (`mark_cards`, rounded staircase). Fix the frost-before-fade nit by scaling region alpha with the entrance |

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

Backend extraction above. Move `gpui-spike/` to `gpui/`, drop the
`LOOK_SPIKE_*` knobs in favour of config, pin gpui-ce (decision 1 below),
load `.look.config`, resolve theme and layout from it, D-Bus `Toggle`,
`--toggle` and `--hidden` CLI, single instance, global hotkey through the
existing `wayland_shortcut.rs` and `launcher_hotkey.rs`, hide on focus loss,
crash hook. Keep `tools/capture.sh` and `tools/pace.sh`.

Done when: `lookapp-gpui` starts hidden on login, Alt+Space summons it with
the configured theme, Esc hides it, pacing stays at 16.7 ms, on niri and in
the Windows VM.

### M1 Search and launch

Wire `look-engine` (`search_scored`, usage recording, index refresh on show),
real rows with icons, open and reveal, hint bar, prefix modes `a" f" d" r"`,
quick folders, web URL rows, calc inline. No preview yet.

Done when: the user can replace the Tauri build for app and file launching
for a day. This is the first dogfood point and the moment `src/` freezes.

### M2 Preview column

All preview kinds from screen 2, the highlight runs output, folder listing,
clipboard image rendering, process live view.

### M3 Home launchpad

Screen 1 in full, including confirm-to-press and Now Playing transport.

### M4 Modes and row tooling

Screen 3, picked multi-select, row actions, action menu, levels, source
blocks, AI and instant answers, running apps strip, banner and health,
confirm bar.

### M5 Command screens

Screen 4. Order: index, calc, shell, sys, kill, then todo, pomo, speed.

### M6 Settings, help, update

Screen 5, shortcut recorder, update widget.

### M7 Parity and flip

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
   tested in the GNOME VM at M7.
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
