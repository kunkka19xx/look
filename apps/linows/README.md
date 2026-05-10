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

**For dev in VM (nixos)**

```bash
WEBKIT_DISABLE_COMPOSITING_MODE=1 cargo tauri dev
```
