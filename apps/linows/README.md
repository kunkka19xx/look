# linows

Desktop app for **Windows + Linux**, drawn by gpui-ce. The binary is `lookapp`.

The macOS SwiftUI app (`apps/macos/`) is the design source of truth; this app replicates its look, feel, and feature set.

The shell lives in `gpui/` (see its README); everything that is not drawing lives in `backend/`.

## Architecture

```
apps/linows/
  gpui/                  The shell: window, screens, keys (lookapp)
    src/
      main.rs            Entry point, window, control socket, global hotkey
      launcher.rs        The launcher view: bar, results, preview, footer
      theme.rs           The look, resolved from ~/.look/config
      settings/          Ctrl+Shift+, : the settings screen (Extend tab, lookbook browser)
      commands/          /calc, /kill, /pomo, /shell, /speed, /sys, /todo
      host/              Per-OS window plumbing (layer shell, X11, Windows)
  backend/               Everything that is not drawing
    src/
      lib.rs             What the shell calls; host.rs is what it needs back
      state.rs           Engine cache, scoped-refresh watcher (non-recursive
                         file roots, noise filter, debounce + cooldown, off-thread reindex,
                         RAII slot guard). See tools/perf/WATCHER_PERF.md for benchmarks.
      search.rs          Query to rows, over core/engine
      sources.rs         Declared blocks: what a row is, what Enter runs, previews
      sources_watch.rs   Watches ~/.look/sources and ~/.look/connectors, reloads on change
      connectors.rs      Starts and stops the processes behind connect blocks
      lookbook.rs        lookbook catalog fetch, preview and install
      launch.rs          Open, reveal, focus an existing window, run as admin
      config.rs          Config get/set (.look/config persistence)
      files.rs           File meta, version, clipboard copy, music scan, folder pick
      clipboard.rs       Clipboard history monitor
      shell.rs           Shell command execution
      calc.rs            Calculator (functions, constants, !, %, commas)
      music.rs           Background music player (rodio, ALSA)
      process.rs         Running apps list + kill
      sysinfo.rs         System info (OS, memory, CPU, battery, uptime, disk)
      todo.rs            Daily tasks (shared look-todo store in look.db)
      netspeed.rs        Speed test (shared look-netspeed crate) + local IPv4 lookup
      translate.rs       Translation
      weburl.rs          URL-like query detection + opened-URL history (shared core)
      autostart.rs       Autostart management
      qactions/          Quick Actions adapters (Bluetooth, Wi-Fi, …)
      platform/          Platform-specific code
        linux/           Icons, WM detection, Wayland shortcuts, GNOME ext, compositor blur, …
        windows/         Icons, effects, drives, known folders, …
        shared.rs        Shared platform helpers
  icons/                 App icons shipped by the deb, AppImage and Nix builds
```

## Why This Exists

The previous Windows app was built with WinUI3/C#. It didn't match the macOS app's look and feel; the UI felt inconsistent across platforms. A Tauri app replaced it with a web-based frontend that could look identical on Windows and Linux, and the gpui shell in `gpui/` then replaced that, drawing the same design on the GPU with no browser engine in the process. The macOS SwiftUI app stays the single design reference throughout.

Both migrations are complete, and both of the old shells live on in git history: the WinUI3 app under `apps/windows/`, the Tauri shell under `apps/linows/src-tauri/` and `apps/linows/src/`.

## Key Decisions

- Direct Rust crate deps (no FFI/cdylib); the shell calls the `backend/` crate, which calls the core engine directly
- Own Cargo workspace (not part of core/ workspace), so a bare `cargo build` builds the shell and its backend
- macOS design language: dark, blurred, rounded, minimal
- Audio playback via `rodio` (Rust)

## Linux Desktop Environments

| Environment   | Distro | Status   | Notes                                                   |
| ------------- | ------ | -------- | ------------------------------------------------------- |
| GNOME Xorg    | NixOS  | Tested   | Full support                                            |
| GNOME Wayland | Ubuntu | Tested   | Dock icon visible while Look is open (see Known Issues) |
| GNOME Wayland | NixOS  | Tested   | Full support                                            |
| GNOME Wayland | Arch   | Tested   | Full support                                            |
| i3 X11        | NixOS  | Tested   | No system settings entries                              |
| Sway          | NixOS  | Tested   | No system settings entries                              |
| Hyprland      | Arch   | Tested   | No system settings entries                              |
| KDE Plasma    |        | Untested |                                                         |

**System settings** (Appearance, Wi-Fi, Sound, etc.) are shown when the desktop's settings app is detected: `gnome-control-center` on GNOME and its relatives, KDE System Settings (`systemsettings`) on Plasma, where only the modules your install ships are listed. On i3, sway, or minimal distros without either, these entries are skipped.

## Optional Dependencies

