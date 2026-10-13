# linows on gpui-ce

The launcher shell drawn by gpui-ce, shipping as `lookapp` since 2026-10. The Tauri shell in `../src-tauri` is the backup. Everything that is not drawing comes from `../backend` (`linows_backend`), the same crate the Tauri commands call. The design reference is the macOS app; `PLAN-GPUI-NATIVE.md` holds the rules and what is still open.

## Running

From `apps/linows` inside `nix develop`:

```sh
cargo run --release            # opens at once
cargo run --release -- --hidden
printf toggle | socat - UNIX-CONNECT:$XDG_RUNTIME_DIR/lookapp.sock
```

`lookapp --toggle` reaches a running launcher the way the hotkey does, over the `com.look.Desktop` D-Bus call; `lookapp reload-config` goes over the control socket. On Windows the socket is `127.0.0.1:47811`, and `make app-run` from the repo root wraps cargo in the VS 2022 Build Tools environment.

Commands on the socket: `toggle`, `show`, `hide`, `reload-config`, `quit`, `query <text>`, `key <keystroke>`, `click <x> <y>`, `drag <x1> <y1> <x2> <y2>`, `wheel <x> <y> <lines>`. `query` fills the field without typing, `key ctrl-<` reaches what a chord reaches, `click`, `drag` and `wheel` what the pointer reaches, in window pixels. Probes go through this socket, never through wtype or ydotool.

## Measuring

A second instance runs beside the launcher in use when `LOOK_CONTROL_SOCKET` (Linux, keep the path short) or `LOOK_CONTROL_PORT` (Windows) names its own endpoint. With `LOOK_PACE_PROBE=1` it also logs a timestamp per frame and each search's time, and takes neither the hotkey nor the D-Bus name.

```sh
tools/capture.sh gpui 8        # eight summon cycles, scored for one-frame dips
tools/pace.sh exclusive        # steady-state frame pacing
```

`LOOK_MOTION_SCALE=8` slows every motion for frame strips.

## Findings

Kept because the numbers still describe this code. All measured on the live session: screenshots with grim, `WAYLAND_DEBUG=1` traces, `tools/capture.sh`.

### 2026-10-05, swayfx 0.6 on NixOS, RADV

- Layer shell: `WindowKind::LayerShell`, centred by the compositor, one `get_layer_surface` per show and a clean `destroy` per hide. Hiding closes the window; `QuitMode::Explicit` keeps the process.
- Entrance motion at 60 fps over eight summon cycles: zero isolated dips, no stale frame at the end of any animation. The WebKitGTK 2.54 flash does not exist here.
- Input method: `zwp_text_input_v3` with preedit kept as marked text; Vietnamese typed through fcitx5. Adwaita Sans through cosmic-text renders the diacritics.
- Cost: 43 MB PSS (72 MB RSS), 23 threads, 0 CPU hidden. The Tauri build was 355 MB PSS, 232 MB of it WebKit's web and network processes.
- Frame pacing: first measured at 33 to 50 ms, because gpui throttles an inactive window to 30 fps and swayfx withdraws on-demand focus in the same batch it grants it. `KeyboardInteractivity::Exclusive` fixes both: 16.7 ms per frame, max 17. niri holds on-demand focus and keeps that mode (16.6 ms mean, max 18). gpui-ce git main adds `WindowOptions.inactive_frame_interval`, which would lift the cap outright.
- Blur behind on swayfx has no protocol: `swaymsg blur enable` plus `layer_effects "lookapp"` with `blur enable`, `blur_ignore_transparent enable`, `shadows disable`. Card shadows must be off under it, since any alpha above zero gets blurred.
- Blur behind on niri, KWin, Hyprland and Mutter 51: the backend's `blur_wayland` adopts gpui's display and surface (`Backend::from_foreign_display`, `ObjectId::from_ptr`), the region is the card bounds gathered with `on_children_prepainted` once per prepaint, corners as a staircase inside the arc. Per-card frost, sharp gaps, 16.7 ms per frame while the region moves. The frost lands a few frames before the cards fade in.
- HiDPI on niri at scale 2 and 1.5: fractional-scale-v1 with viewporter, native-resolution glyphs, blur region aligned, live scale change re-renders, 16.7 ms per frame.

### 2026-10-05 and 2026-10-11, Windows 11 26200 VM

- Builds with no extra setup (gpui_ce_windows: DirectX, DirectWrite). Release binary 10 MB.
- Window is `WindowKind::PopUp` centred with `Bounds::centered`; `host::decorate` restyles gpui's captioned window to a bare `WS_POPUP` so gaps and corners are see-through.
- Blur dropped: `SetWindowRgn` edges are binary, so rounded corners stair-step, and DWM cannot shape its backdrop any other way. Clear glass keeps gpui's anti-aliased corners. The VM's QXL adapter has no D3D, so blur was never seen on real hardware anyway.
- Hotkey through `global-hotkey` 0.8: rebound on save and config reload, released while the recorder listens. Focus from a background process needs `window_focus::bring_to_front`, which attaches to the foreground thread's input queue for the call.
- Window kept across hides (`host::KEEP_WINDOW`): a new window per summon left about 100 MB private behind under WARP. The hide parks an empty root, presents it, then `SW_HIDE`s.
- Cost, release build, 20 summons with searches: 60 MB private, 89 MB working set, 1 process, 0 CPU hidden. Summon to first frame p50 6 ms, search 1.7 to 3.2 ms. Tauri 0.7.2 on the same VM: 144 MB private, 396 MB working set, 7 processes.

### 2026-10-13, dev profile

Unoptimised gpui paints text at about 1 ms per run, so Settings > Shortcuts drew at 110 ms a frame in a debug build while release sat at vsync. `[profile.dev.package]` in `../Cargo.toml` now builds gpui-ce, gpui_ce_wgpu, taffy and cosmic-text at opt-level 2; debug frames are 16.7 ms again.

## Notes

- Keep to the surface shared with upstream gpui: `div`, `Styled`, `with_animation`, `EntityInputHandler`, `WindowKind::LayerShell`.
- gpui-ce 0.2.2 on crates.io ships no Linux backend; `gpui_ce_platform` 0.1.0 brings `gpui_ce_linux` (Wayland, X11, wgpu 29, cosmic-text). Git main has since moved text to Parley, and the x11 feature cannot be turned off through the platform crate, so the dev shell links libxcb too.
- gpui's `Hsla` and `Rgba` are palette types in this version; `palette` is a direct dependency only for `IntoColor`.
