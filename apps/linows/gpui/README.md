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


## Notes

- Keep to the surface shared with upstream gpui: `div`, `Styled`, `with_animation`, `EntityInputHandler`, `WindowKind::LayerShell`.
- gpui-ce 0.2.2 on crates.io ships no Linux backend; `gpui_ce_platform` 0.1.0 brings `gpui_ce_linux` (Wayland, X11, wgpu 29, cosmic-text). Git main has since moved text to Parley, and the x11 feature cannot be turned off through the platform crate, so the dev shell links libxcb too.
- gpui's `Hsla` and `Rgba` are palette types in this version; `palette` is a direct dependency only for `IntoColor`.
- `gpui_ce_wgpu` is **not** the crates.io build. `[patch.crates-io]` in `../Cargo.toml` redirects it to [kunkka19xx/gpui-ce](https://github.com/kunkka19xx/gpui-ce), branch `wgpu-pipeline-cache-crates-io`: crates.io 0.1.0 plus one commit that caches wgpu pipelines per device rather than rebuilding them per window, which Mesa virgl never frees (~4 MB a summon in QEMU/KVM guests). It is the proposed upstream change, so drop the patch once a release ships it. Moving that branch means re-pinning `outputHashes."gpui_ce_wgpu-0.1.0"` in `../nix/package.nix`, or the Nix build fails on a hash mismatch.
