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

| Environment   | Distro | Status   | Notes                                                   |
| ------------- | ------ | -------- | ------------------------------------------------------- |
| GNOME Xorg    | NixOS  | Testing  | Full support                                            |
| GNOME Wayland | Ubuntu | Testing  | Dock icon visible while Look is open (see Known Issues) |
| GNOME Wayland | NixOS  | Testing  | Full support                                            |
| i3 X11        | NixOS  | Testing  | No system settings entries                              |
| Sway          |        | Untested | No system settings entries                              |
| KDE Plasma    |        | Untested |                                                         |

**System settings** (Appearance, Wi-Fi, Sound, etc.) are only shown when `gnome-control-center`
is detected. On i3, sway, or minimal distros without GNOME, these entries are skipped.

## Optional Dependencies

| Package        | Used for                          | Fallback                   |
| -------------- | --------------------------------- | -------------------------- |
| `xclip`        | Copy files to clipboard (X11)     | Shows "Copy failed" banner |
| `wl-clipboard` | Copy files to clipboard (Wayland) | Shows "Copy failed" banner |

Text clipboard (copy path, clipboard history) works without any external tools.
File clipboard (copy a file to paste into a file manager) needs one of the above.

```bash
# Debian/Ubuntu
sudo apt install xclip              # X11
sudo apt install wl-clipboard       # Wayland

# Fedora
sudo dnf install xclip              # X11
sudo dnf install wl-clipboard       # Wayland

# Arch
sudo pacman -S xclip                # X11
sudo pacman -S wl-clipboard         # Wayland
```

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

**Prerequisites on VM** (first time only):

```bash
sudo apt install openssh-server patchelf
```

**Deploy** (from host, run as one script):

```bash
scp -O apps/linows/src-tauri/target/release/bundle/deb/Look_*.deb kunkka@192.168.122.x:/tmp/
```

**Install on VM:**

```bash
pkill lookapp                              # stop running instance
sudo dpkg -r look && sudo dpkg -i /tmp/Look_*.deb
sudo patchelf --set-interpreter /lib64/ld-linux-x86-64.so.2 /usr/bin/lookapp
lookapp                                    # launch from terminal to see logs
```

> **Why?** NixOS builds link against `/nix/store/.../ld-linux-x86-64.so.2` which doesn't
> exist on Ubuntu/Debian. `patchelf` rewrites it to the standard `/lib64/ld-linux-x86-64.so.2`.
> Without this, the binary shows `cannot execute: required file not found`.
> For proper release builds, use Ubuntu/Debian CI or add a patchelf step to the build script.

**VM without GPU** (no `/dev/dri`): the app auto-detects this and sets
`WEBKIT_DISABLE_GPU=1` + `WEBKIT_DISABLE_DMABUF_RENDERER=1` to prevent WebKitGTK segfaults.

**GNOME Shell extension** requires log out/in after first install to load.

**Keyboard shortcuts:**

| Shortcut     | Action                  |
| ------------ | ----------------------- |
| Alt+Space    | Toggle Look window      |
| Esc          | Hide Look               |
| Alt+Shift+Q  | Quit Look               |
| Ctrl+Shift+, | Open settings           |
| Ctrl+Shift+; | Reload config from file |
| Ctrl+H       | Help screen             |

**Known issues on Ubuntu:**

- GNOME's default Alt+Space (window menu) is auto-disabled by Look on Wayland; restored when Look exits
- **GNOME Wayland: dock icon visible while Look is open.** Tauri sets `skip_taskbar_hint`
  asynchronously after the GTK window is mapped, so GNOME's dock ignores it. Native GTK apps
  like Ulauncher set this hint in the constructor (before mapping), which works. On X11, the
  hint works correctly. The icon disappears when Look is hidden (Esc / Alt+Space).
  **Contributions welcome** — if you know a way to set GTK hints before Tauri maps the window,
  please open a PR!

  **Workaround — hide running app indicators from the dock:**

  ```bash
  # Ubuntu Dock
  gsettings set org.gnome.shell.extensions.ubuntu-dock show-running false

  # Dash to Dock
  gsettings set org.gnome.shell.extensions.dash-to-dock show-running false

  # Undo: replace 'false' with 'true'
  ```

**For dev in VM (nixos)**

```bash
WEBKIT_DISABLE_COMPOSITING_MODE=1 cargo tauri dev
```
