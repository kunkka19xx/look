# Tauri Windows port — working notes

Working document for bringing `apps/linows/` to Windows parity. Updated as we go.

## Goal

Ship Look on Windows from the same Tauri v2 codebase as Linux, replacing the WinUI3 app at `apps/windows/`. macOS (`apps/macos/`) remains the design source of truth; Linux UI is the current parity baseline for non-macOS look-and-feel.

## Constraints

- **Do not regress Linux UI.** Linux is in good shape. Windows-specific UI tweaks must be scoped (e.g. `<html data-platform="windows">` selector, or a class set from Rust at boot via Tauri OS plugin). Do not edit shared CSS selectors that already render correctly on Linux.
- **Backend restructure is fine.** Reorganising Rust modules under `src-tauri/src/platform/{linux,windows,shared}/` is the planned approach and is not affected by the UI rule above.
- **WinUI3 app (`apps/windows/`) is reference-only.** Keep in-tree, unmaintained. Mine it for behaviour (autostart, focus-existing-window, icon pipeline, AUMID handling, theme presets) but do not ship it.
- **Packaging target: NSIS, per-user install, no admin.** Matches the current WinUI3 ship path (`%LOCALAPPDATA%\Programs\Look`).

## Current state (audit, 2026-05-16)

`apps/linows/` is Linux-biased. Compiles for Windows in principle but most platform plumbing is missing.

| Subsystem | Windows status | Source |
|---|---|---|
| Window effects (Mica/Acrylic) | done | `src-tauri/src/platform.rs:421-452` |
| Reveal in `explorer.exe` | done | `src-tauri/src/commands.rs:184` |
| Config dir via `LOCALAPPDATA` | done | `src-tauri/src/state.rs:334` |
| `autostart.rs` | missing — XDG-only, needs `HKCU\…\Run` | `src-tauri/src/autostart.rs` |
| Icon extraction | missing — XDG/.desktop only, needs Shell APIs | `src-tauri/src/platform.rs` |
| Process list / kill | missing — `/proc`-based | `src-tauri/src/process.rs` |
| Clipboard file copy | missing — `xclip`/`wl-clipboard` only | `src-tauri/src/commands.rs` |
| Window focus / focus-existing-app | missing — 5 `linux_*` modules (~1.1k LOC), no Windows analogue | `src-tauri/src/linux_*.rs` |
| Global hotkey | unknown — Tauri plugin should suffice on Windows; verify | — |
| `tauri.conf.json` bundle targets | missing — only `deb`+`appimage` | `src-tauri/tauri.conf.json:37` |
| `Cargo.toml` Windows target deps | missing — only Linux `[target.cfg]` block | `src-tauri/Cargo.toml:24` |
| CI workflow for linows on Windows | missing | `.github/workflows/` |

42 `cfg(target_os="linux")` gates across 5 files. Five top-level `linux_*` modules. Adding Windows backends without restructure would double the noise.

## Proposed restructure (no behaviour change)

```
src-tauri/src/
├── main.rs, state.rs, config.rs, commands.rs, shell.rs,
│   calc.rs, music.rs, clipboard.rs, sysinfo.rs, translate.rs, files.rs
└── platform/
    ├── mod.rs                 # cfg-gated re-exports
    ├── shared.rs              # read_icon_file, shared helpers
    ├── linux/
    │   ├── gnome_ext.rs       (was linux_gnome_ext.rs)
    │   ├── transparency.rs    (was linux_transparency.rs)
    │   ├── wayland_shortcut.rs, window_focus.rs, wlr_focus.rs
    │   ├── autostart.rs       (XDG .desktop part of autostart.rs)
    │   ├── icons.rs           (XDG/.desktop scan from platform.rs)
    │   ├── process.rs         (Linux half of process.rs)
    │   └── clipboard.rs       (xclip/wl-clipboard fallback)
    └── windows/
        ├── autostart.rs       (HKCU\…\Run)
        ├── icons.rs           (SHGetFileInfo / IShellItemImageFactory)
        ├── process.rs         (Toolhelp32 + TerminateProcess)
        ├── clipboard.rs       (CF_HDROP file copy)
        ├── window_focus.rs    (SetForegroundWindow + AUMID match)
        └── effects.rs         (extracted Mica/Acrylic)
```

## Plan

### Step 0 — Verify current state compiles on Windows
- `rustc --version`, `cargo tauri --version`, `where.exe cl.exe`, `rustup target list --installed`
- `cargo check` in `apps/linows/src-tauri`
- If green: proceed. If red: fix compile errors first as a precursor PR.