| Package           | Used for                                                                                                                                                  | Fallback                                                                                        |
| ----------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------- |
| `curl`            | Web answers, translation, `/speed`, the lookbook catalog                                                                                                  | Answers stay empty; speed test reports "curl is not available on this system"                   |
| `xclip`           | Copy files to clipboard (X11)                                                                                                                             | Shows "Copy failed" banner                                                                      |
| `wl-clipboard`    | Copy files to clipboard (Wayland)                                                                                                                         | Shows "Copy failed" banner                                                                      |

Text clipboard (copy path, clipboard history) works without any external tools. File clipboard (copy a file to paste into a file manager) needs one of the above.

The overlay window (above fullscreen windows, out of the taskbar and Alt+Tab) needs no package: gpui speaks `wlr-layer-shell` itself wherever the compositor implements it (KDE Plasma, sway, niri, Hyprland). GNOME/mutter does not, so there Look is an ordinary toplevel that GNOME places.

```bash
# Debian/Ubuntu
sudo apt install xclip                    # X11
sudo apt install wl-clipboard             # Wayland

# Fedora
sudo dnf install xclip                    # X11
sudo dnf install wl-clipboard             # Wayland

# Arch
sudo pacman -S xclip                      # X11
sudo pacman -S wl-clipboard               # Wayland
```

## Build

[BUILDING.md](BUILDING.md) is canonical for prerequisites, Windows `vcvars`, and packaging. The short version:

```bash
cd apps/linows
cargo run             # development
cargo run --release   # production profile
cargo build --locked  # backend + gpui shell
```

**NixOS dev shell** (uses the repo-root flake.nix, found from any subdirectory):

```bash
nix develop
nix develop -c cargo run
nix build .#default -L   # the packaged build, from the repo root
```

## Testing on Ubuntu VM (virt-manager)

**Build .deb on NixOS host** (needs `cargo install cargo-deb`, the same tool CI uses):

```bash
nix develop -c cargo build --release -p lookapp
nix develop -c cargo deb -p lookapp --no-build
# Output: target/debian/lookapp_*.deb
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
scp -O apps/linows/target/debian/lookapp_*.deb kunkka@192.168.122.x:/tmp/
```

**Install on VM:**

```bash
pkill lookapp                              # stop running instance
sudo dpkg -r lookapp && sudo dpkg -i /tmp/lookapp_*.deb
sudo patchelf --set-interpreter /lib64/ld-linux-x86-64.so.2 /usr/bin/lookapp
lookapp                                    # launch from terminal to see logs
```

> **Why?** NixOS builds link against `/nix/store/.../ld-linux-x86-64.so.2` which doesn't exist on Ubuntu/Debian. `patchelf` rewrites it to the standard `/lib64/ld-linux-x86-64.so.2`. Without this, the binary shows `cannot execute: required file not found`. For proper release builds, use Ubuntu/Debian CI or add a patchelf step to the build script.

**VM without GPU** (no `/dev/dri` or virtual GPU driver): wgpu falls back to software GL through Mesa, slower but drawn the same.

**GNOME Shell extension** requires log out/in after first install to load.

**Keyboard shortcuts:**

| Shortcut         | Action                                         |
| ---------------- | ---------------------------------------------- |
| Alt+Space        | Toggle Look window                             |
| Esc              | Hide Look                                      |
| Alt+Shift+Q      | Quit Look                                      |
| Tab/Shift+Tab    | Navigate results                               |
| Enter            | Open selected                                  |
| Ctrl+Enter       | Search web                                     |
| Ctrl+C           | Copy path to clipboard                         |
| Ctrl+F           | Reveal in file manager                         |
| Ctrl+P           | Pick (multi-select)                            |
| Ctrl+Shift+P     | Clear all picks                                |
| Ctrl+/           | Toggle command mode                            |
| Ctrl+Shift+,     | Open settings                                  |
| Ctrl+Shift+;     | Reload config from file                        |
| Ctrl+H           | Help screen                                    |
| Ctrl+Shift+H     | Hide selected app from Look                    |
| Ctrl+Shift+Enter | Run selected app as admin (Windows)            |
| Ctrl+D           | Remove selected clipboard entry (in `c"` mode) |

**Known issues on Ubuntu:**

- GNOME's default Alt+Space (window menu) is auto-disabled by Look on Wayland; restored when Look exits
- **GNOME Wayland: dock icon visible while Look is open.** Mutter has no layer shell, so Look is an ordinary toplevel there, and GNOME's dock lists it until Look hides (Esc / Alt+Space). On compositors with `wlr-layer-shell` the window is an overlay surface and never reaches the dock.

    **Workaround: hide running app indicators from the dock:**

    ```bash
    # Ubuntu Dock
    gsettings set org.gnome.shell.extensions.ubuntu-dock show-running false

    # Dash to Dock
    gsettings set org.gnome.shell.extensions.dash-to-dock show-running false

    # Undo: replace 'false' with 'true'
    ```
