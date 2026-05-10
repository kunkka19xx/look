# linows

Tauri v2 desktop app for **Windows + Linux**. Vanilla HTML/CSS/JS frontend.

The macOS SwiftUI app (`apps/macos/`) is the design source of truth — this app replicates
its look, feel, and feature set using web technologies.

## Architecture

```
apps/linows/
  src-tauri/           Rust backend (Tauri commands, state, platform logic)
    src/
      main.rs          Entry point, plugin registration, global hotkey
      commands.rs      #[tauri::command] handlers (search, open, shell, etc.)
      state.rs         AppState (engine cache, file watchers, index refresh)
      platform.rs      Icon extraction (freedesktop, XDG_DATA_DIRS, .desktop)
      process.rs       Running apps list (match .desktop vs /proc) + kill
      sysinfo.rs       System info (OS, memory, CPU, battery, uptime, disk)
      calc.rs          Calculator (functions, constants, !, %, commas)
      music.rs         Background music player (rodio, ALSA)
    capabilities/
      default.json     Tauri v2 permissions (events, dialog)
  src/                 Vanilla frontend (served by Tauri webview)
    index.html
    css/
    js/
    assets/
```

## Why This Exists

The previous Windows app (`apps/windows/`) was built with WinUI3/C#. It didn't match the
macOS app's look and feel — the UI felt inconsistent across platforms. This Tauri app
replaces it with a web-based frontend that can look identical on Windows and Linux, using
the macOS SwiftUI app as the single design reference.

The WinUI3 app remains in `apps/windows/` (bug fixes only) until this migration is complete.

## Key Decisions

- Direct Rust crate deps (no FFI/cdylib) — Tauri commands call core engine directly
- Own Cargo workspace (not part of core/ workspace)
- ES modules (`<script type="module">`) — no bundler
- CSS custom properties for theming
- macOS design language: dark, blurred, rounded, minimal
- Audio playback via `rodio` (Rust) — WebKitGTK's HTML5 Audio has issues on Linux
- Folder picker via `tauri-plugin-dialog` — cross-platform native dialogs
- Tauri v2 capabilities in `capabilities/default.json` for event/dialog permissions

## Linux Desktop Environments

| Environment    | Status    |
|----------------|-----------|
| GNOME Xorg     | Testing   |
| i3 X11         | Testing   |
| GNOME Wayland  | Testing   |
| Sway           | Untested  |

## Build

```bash
cd apps/linows
cargo tauri dev       # development
cargo tauri build     # production
```

**NixOS dev shell** (uses flake.nix):

```bash
nix develop
cargo tauri dev
cargo tauri build --bundles deb    # build .deb package
```

## Testing on Ubuntu VM (virt-manager)

**Build .deb on NixOS host:**

```bash
nix develop --command cargo tauri build --bundles deb
# Output: src-tauri/target/release/bundle/deb/Look_*.deb
```

**Get VM IP** (on the VM):

```bash
ip addr | grep inet
# Look for 192.168.122.x on enp1s0
```

**Copy .deb to VM** (from host):

```bash
scp -O src-tauri/target/release/bundle/deb/Look_*.deb ubuntu@192.168.122.x:/tmp/
```

**Install on VM:**

```bash
sudo dpkg -r look                          # remove old version
sudo apt install -f                        # install missing deps (xclip etc.)
sudo dpkg -i /tmp/Look_*.deb              # install new version
```

**NixOS-built binaries need patching** (NixOS linker path doesn't exist on Ubuntu):

```bash
sudo apt install patchelf
sudo patchelf --set-interpreter /lib64/ld-linux-x86-64.so.2 /usr/bin/lookapp
```

> **Why?** NixOS builds link against `/nix/store/.../ld-linux-x86-64.so.2` which doesn't
> exist on Ubuntu/Debian. `patchelf` rewrites it to the standard `/lib64/ld-linux-x86-64.so.2`.
> Without this, the binary shows `cannot execute: required file not found`.
> For proper release builds, use Ubuntu/Debian CI or add a patchelf step to the build script.

**VM without GPU** (no `/dev/dri`): the app auto-detects this and sets
`WEBKIT_DISABLE_GPU=1` + `WEBKIT_DISABLE_DMABUF_RENDERER=1` to prevent WebKitGTK segfaults.

**GNOME Shell extension** requires log out/in after first install to load.

**Known issues on Ubuntu:**
- D-Bus activated apps (e.g. Ptyxis terminal) may need 2 launch attempts — first call registers the D-Bus service, second opens the window
- GNOME's default Alt+Space (window menu) is auto-disabled by Look on Wayland; restored when Look exits

**For dev in VM (nixos)**

```bash
WEBKIT_DISABLE_COMPOSITING_MODE=1 cargo tauri dev
```
