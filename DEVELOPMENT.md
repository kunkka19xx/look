# Development

Guide for building Look locally and contributing to the project.

## Repository layout

```text
.
├── apps/
│   ├── macos/
│   │   └── LauncherApp/          # Swift macOS app (Xcode project)
│   ├── linows/                   # Tauri v2 app, Linux + Windows (under development)
│   │   ├── src-tauri/            #   Rust backend (commands, config, platform, etc.)
│   │   ├── src/                  #   Frontend (vanilla HTML/CSS/JS, ES modules)
│   │   └── flake.nix             #   NixOS dev shell
│   └── windows/
│       └── LauncherApp/          # Legacy WinUI 3 app (archived, superseded by linows)
├── core/
│   ├── engine/                   # Query engine, search pipeline
│   ├── indexing/                 # Candidate model, source traits
│   ├── matching/                 # Fuzzy matching
│   ├── ranking/                  # Ranking heuristics
│   └── storage/                  # SQLite-backed storage
├── bridge/
│   └── ffi/                      # Rust FFI bridge (consumed by macOS/Windows native apps)
├── tools/
│   └── perf/                     # Watcher / refresh benchmarks (separate crate, never bundled)
├── docs/                         # User guide, architecture, design decisions
├── scripts/                      # Build, release, install scripts
└── assets/                       # Icons, screenshots, demo GIF
```

## Prerequisites

Common:

- Rust stable toolchain (for the core engine and FFI bridge)
- GNU Make (top-level `Makefile` dispatches to `scripts/Makefile.mac` or `scripts/Makefile.win` based on host OS)

macOS:

- macOS 15.0+
- Xcode (for the app shell)

Windows / Linux (linows, the Tauri app):

- Rust stable + `cargo-tauri` CLI (`cargo install tauri-cli --version "^2" --locked`)
- Windows: Visual Studio 2022 Build Tools (Desktop C++ workload); WebView2 ships with Windows 11
- Linux: distro WebKitGTK/GTK system libraries (or `nix develop` on NixOS)

The per-distro package lists, the Windows `vcvars` setup and `LNK1104` notes, and all packaging/installer details are canonical in [apps/linows/BUILDING.md](apps/linows/BUILDING.md).

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

Linows (Tauri) dev run: `cd apps/linows && cargo tauri dev` (release: `cargo tauri build`; on NixOS prefix with `nix develop -c`). Per-distro and Windows `vcvars` specifics are in [apps/linows/BUILDING.md](apps/linows/BUILDING.md).

Run the local dev app, macOS/Windows (from repo root):

```bash
make app-run
```

`make app-run` behavior (macOS):

- builds a local Debug app bundle with Xcode
- stops any running `Look` process (including a Homebrew-installed instance)
- launches with `LOOK_CONFIG_PATH=$HOME/.look.dev.config`
- shows a red `TEST APP` badge so the dev run is visually distinct

`make app-run` behavior (Windows):

- stops any running `lookapp` process
- runs `cargo tauri dev` for the linows app (`apps/linows/`) under the VS 2022 `vcvars` environment, with hot reload
- `make app-run-release` builds the release bundle instead (`cargo tauri build`)

Install a side-by-side test build (`Look Dev`) without replacing the normal install (macOS only):

```bash
make app-run-dev
```

`make app-run-dev` (macOS) builds a local Debug bundle, installs `/Applications/Look Dev.app` with bundle id `noah-code.Look.Dev`, leaves the Homebrew `/Applications/Look.app` untouched, then launches `Look Dev` with `LOOK_CONFIG_PATH=$HOME/.look.dev.config`. On Windows there is no separate dev install; use `make app-run` (hot reload) or `make app-run-release`.

Override the macOS dev config path:

```bash
make app-run-dev DEV_CONFIG_PATH="$HOME/.look.qa.config"
```

`make help` lists every target available on the current host (macOS or Windows).

## Benchmarks

All benches live in a separate `tools/perf` crate. Nothing in `apps/` or
`bridge/` depends on it, so they never end up in a shipped binary.

```bash
cd tools/perf
cargo run --release --bin query_engine_bench     # query throughput + fuzzy scoring micro-bench
cargo run --release --bin scoped_refresh_bench   # per-call latency: ALL / APPS_ONLY / FILES_ONLY
cargo run --release --bin watcher_stress         # simulated event streams, BEFORE vs AFTER
cargo run --release --bin real_fs_stress         # real notify watcher + worker doing real disk I/O
```

Watcher / index-refresh methodology, scenarios, and a side-by-side report
live at [tools/perf/WATCHER_PERF.md](tools/perf/WATCHER_PERF.md).

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

- maintainer PRs target `main` directly
- external contributions: branch from `dev` and open PRs into `dev`
- run local checks before opening a PR:
  ```bash
  cargo test --workspace --manifest-path core/Cargo.toml
  cargo test --manifest-path bridge/ffi/Cargo.toml
  # if touching linows:
  cargo clippy --manifest-path apps/linows/src-tauri/Cargo.toml
  cargo fmt --all --manifest-path apps/linows/src-tauri/Cargo.toml -- --check
  ```
- update docs when user-visible behavior changes
- see [CONTRIBUTING.md](CONTRIBUTING.md) and the issue templates under [.github/ISSUE_TEMPLATE/](.github/ISSUE_TEMPLATE/)

## Further reading

- [docs/architecture.md](docs/architecture.md) - canonical architecture reference
- [docs/backend-guide.md](docs/backend-guide.md) - backend edit targets and verification
- [docs/user-guide.md](docs/user-guide.md) - user guide
- [docs/features.md](docs/features.md) - feature status
- [apps/linows/BUILDING.md](apps/linows/BUILDING.md) - linows build, packaging, and install methods
