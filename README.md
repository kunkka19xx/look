# look

<img src="assets/icon.png" alt="look icon" width="96" />

A keyboard-first, local-first macOS launcher. Open apps, files, folders, clipboard history, and quick commands without leaving the keyboard.

[![CI](https://github.com/kunkka19xx/look/actions/workflows/ci.yml/badge.svg)](https://github.com/kunkka19xx/look/actions/workflows/ci.yml) [![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE) ![Platform](https://img.shields.io/badge/platform-macOS%2015%2B-lightgrey) [![Homebrew](https://img.shields.io/badge/homebrew-cask-orange)](#install) [![Stars](https://img.shields.io/github/stars/kunkka19xx/look?style=flat&logo=github)](https://github.com/kunkka19xx/look/stargazers)

![Look demo](assets/look.gif)

> 📘 **Docs:** [noah-code.com/docs/look](https://noah-code.com/docs/look) · 🎬 [Demo on YouTube](https://www.youtube.com/watch?v=4Twb4We3PIs)

## Install

```bash
brew tap kunkka19xx/tap
brew install --cask look
```

Then bind `Cmd+Space` to Look (disable Spotlight's shortcut in `System Settings > Keyboard > Keyboard Shortcuts > Spotlight`). Release builds are signed and notarized — no Gatekeeper bypass needed.

Other install options and manual setup: see [Installation details](#installation-details).

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

| Key | Action |
|---|---|
| `Cmd+Space` | Toggle launcher |
| `Enter` | Open / run |
| `Cmd+Enter` | Web search |
| `Cmd+F` | Reveal in Finder |
| `Cmd+/` | Command mode (`calc`, `shell`, `kill`, `sys`) |
| `Cmd+Shift+,` | Settings |
| `Escape` | Back / hide |

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

## Installation details

Homebrew (install and update):

```bash
# install
brew tap kunkka19xx/tap
brew install --cask look

# update
brew upgrade --cask kunkka19xx/tap/look

# uninstall
brew uninstall --cask look
```

Curl installer:

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

## Scope

In scope:

- apps, files, folders, clipboard, command mode, translation, regex/path search
- local-first behavior, zero telemetry
- near-term plugin/extension exploration

Out of scope for v1:

- online-first behavior
- semantic/vector search
- full content indexing (names and metadata only)

Platform direction: macOS now, Windows next. Linux is not a near-term priority because `rofi` already covers the workflow well.

## License

MIT — see [LICENSE](LICENSE).

## Contributors

Thanks to everyone who has contributed — see the [contributor graph](https://github.com/kunkka19xx/look/graphs/contributors).

Contribution flow: branch from `dev`, open PRs into `dev`. See [CONTRIBUTING.md](CONTRIBUTING.md) and [DEVELOPMENT.md](DEVELOPMENT.md).
