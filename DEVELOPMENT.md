# Development

Guide for building Look locally and contributing to the project.

## Repository layout

```text
.
├── apps/
│   ├── macos/
│   │   └── LauncherApp/          # Swift macOS app (Xcode project)
│   └── windows/
│       └── LauncherApp/          # WinUI 3 / .NET Windows app
├── core/
│   ├── engine/                   # Query engine, search pipeline
│   ├── indexing/                 # Candidate model, source traits
│   ├── matching/                 # Fuzzy matching
│   ├── ranking/                  # Ranking heuristics
│   └── storage/                  # SQLite-backed storage
├── bridge/
│   └── ffi/                      # Rust FFI bridge (consumed by both apps)
├── docs/                         # User guide, architecture, design decisions
├── scripts/                      # Build, release, install scripts
└── assets/                       # Icons, screenshots, demo GIF
```

## Prerequisites

Common:

- Rust stable toolchain (for the core engine and FFI bridge)
- GNU Make (top-level `Makefile` dispatches to `Makefile.mac` or `Makefile.win` based on host OS)

macOS:

- macOS 15.0+
- Xcode (for the app shell)

Windows:

- Windows 10 19041+ / Windows 11 (x64 or ARM64)
- .NET 10 SDK
- Visual Studio Build Tools with the C++ workload (the Rust FFI build script uses `vswhere` + `VsDevCmd.bat`)
- **GNU Make + Git Bash, both required.** Run every `make` target from a Git Bash shell — not from cmd or PowerShell. `Makefile.win` sets `SHELL := bash.exe`, and the recipes use Unix tools (`rm -rf`, `cp -r`, `env`, `printf`, `mkdir -p`) that don't exist on cmd, plus `$HOME` resolution that PowerShell doesn't expose as an env var.
- Install steps:
  ```powershell
  winget install GnuWin32.Make           # provides make.exe
  # If make is on disk but not on PATH after install, append C:\Program Files (x86)\GnuWin32\bin
  ```
  Then in **Git Bash** (open from Start menu after the install — older sessions won't see the new PATH):
  ```bash
  which make            # /c/Program Files (x86)/GnuWin32/bin/make
  cd /c/path/to/look
  make help             # shows the Windows targets
  ```
- Optional: `sqlite3` on PATH (`winget install sqlite.sqlite`) for `make db-*` targets

> **Common gotchas on Windows**
> - `make: command not found` — open a fresh Git Bash window after the winget install so PATH refreshes
> - `'true' is not recognized as an internal or external command` — you're running make from cmd/PowerShell, not Git Bash
> - `/AppData/Local/...` (with empty leading path) instead of `/c/Users/<you>/AppData/Local/...` — same; switch to Git Bash so `$HOME` resolves

## Building and running

Rust workspace checks:

```bash
cd core
cargo check --workspace
cargo test --workspace
```

FFI bridge checks:

```bash
cd bridge/ffi
cargo check
cargo test
```

Run the local dev app (from repo root):

```bash
make app-run
```

`make app-run` behavior (macOS):

- builds a local Debug app bundle with Xcode
- stops any running `Look` process (including a Homebrew-installed instance)
- launches with `LOOK_CONFIG_PATH=$HOME/.look.dev.config`
- shows a red `TEST APP` badge so the dev run is visually distinct

`make app-run` behavior (Windows):

- runs `dotnet build -c Debug -p:Platform=x64 -r win-x64` (matches the PR-CI command)
- stops any running `LauncherApp` process
- launches the freshly built exe with the same `LOOK_*` dev env vars
- override platform with `PLATFORM=ARM64 RID=win-arm64`

Install a side-by-side test build (`Look Dev`) without replacing the normal install:

```bash
make app-run-dev
```

`make app-run-dev` behavior:

- macOS: builds a local Debug bundle, installs `/Applications/Look Dev.app` with bundle id `noah-code.Look.Dev`, keeps the Homebrew `/Applications/Look.app` untouched, then launches `Look Dev` with `LOOK_CONFIG_PATH=$HOME/.look.dev.config`.
- Windows: runs `dotnet publish` with the `win-<arch>.pubxml` profile (matches the release-CI command), copies the publish output to `%LOCALAPPDATA%\Programs\Look Dev\`, then launches that side-by-side install with the dev env vars.

Override dev config path:

```bash
make app-run DEV_CONFIG_PATH="$HOME/.look.qa.config"
make app-run-dev DEV_CONFIG_PATH="$HOME/.look.qa.config"
```

`make help` lists every target available on the current host (macOS or Windows).

## Benchmarks

Run the query-engine benchmark:

```bash
cargo run --manifest-path core/engine/Cargo.toml --example perf_bench
```

Benchmark snapshots land under [docs/bench-notes/](docs/bench-notes/). Add a new snapshot when scoring, matching, or indexing changes.

## Releasing (maintainers)

Build release artifacts and Homebrew cask:

```bash
./scripts/build-release.sh 1.0.0
./scripts/generate-homebrew-cask.sh 1.0.0 <sha256> kunkka19xx/look
```

Signing and notarization:

- a paid Apple Developer membership is required for Developer ID signing and notarization
- strict release runs require signing and notary secrets
- non-strict test runs can still build artifacts when secrets are missing

Signing/notarization walkthrough: [docs/apple-developer-release-guide.md](docs/apple-developer-release-guide.md).

## Contribution flow

- branch from `dev` and open PRs into `dev`
- PRs to `main` are reserved for maintainer-coordinated hotfix and release work
- run local checks before opening a PR:
  ```bash
  cargo test --workspace --manifest-path core/Cargo.toml
  cargo test --manifest-path bridge/ffi/Cargo.toml
  ```
- update docs when user-visible behavior changes
- see [CONTRIBUTING.md](CONTRIBUTING.md) and the issue templates under [.github/ISSUE_TEMPLATE/](.github/ISSUE_TEMPLATE/)

## Further reading

- [docs/architecture.md](docs/architecture.md) — canonical architecture reference
- [docs/backend-guide.md](docs/backend-guide.md) — backend edit targets and verification
- [docs/features.md](docs/features.md) — feature status
- [docs/tasks.md](docs/tasks.md) — task breakdown
