# Development

Guide for building Look locally and contributing to the project.

## Repository layout

```text
.
├── apps/
│   └── macos/
│       └── LauncherApp/          # Swift macOS app (Xcode project)
├── core/
│   ├── engine/                   # Query engine, search pipeline
│   ├── indexing/                 # Candidate model, source traits
│   ├── matching/                 # Fuzzy matching
│   ├── ranking/                  # Ranking heuristics
│   └── storage/                  # SQLite-backed storage
├── bridge/
│   └── ffi/                      # Rust→Swift FFI bridge
├── docs/                         # User guide, architecture, design decisions
├── scripts/                      # Build, release, install scripts
└── assets/                       # Icons, screenshots, demo GIF
```

## Prerequisites

- macOS 15.0+
- Xcode (for the app shell)
- Rust stable toolchain (for the core engine and FFI bridge)

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

`make app-run` behavior:

- builds a local Debug app bundle with Xcode
- stops any running `Look` process (including a Homebrew-installed instance)
- launches with `LOOK_CONFIG_PATH=$HOME/.look.dev.config`
- shows a red `TEST APP` badge so the dev run is visually distinct

Install a side-by-side Launchpad test build (`Look Dev`) without replacing Homebrew `Look`:

```bash
make app-run-dev
```

`make app-run-dev` behavior:

- builds a local Debug app bundle with Xcode
- installs `/Applications/Look Dev.app` with bundle id `noah-code.Look.Dev`
- keeps Homebrew-installed `/Applications/Look.app` untouched
- launches `Look Dev` with `LOOK_CONFIG_PATH=$HOME/.look.dev.config`

Override dev config path:

```bash
make app-run DEV_CONFIG_PATH="$HOME/.look.qa.config"
make app-run-dev DEV_CONFIG_PATH="$HOME/.look.qa.config"
```

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
