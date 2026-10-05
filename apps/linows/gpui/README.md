# linows on gpui-ce

The launcher shell drawn by gpui-ce instead of WebKitGTK. It grows screen by
screen beside the Tauri shell in `../src-tauri`, which keeps shipping until
this one reaches parity; the plan and the milestones are in
`../PLAN-GPUI-PORT.md`. Everything that is not drawing comes from
`../backend` (`linows_backend`), the same crate the Tauri commands call.

It started as a two day spike. What that spike measured (60 fps with no
one-frame dips, fcitx5 input, per-card blur, 43 MB resident) is recorded
below under Findings, since the numbers still describe this code.

## What is in here

| Path | Role |
| --- | --- |
| `src/main.rs` | Startup, the backend hooks (`Host`, `LauncherWindow`), the window, the control socket |
| `src/launcher.rs` | The shell: top bar, launchpad bento, results list, hint bar, keyboard |
| `src/search.rs` | Query field. `EntityInputHandler` carries fcitx5 preedit and commits |
| `src/theme.rs` | The look, resolved from `~/.look/config` the way the webview resolves it |
| `src/motion.rs` | The motion numbers, copied from the CSS |
| `src/host/{linux,windows}.rs` | Window kind, bounds, decoration, the control socket per OS |
| `src/blur.rs`, `src/blur/windows.rs` | Blur regions: the backend's Wayland protocols on gpui's surface, or a window region on Windows |
| `tools/capture.sh`, `tools/dips.py` | The one frame glitch detector from `docs/webkit-254-flicker/` |
| `tools/pace.sh` | Steady-state frame pacing from the `LOOK_PACE_PROBE=1` log |

Dependencies: `gpui-ce` 0.2.2 and `gpui_ce_platform` 0.1.0 (wayland feature
on Linux), plus `async-channel`, `anyhow`, `palette`, `chrono` and
`raw-window-handle`, all already in gpui's tree. The Wayland blur crates come
through the backend.

## Running

The crate is a workspace member but not a default one, so it is built by name
from `apps/linows` inside `nix develop`:

```sh
cargo run -p look-gpui --release            # opens at once
cargo run -p look-gpui --release -- --hidden
printf toggle | socat - UNIX-CONNECT:$XDG_RUNTIME_DIR/look-gpui.sock
```

`lookapp-gpui --toggle` reaches a running launcher the way the hotkey does,
over the `com.look.Desktop` D-Bus call. That name is shared with the Tauri
shell, so only one of the two owns Alt+Space at a time: quit the installed
Look to hand the key to this one.

Commands on the socket: `toggle`, `show`, `hide`, `query <text>`, `quit`.
`query` fills the field directly so a results screenshot needs no typing.

