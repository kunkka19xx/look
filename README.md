# look

<img src="assets/icon.png" alt="look icon" width="96" />

A keyboard-first, local-first launcher for macOS, Windows, and Linux. Open apps, files, folders, clipboard history, and quick commands without leaving the keyboard.

[macOS](#macos) · [Windows](#windows) · [Linux](#linux-under-development)
📘 [Docs](https://noah-code.com/docs/look) · 📖 [User guide](docs/user-guide.md) · 🎬 [Demo video](https://www.youtube.com/watch?v=4Twb4We3PIs)

https://github.com/user-attachments/assets/176a929d-edbe-46a0-a0c5-229eb9b31c1c

## Install

### macOS

```bash
brew tap kunkka19xx/tap
brew install --cask look
```

Then bind `Cmd+Space` to Look (disable Spotlight's shortcut in `System Settings > Keyboard > Keyboard Shortcuts > Spotlight`). Release builds are signed and notarized — no Gatekeeper bypass needed.

### Windows

Install the latest release with one PowerShell line (no admin required):

```powershell
iex "& { $(irm https://raw.githubusercontent.com/kunkka19xx/look/main/scripts/windows/install-look.ps1) }"
```

The script auto-detects x64 vs ARM64, pulls the matching release zip, verifies its SHA256 against the published manifest, extracts to `%LOCALAPPDATA%\Programs\Look`, and creates Start menu + desktop shortcuts. The release bundle is self-contained (`<WindowsAppSDKSelfContained>true</WindowsAppSDKSelfContained>`) so no separate runtime install is needed.

Uninstall:

```powershell
iex "& { $(irm https://raw.githubusercontent.com/kunkka19xx/look/main/scripts/windows/install-look.ps1) } -Uninstall"
```

SmartScreen may warn on first run while reputation builds — click "More info → Run anyway". The launcher's global hotkey (`Alt+Space`) is configurable in Settings → Appearance.

### Linux (under development)

Download `.deb` or `.AppImage` from [Releases](https://github.com/kunkka19xx/look/releases):

```bash
# Ubuntu/Debian
sudo dpkg -i Look_*.deb

# Any distro (AppImage)
chmod +x Look_*.AppImage
./Look_*.AppImage
```

Uninstall:

```bash
# Ubuntu/Debian
sudo dpkg -r lookapp

# AppImage — just delete the file
rm Look_*.AppImage
```

**NixOS (flake):**

```bash
# Run directly
nix run github:kunkka19xx/look?dir=apps/linows

# Install to profile
nix profile install github:kunkka19xx/look?dir=apps/linows
```

Declarative (NixOS flake):

```nix
# flake.nix
inputs.look.url = "github:kunkka19xx/look?dir=apps/linows";

# In your nixosConfigurations modules list:
modules = [
  look.nixosModules.default
  { programs.lookapp.enable = true; }
];
```

More install methods coming soon (AUR). To build from source, see [apps/linows/BUILDING.md](apps/linows/BUILDING.md).

<details>
<summary>Other install options (curl, pin version, update/uninstall)</summary>

**macOS — Homebrew update / uninstall:**

```bash
# update
brew upgrade --cask kunkka19xx/tap/look

# uninstall
brew uninstall --cask look
```

**macOS — curl installer:**

```bash
curl -fsSL https://raw.githubusercontent.com/kunkka19xx/look/main/scripts/install-look.sh | bash
```

Pin a specific version or repo fork:

```bash
curl -fsSL https://raw.githubusercontent.com/kunkka19xx/look/main/scripts/install-look.sh | bash -s -- --version <version> --repo kunkka19xx/look
```

Direct URL:

```bash
curl -fsSL https://raw.githubusercontent.com/kunkka19xx/look/main/scripts/install-look.sh | bash -s -- --url "https://github.com/kunkka19xx/look/releases/download/v<version>/Look-<version>-macOS.zip"
```

CLI naming note: macOS ships `/usr/bin/look`, so terminal command examples use `lookapp`.

If Look is fully quit and Spotlight is still unbound, relaunch from Launchpad, or via:

```bash
open "/Applications/Look.app"
```

</details>

## What you can do

- **Find and open anything** — apps, files, folders indexed locally. Type, Enter, done.
- **Calc inline** — type `2^10`, `4!`, `200*15%`, `sqrt(2)`, `2*pi`. No command mode needed.
- **Kill a process by port** — `Cmd+/` then `kill :3000`. Confirms before killing.
- **Search clipboard history** — `c"meeting` finds the snippet you copied an hour ago.
- **Translate or look up a word** — `t"hello` for quick translation, `tw"word` for a definition panel.
- **Regex, path, and kind-scoped search** — `r"^Visual.*`, `git/project/readme`, `a"safari`, `f"note`, `d"documents`.

All local. No account. No telemetry. No plugin marketplace to manage.

## Why look

- **Fast** — typical search under 1 ms on a 2000-item index; empty-query browse under 30 µs.
- **Small** — single native macOS app, no Electron, no background daemons.
- **Local-first** — candidates indexed in a local SQLite file; the only network calls are explicit (`t"`, `tw"`, `Cmd+Enter` web search).
- **Zero-config by default** — presets cover common apps (`alias_note`, `alias_code`, `alias_term`, `alias_chat`, `alias_music`, `alias_brow`). Configure more via `~/.look.config` when you want to.
- **Keyboard-first** — every action has a key; mouse never required.

If you want a launcher that stays out of your way and does exactly what you asked, that's the pitch.

## Essential shortcuts

| Action                                        | macOS            | Windows             |
| --------------------------------------------- | ---------------- | ------------------- |
| Toggle launcher                               | `Cmd+Space`      | `Alt+Space`         |
| Open / run                                    | `Enter`          | `Enter`             |
| Web search                                    | `Cmd+Enter`      | `Ctrl+Enter`        |
| Reveal in file manager                        | `Cmd+F` (Finder) | `Ctrl+F` (Explorer) |
| Command mode (`calc`, `shell`, `kill`, `sys`) | `Cmd+/`          | `Ctrl+/`            |
| Settings                                      | `Cmd+Shift+,`    | `Ctrl+Shift+,`      |
| Back / hide                                   | `Escape`         | `Escape`            |

(Throughout the rest of the docs, `Cmd+X` on macOS maps to `Ctrl+X` on Windows; the launcher-toggle hotkey is the only one that uses a different modifier — `Alt+Space` instead of `Cmd+Space` because Windows reserves `Win+Space`.)

Full reference: [docs/user-guide.md](docs/user-guide.md).

## Themes

Built-in: Catppuccin, Tokyo Night, Rose Pine, Gruvbox, Dracula, Kanagawa, plus Custom. Switch in `Settings > Appearance`.

<p align="center">
  <img src="assets/look-ui/1.png" width="45%" />
  <img src="assets/look-ui/2.png" width="45%" />
</p>
<p align="center">
  <img src="assets/look-ui/3.png" width="45%" />
  <img src="assets/look-ui/4.png" width="45%" />
</p>
<p align="center">
  <img src="assets/look-ui/5.png" width="45%" />
  <img src="assets/look-ui/6.png" width="45%" />
</p>

## Documentation

- 📘 [Docs site](https://noah-code.com/docs/look) — hosted, searchable user guide and reference
- [User guide (in-repo)](docs/user-guide.md) — full feature reference, shortcuts, configuration, permissions, troubleshooting
- [Architecture](docs/architecture.md) — how the Swift app + Rust core fit together
- [Features](docs/features.md) — what's shipped, what's planned
- [Contributing](CONTRIBUTING.md) — how to contribute
- [Development](DEVELOPMENT.md) — building locally, repo layout, release process

## Scope

In scope:

- apps, files, folders, clipboard, command mode, translation, regex/path search
- local-first behavior, zero telemetry
- near-term plugin/extension exploration

Out of scope for v1:

- online-first behavior
- semantic/vector search
- full content indexing (names and metadata only)

### Platform direction

- **macOS** — shipped and stable (SwiftUI, native). This is the design source of truth.
- **Windows + Linux** — a new shared Tauri v2 app (`apps/linows/`) is under active development. It targets both platforms with a single codebase (Rust backend, vanilla HTML/CSS/JS frontend). Current status:
  - Core search, preview, multi-pick, clipboard history, translation — done
  - Command mode (calc, pomo, kill, shell, sys) — done
  - Settings screen (appearance, themes, blur, font autocomplete) — done
  - Platform-aware blur (Mica/Acrylic on Windows, CSS backdrop-filter on Linux)
  - Dynamic window scaling based on monitor resolution
  - 6 built-in themes + Custom
- **Windows (WinUI3)** — the current `apps/windows/` WinUI3/C# app is in maintenance mode (bug fixes only). It will be archived once the Tauri app reaches feature parity.

## License

MIT — see [LICENSE](LICENSE).

## Contributors

Thanks to everyone who has contributed — see the [contributor graph](https://github.com/kunkka19xx/look/graphs/contributors).

Contribution flow: branch from `dev`, open PRs into `dev`. See [CONTRIBUTING.md](CONTRIBUTING.md) and [DEVELOPMENT.md](DEVELOPMENT.md).
