# linows — Implementation Tasks

Based on macOS app as source of truth. Organized by phase.

---

## Phase 1: Core Search (MVP)

### Scaffold
- [x] Create Tauri v2 project structure (src-tauri/, src/)
- [x] Cargo.toml with core crate deps (look-engine, look-indexing, look-storage)
- [x] tauri.conf.json (borderless, always-on-top, 860x580)
- [x] flake.nix dev shell with cargo-tauri
- [ ] Build script integration (Makefile targets)

### Backend (Rust)
- [x] `state.rs` — Engine cache (RwLock<QueryEngine>), bootstrap refresh, file watchers
- [x] `commands.rs` — search(query, limit), record_usage(id, action)
- [x] `commands.rs` — open_path(path, kind, id), reveal_path(path)
- [x] `commands.rs` — reload_config(), request_index_refresh()
- [x] `commands.rs` — get_file_meta(path), get_app_version(path), get_home_dir()
- [x] `platform.rs` — Icon extraction (freedesktop theme + XDG_DATA_DIRS, .desktop Icon= parsing)
- [x] App launching — gio launch → gtk-launch → direct spawn, focus existing window via i3-msg/xdotool
- [x] Settings URL handling — settings:// paths routed through xdg-open

### Frontend (HTML/CSS/JS)
- [x] `index.html` — Main window structure
- [x] `css/reset.css` — CSS reset
- [x] `css/theme.css` — CSS custom properties (colors, spacing, typography)
- [x] `css/layout.css` — Window layout, search bar, content area
- [x] `css/components.css` — Result rows, panels
- [x] `js/ipc.js` — Tauri invoke wrapper
- [x] `js/app.js` — Main controller, mode switching
- [x] `js/search.js` — Debounced search (70ms), query → invoke → render
- [x] `js/results.js` — DOM rendering of result rows (icon, title, kind subtitle)
- [x] `js/keyboard.js` — Arrow/Tab/Shift+Tab navigation, Enter to open, Escape to hide, wrap-around
- [x] `js/preview.js` — Preview panel (icon, title, badge, metadata, image preview)

### Window & System
- [x] Global hotkey (Alt+Space) via tauri-plugin-global-shortcut
- [x] Single instance via tauri-plugin-single-instance
- [x] Transparency detection (Wayland/compositor → transparent + rounded corners, X11 bare → solid + square)
- [x] Auto-hide on focus loss (transparent-capable platforms only)
- [x] i3/tiling WM support (floating rule, manual centering)

---

## Phase 2: Preview & Multi-pick

### Screens
- [x] Result preview panel — file metadata (size, modified, path), image preview, app version
- [ ] Picked items panel — list of multi-selected items with remove buttons

### Features
- [x] Quick folders (Desktop, Documents, Downloads, Pictures, Videos, Music)
- [ ] Multi-pick (Ctrl+Click to toggle selection)
- [ ] Clipboard write (picked items as paths + text)
- [x] Reveal in file manager (Ctrl+F)
- [x] Hint bar (bottom status text)

---

## Phase 3: Clipboard & Commands

### Screens
- [ ] Clipboard history view — list of entries with time, char/line count
- [ ] Command mode panel — 4 cards (calc, shell, kill, sys) with shared input
- [ ] Kill confirmation bar
- [ ] Translation panel — input, language buttons, output, copy/browser actions
- [ ] Banner notifications (animated toast messages)

### Features
- [ ] Clipboard history store (in-memory ring buffer, max 10 entries, 30KB each)
- [ ] Clipboard monitoring (platform clipboard listener)
- [ ] `c"` prefix to browse clipboard history
- [ ] Delete individual clipboard entries
- [ ] Command mode toggle (Ctrl+/)
- [ ] Calculator command — expression evaluation (+, -, *, /, %, ^, parens)
- [ ] Shell command — execute and capture output (<800 chars)
- [ ] Kill command — fuzzy process match, terminate with confirmation
- [ ] System info command — CPU, memory, disk, GPU, network
- [ ] Translation (`t"` prefix) — web translation via Rust bridge
- [ ] Language selection (English, Vietnamese, Japanese)

---

## Phase 4: Settings & Themes

### Screens
- [ ] Settings panel (Ctrl+Shift+,) with 3 tabs:
  - [ ] Appearance tab — colors, blur material, font scale, background image
  - [ ] Shortcuts tab — keyboard shortcut reference (read-only)
  - [ ] Advanced tab — config path, index refresh, scan depth/limit
- [ ] Help screen (Ctrl+H) — keyboard shortcuts

### Themes (CSS custom property sets)
- [x] Catppuccin (default)
- [x] Tokyo Night
- [x] Rose Pine
- [x] Gruvbox
- [x] Dracula
- [x] Kanagawa

### Features
- [ ] Theme switching via CSS custom properties on :root
- [ ] Background image support (CSS background-image + blur overlay)
- [ ] Blur material options (balanced, high_contrast, soft)
- [ ] Font scale control
- [ ] Config file persistence (.look.config format, shared with macOS)
- [ ] Auto-start registration (Windows registry, Linux .desktop autostart)
- [ ] UWP app seeding (Windows — enumerate shell:AppsFolder via PowerShell)

---

## Backlog / Improvements

- [ ] Linux settings handling — detect DE (GNOME/KDE/minimal):
  - GNOME/KDE: `settings://` URLs work via `gnome-control-center` / `systemsettings`
  - Minimal (i3/sway/X11 bare): map to standalone tools (pavucontrol, arandr, blueman-manager, etc.) or hide settings entries
  - Detect via `XDG_CURRENT_DESKTOP`, `DESKTOP_SESSION`, or presence of `gnome-control-center`
- [ ] Some DBUS single-instance apps (blueman-manager, fcitx5-config) fail to launch — known limitation

---

## Platform-Specific Notes

### Windows
- Window blur: Mica/Acrylic via Tauri WindowEffectsConfig
- Icons: SHGetFileInfo or windows-rs shell APIs
- UWP apps: PowerShell enumeration → seed_uwp_apps command
- Auto-start: Registry HKCU\Software\Microsoft\Windows\CurrentVersion\Run
- DB path: %LOCALAPPDATA%/look/look.db

### Linux
- Window blur: Compositor-dependent; fallback to solid dark background
- Icons: freedesktop-icons crate + MIME type detection
- Apps: .desktop file scanning in /usr/share/applications, ~/.local/share/applications
- Auto-start: ~/.config/autostart/look.desktop
- DB path: ~/.local/share/look/look.db
- Global hotkey: Works on X11; Wayland support may be limited
- i3/tiling WMs: needs `for_window [title="Look"] floating enable, border none` in config