### Step 1 — Restructure (no-op refactor)

**Concrete move/extract plan** (audited 2026-05-16):

**Pure file moves (rename only):**
- `src/linux_gnome_ext.rs`        → `src/platform/linux/gnome_ext.rs`
- `src/linux_transparency.rs`     → `src/platform/linux/transparency.rs`
- `src/linux_wayland_shortcut.rs` → `src/platform/linux/wayland_shortcut.rs`
- `src/linux_window_focus.rs`     → `src/platform/linux/window_focus.rs`
- `src/linux_wlr_focus.rs`        → `src/platform/linux/wlr_focus.rs`

**Split files (Linux block → `platform/linux/`, Windows block → `platform/windows/`, shared → keep):**

- `src/platform.rs` (481 lines) is the messiest — three concerns mixed:
  1. `get_icon`, `IconCache`, `IconResult`, `resolve_icon` (dispatcher) — **stays at top of platform/mod.rs** (cross-platform entry; dispatches to platform-specific resolver)
  2. `xdg_data_dirs`, `resolve_app_icon`, `parse_desktop_icon`, `resolve_themed_icon`, `resolve_file_icon`, scoring helpers — **move to `platform/linux/icons.rs`**
  3. `set_window_effect` (Windows-only Mica/Acrylic) — **move to `platform/windows/effects.rs`** (re-exported as `#[tauri::command]` from `platform/mod.rs`)
  4. `detect_compositor`, `is_tiling_wm` — **move to `platform/linux/wm.rs`** (or fold into `transparency.rs`)
  5. `read_icon_file` (base64-encode any image file, no OS calls) — **move to `platform/shared.rs`** (used by Linux today; Windows will reuse for cached PNGs)

- `src/autostart.rs` (52 lines) — XDG-only. Today the `set_autostart`/`get_autostart` Tauri commands hardcode `~/.config/autostart/look.desktop`.
  - **Move current body → `platform/linux/autostart.rs`**
  - **New `platform/windows/autostart.rs`** writing `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`
  - **`src/autostart.rs` becomes the dispatch shell** with `#[tauri::command]`s that delegate via `cfg(target_os = …)` to the platform module

- `src/process.rs` (390 lines) — no cfg gates today, reads `/proc` directly. Will silently return empty Vec on Windows.
  - **Move current body → `platform/linux/process.rs`**
  - **New `platform/windows/process.rs`** using Toolhelp32 + `TerminateProcess`
  - **`src/process.rs` becomes dispatch shell** — keeps `RunningApp` struct, `list_processes`/`kill_process` commands, delegates impl per-cfg

- `src/state.rs` — `look_db_path()` has Windows + Linux branches (`state.rs:334-352`).
  - Keep helper function in `state.rs`. It's already neatly cfg-gated and only ~20 lines; not worth splitting.

- `src/commands.rs` — reveal-in-Explorer has cross-platform branches (`commands.rs:184-…`); clipboard file-copy uses `xclip`/`wl-clipboard` (Linux-only).
  - Keep `#[tauri::command]` wrappers in `commands.rs`.
  - **Extract Linux clipboard-file-copy helper → `platform/linux/clipboard.rs`**
  - **New `platform/windows/clipboard.rs`** with CF_HDROP file copy

**New files:**

- `src/platform/mod.rs` — module tree; cfg-gated `pub use platform::{linux,windows}::*;` lines.
- `src/platform/shared.rs` — `read_icon_file`, any other genuinely platform-shared helpers.
- `src/platform/linux/mod.rs`, `src/platform/windows/mod.rs` — sub-tree roots.

**main.rs touch points:**
- Replace 5 `mod linux_xxx;` declarations (lines 11, 13, 15, 17, 19) with `mod platform;`.
- Replace 8 call sites (`linux_transparency::has_compositor`, `linux_window_focus::activate_self`, `linux_gnome_ext::ensure_installed`, `linux_wayland_shortcut::start`, etc.) with `platform::xxx` or keep direct paths through `cfg(target_os="linux") { use crate::platform::linux::… }`. Decide once we land Step 1.

**Verification gates (must pass before declaring Step 1 done):**
1. `cargo check --target x86_64-pc-windows-msvc` from `apps/linows/src-tauri` — same result as baseline.
2. `cargo check` on Linux (NixOS dev shell or Ubuntu VM) — same result as baseline.
3. `cargo tauri dev` on Linux — UI loads identically (no UI changes in this step).
4. Diff is move-only — `git log -p --follow` should show file relocations with no semantic edits beyond `mod` paths.

