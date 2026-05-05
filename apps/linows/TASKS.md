# linows — Implementation Tasks

Based on macOS app as source of truth. Organized by phase.

---

## Phase 1: Core Search (MVP)

### Scaffold
- [ ] Create Tauri v2 project structure (src-tauri/, src/)
- [ ] Cargo.toml with core crate deps (look-engine, look-indexing, look-storage)
- [ ] tauri.conf.json (borderless, transparent, always-on-top, 620x600)
- [ ] Build script integration (Makefile targets)

### Backend (Rust)
- [ ] `state.rs` — Engine cache (RwLock<QueryEngine>), bootstrap refresh, file watchers
- [ ] `commands.rs` — search(query, limit), record_usage(id, action)
- [ ] `commands.rs` — open_path(path), reveal_path(path)
- [ ] `commands.rs` — reload_config(), request_index_refresh()
- [ ] `platform.rs` — File icon extraction (SHGetFileInfo on Windows, freedesktop on Linux)
- [ ] `config.rs` — Load/reload RuntimeConfig, db path resolution

### Frontend (HTML/CSS/JS)
- [ ] `index.html` — Main window structure
- [ ] `css/reset.css` — CSS reset
- [ ] `css/theme.css` — CSS custom properties (colors, spacing, typography)
- [ ] `css/layout.css` — Window layout, search bar, content area
- [ ] `css/components.css` — Result rows, panels, badges
- [ ] `js/ipc.js` — Tauri invoke wrapper
- [ ] `js/app.js` — Main controller, mode switching
- [ ] `js/search.js` — Debounced search (70ms), query → invoke → render
- [ ] `js/results.js` — DOM rendering of result rows (icon, title, kind badge, path)
- [ ] `js/keyboard.js` — Arrow navigation, Enter to open, Escape to hide

### Window & System
- [ ] Global hotkey (Alt+Space) via tauri-plugin-global-shortcut
- [ ] Single instance via tauri-plugin-single-instance
- [ ] Window blur/transparency (Mica on Windows, compositor on Linux, solid fallback)
- [ ] Auto-hide on focus loss

---

## Phase 2: Preview & Multi-pick

### Screens
- [ ] Result preview panel — file metadata (size, modified, path), image preview
- [ ] Picked items panel — list of multi-selected items with remove buttons

### Features
- [ ] Quick folders (Desktop, Documents, Downloads, Pictures, Videos, Music)
- [ ] Multi-pick (Ctrl+Click to toggle selection)
- [ ] Clipboard write (picked items as paths + text)
- [ ] Reveal in file manager (Ctrl+F)
- [ ] Hint bar (bottom status text)

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
- [ ] Catppuccin (default)
- [ ] Tokyo Night
- [ ] Rose Pine
- [ ] Gruvbox
- [ ] Dracula
- [ ] Kanagawa

### Features
- [ ] Theme switching via CSS custom properties on :root
- [ ] Background image support (CSS background-image + blur overlay)
- [ ] Blur material options (balanced, high_contrast, soft)
- [ ] Font scale control
- [ ] Config file persistence (.look.config format, shared with macOS)
- [ ] Auto-start registration (Windows registry, Linux .desktop autostart)
- [ ] UWP app seeding (Windows — enumerate shell:AppsFolder via PowerShell)

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