Keys: Esc hides, Enter opens the selected row, Ctrl+Enter searches the web
for the query, Up/Down, Tab/Shift+Tab or Ctrl+N/P move, Ctrl+F reveals the
row, Ctrl+C copies its path (or the field's selection), Ctrl+V pastes,
Ctrl+A selects all, Shift with the arrows extends a selection, Ctrl with
Left/Right jumps words, Ctrl+Backspace or Ctrl+W deletes one, Ctrl+U clears.

Capture eight summon cycles and score them, or measure steady-state pacing.
Both start a measurement instance beside the launcher in use: with
`LOOK_PACE_PROBE=1` it logs a timestamp per frame and each search's time,
takes no hotkey or D-Bus name, and answers on the socket
`LOOK_CONTROL_SOCKET` names instead of the default one.

```sh
tools/capture.sh gpui 8
tools/pace.sh exclusive
```

## What is wired, and what is not

Wired through the backend: the engine (search with the `a" f" d" r" rc"`
scopes, usage, index refresh on show), the pinned home folders, the inline
calculator, URL rows (live and remembered), row icons resolved off the UI
thread, opening rows (apps, files, URLs, with the focus dance each desktop
needs), Ctrl+F reveal, Ctrl+C copy path, Ctrl+Enter web search, the query
field's selection, word moves and paste, query retention across a short
dismissal, text copies through gpui's clipboard, the theme and layout keys
of the config, the global hotkey and D-Bus service on Wayland, the crash
hook.

Not yet: the preview column, the launchpad tiles (the bento is still
placeholder tiles), the `c" ci" t" ps"` modes and the `"` and `:` menus,
file and image copies (the backend shells out to wl-copy or xclip for
those), the clipboard monitor, autostart registration, hide on focus loss
on Windows, a Windows hotkey. Each is a milestone in the plan.

## Findings, 2026-10-05, swayfx 0.6 on NixOS, RADV

Checked by running the binary on the live session, screenshots with grim,
`WAYLAND_DEBUG=1` traces, and `tools/capture.sh gpui 8`.

- Layer shell. `WindowKind::LayerShell` with no anchor is centred by the
  compositor at 1008x672, keyboard interactivity exclusive, one
  `get_layer_surface` per show and a clean `destroy` per hide. Hiding closes
  the window; the process stays because `QuitMode::Explicit` is set (the
  Linux default quits with the last window).
- Transparent window. `WindowBackgroundAppearance::Transparent` with the
  floating layout: the desktop shows through every gap, the cards keep their
  own borders, radii and shadows, corners drawn by gpui.
- Entrance motion. Panel fade plus inset (gpui 0.2.2 has no element scale, so
  the 3.5 percent arrive scale is an animated padding), top bar rise, eleven
  staggered tiles. Eight summon cycles recorded at 60 fps: zero isolated
  dips, no stale frame at the end of any animation. The WebKitGTK 2.54 flash
  is a compositor bug there, and nothing like it exists here.
- Input method. `zwp_text_input_v3` enable, content type, cursor rectangle
  and commit on focus; the field keeps marked text for preedit. Vietnamese
  typed through fcitx5 by hand.
- Fonts. Adwaita Sans through cosmic-text, Vietnamese diacritics render.
- Cost. 43 MB PSS (72 MB RSS), 23 threads, hidden or shown, 0 CPU hidden.
  The Tauri build is 355 MB PSS, of which the WebKit web and network
  processes are 232 MB.
- Frame pacing. First measured at 33 to 50 ms per frame. The cause: gpui
  throttles an inactive window to 30 fps while animating, and swayfx gives an
  on-demand layer surface keyboard focus then withdraws it in the same batch,
  so the window never counted as active and never got keys either.
  `KeyboardInteractivity::Exclusive` fixes both: 16.7 ms per frame, max 17.
  niri holds on-demand focus, so it keeps that mode (`tools/pace.sh`:
  16.6 ms mean, max 18). gpui-ce git main adds
  `WindowOptions.inactive_frame_interval`, which would lift the cap outright.
- Blur behind, swayfx. No protocol there: `swaymsg blur enable` plus
  `layer_effects "lookapp"` with `blur enable`, `blur_ignore_transparent
  enable`, `shadows disable` (one effect per call). Card shadows have to be
  off under it, since any alpha above zero gets blurred.
- Blur behind, niri (and KWin, Hyprland, Mutter 51). The backend's
  `blur_wayland` on gpui's raw handles: `Backend::from_foreign_display` and
  `ObjectId::from_ptr` adopt gpui's display and surface because gpui_ce_linux
  builds wayland-backend with `client_system`. The region is the card bounds
  gathered with `on_children_prepainted`, sent from the root once per
  prepaint, deduped, with the rounded corners as a staircase of rows inside
  the arc (`BlurRect::rounded`). Per-card frost, sharp gaps, 16.7 ms per
  frame with the region moving during the entrance, six hide and show cycles
  re-attach cleanly. One rough edge: the frost lands a few frames before the
  cards fade in.
- HiDPI, niri at output scale 2 and 1.5: fractional-scale-v1 with viewporter,
  logical size kept, native-resolution glyphs, blur region aligned, live
  scale change re-renders, 16.7 ms per frame.

## Findings, 2026-10-05, Windows 11 26200 VM

- Builds with no extra setup: gpui_ce_windows (DirectX, DirectWrite). Release
  binary 10 MB.
- Window is `WindowKind::PopUp` (topmost tool window, no taskbar entry)
  centred with `Bounds::centered`. gpui creates it with style 0, which
  Windows promotes to a captioned overlapped window, so DWM paints frame,
  shadow and the theme backdrop behind the transparent pixels.
  `host::decorate` restyles it to a bare `WS_POPUP` at its client rect; then
  gaps and corners are truly see-through. Card radii, card shadows and
  Vietnamese text render as on Linux.
- Commands go over `127.0.0.1:47811` (std has no named pipe server).
- Cost: 41 MB private, 45 MB working set, 16 threads, 0 CPU hidden.
- Blur behind: the card region goes to `SetWindowRgn`, so the window only
  exists where the cards are and the gaps stay sharp. The blur itself is the
  DWM system backdrop (`DWMSBT_TRANSIENTWINDOW` plus
  `DwmExtendFrameIntoClientArea`), the call `window-vibrancy` makes for the
  Tauri build. gpui's own `WindowBackgroundAppearance::Blurred` uses the
  legacy accent API and paints the whole window black. Regions are binary,
  so the corner staircase is not anti-aliased.
- Blur not verified: the VM's QXL adapter has no D3D, so DWM disables blur
  for every app. Needs a GPU machine.

## Notes

- Keep to the surface shared with upstream gpui: `div`, `Styled`,
  `with_animation`, `EntityInputHandler`, `WindowKind::LayerShell`.
- gpui-ce 0.2.2 on crates.io ships no Linux backend; `gpui_ce_platform` 0.1.0
  brings `gpui_ce_linux` (Wayland, X11, wgpu 29, cosmic-text). Git main has
  since moved text to Parley, and the x11 feature cannot be turned off through
  the platform crate, so the dev shell links libxcb too.
- gpui's `Hsla` and `Rgba` are palette types in this version; `palette` is a
  direct dependency only for `IntoColor`.