### Step 2 — Windows backends (milestone-paced, full parity is the goal)
- **M1 — runnable:** window opens, global hotkey, search results render, open file/folder/url. (Most of this is already cross-platform; M1 is mostly Step 1 + verification.)
- **M2 — icons + reveal:** `platform/windows/icons.rs` (Shell APIs), reveal-in-Explorer already works.
- **M3 — autostart + window focus:** `HKCU\…\Run`, `SetForegroundWindow` + AUMID-aware focus-existing-app.
- **M4 — process + clipboard file copy:** Toolhelp32 process enum, `TerminateProcess`, `CF_HDROP`.
- **M5 — packaging:** NSIS bundle target in `tauri.conf.json`, Windows CI workflow, signing story.

### Step 3 — UI scoping (only where needed)
- Add platform-class attribute at boot (Rust → JS) so any Windows-specific CSS can be scoped without touching Linux selectors.
- Identify any Windows-specific UI needs (Mica blur tuning, title-bar absence handling, taskbar/tray icon, font fallbacks). Defer until M1 surfaces actual gaps.

## Open questions (resolved 2026-05-16)

1. **WinUI3 app fate?** → Keep in-tree as unmaintained reference. Don't ship.
2. **First-runnable scope?** → Full parity day-one is the goal; we'll stage M1–M4 so there's a runnable build at every step.
3. **Packaging target?** → NSIS, per-user, no admin. Match WinUI3's `%LOCALAPPDATA%\Programs\Look` UX.
4. **Restructure first or backends first?** → Restructure first as a no-op PR.
5. **Verify-first?** → Yes — run toolchain checks + `cargo check` on this Win11 host before planning further.

## Decisions log

- **2026-05-16** — Linux UI is the parity baseline among non-macOS; do not regress it while doing Windows work. Windows-only UI tweaks must be platform-scoped.
- **2026-05-16** — WinUI3 stays in-tree as reference; not shipped, not maintained.
- **2026-05-16** — Windows packaging will use NSIS (per-user, no admin) to match WinUI3's existing UX.
- **2026-05-16** — Restructure first as a no-op PR, then Windows backends incrementally.
- **2026-05-16** — Windows Rust builds **must run under VS 2022 BT `vcvarsall.bat x64` env**, not a bare shell. Rustc autodetects VS 2026 Community's `link.exe` but that install lacks the Windows SDK, so the linker can't find `msvcrt.lib` (LNK1104). Existing `build-ffi.bat` already follows this pattern. For ad-hoc dev: wrap cargo calls in `cmd /c 'call "<vs2022bt>\VC\Auxiliary\Build\vcvarsall.bat" x64 && cargo …'`. Same applies to `cargo install tauri-cli` — without the env, build scripts fail to link.

## Status

- [x] Step 0 — verify Windows toolchain + `cargo check` (passes on `x86_64-pc-windows-msvc` under VS 2022 BT `vcvarsall.bat x64`)
- [x] Step 1 — restructure to `platform/{linux,windows,shared}/`
  - Phase A (committed): 5 `linux_*.rs` → `platform/linux/*.rs`, old `platform.rs` → `platform/mod.rs`, 17 call sites updated
  - Phase B (pending Linux verification + commit): split `platform/mod.rs` into `linux/icons.rs` + `linux/wm.rs` + `windows/effects.rs` + `shared.rs`; split `autostart.rs`, `process.rs` into platform-dispatched shells; extract clipboard file-copy from `files.rs` to `platform/{linux,windows}/clipboard.rs`. Windows stubs return safe defaults (Vec::new / Ok(()) / Err) marked `TODO(M3)` or `TODO(M4)`. cargo check green on Windows in 1.62s.
- [ ] Step 2 M1 — runnable Windows build (window + search + open)
- [ ] Step 2 M2 — icons
- [ ] Step 2 M3 — autostart + window focus
- [ ] Step 2 M4 — process + clipboard file copy
- [ ] Step 2 M5 — NSIS packaging + CI
- [ ] Step 3 — Windows UI scoping (as needed)

### Linux verification needed for Phase B

Phase B was developed on a Windows host. Linux compilation was not verified locally. Before merging, run on a Linux dev machine:

```bash
cd apps/linows
cargo check --manifest-path src-tauri/Cargo.toml
cargo tauri dev    # smoke test: launcher opens, search/open/reveal/clipboard/autostart still work
```

If anything Linux-only breaks, the most likely culprits are stale `crate::linux_*` paths that grep missed (none expected — grep was clean), or visibility issues with `pub(crate)` vs the old default `pub`.
