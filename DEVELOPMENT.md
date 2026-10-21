# Development

Guide for building Look locally and contributing to the project.

## Repository layout

```text
.
├── apps/
│   ├── macos/                    # Swift macOS app (Xcode project)
│   └── linows/                   # gpui app, Linux + Windows
│       ├── gpui/                 #   The shell: window, screens, keys (lookapp)
│       ├── backend/              #   Everything that is not drawing (shared)
│       ├── src-tauri/, src/      #   The Tauri shell, kept as the backup
│       └── flake.nix             #   NixOS dev shell
├── core/                         # Shared Rust, consumed by every shell
│   ├── ai/                       # Routing, planning, lexicon
│   ├── answers/                  # Platform-agnostic "web answer" features
│   ├── calc/                     # Calculator expression evaluation
│   ├── engine/                   # Query engine, search pipeline, config
│   ├── indexing/                 # Candidate model, source traits
│   ├── lunar/                    # Solar-to-lunar date conversion
│   ├── matching/                 # Fuzzy matching
│   ├── netspeed/                 # Bandwidth measurement
│   ├── qactions/                 # Quick Actions catalog (declarative half)
│   ├── ranking/                  # Ranking heuristics
│   ├── sources/                  # User-declared source blocks
│   ├── storage/                  # SQLite-backed storage
│   ├── todo/                     # Todo backend
│   └── tools/                    # Preferred tools: catalog + command composition
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

- Rust stable toolchain (for the core engine) plus the **nightly** toolchain (the macOS "Build Rust FFI" phase builds with `cargo +nightly`)
- GNU Make (top-level `Makefile` dispatches to `scripts/Makefile.mac` or `scripts/Makefile.win` based on host OS)

macOS:

- macOS 15.0+
- Xcode (for the app shell)

Windows / Linux (linows, the gpui app):

- Rust stable
- Windows: Visual Studio 2022 Build Tools (Desktop C++ workload)
- Linux: xkbcommon, xcb, wayland, alsa, dbus, openssl, fontconfig, freetype dev packages (or `nix develop` on NixOS)

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

macOS architectures:

Releases ship **one asset per architecture**, not a universal binary. `Look.app` is 98.9% Mach-O (a 28 MB executable against 316 KB of resources), so a fat binary would roughly double every Apple Silicon download to carry a slice those users never execute.

| asset                             | architecture |
| --------------------------------- | ------------ |
| `Look-<version>-macOS.zip`        | arm64        |
| `Look-<version>-macOS-x86_64.zip` | x86_64       |

Build either locally, from the repo root:

```bash
./scripts/release-macos-app.sh 0.1.1               # arm64, the default
ARCHS=x86_64 ./scripts/release-macos-app.sh 0.1.1  # Intel
```

The script pins `-destination "generic/platform=macOS"` ("Any Mac") and passes `ARCHS` explicitly. Without the destination, `xcodebuild` resolves the concrete "My Mac" destination and filters `ARCHS` down to the host arch, which is why every release before 0.7.2 was arm64 only. It then asserts the built binary is exactly the requested slice, so a collapsed build fails instead of shipping.

Cross-compiling to Intel needs the target on the **nightly** toolchain, because that is what the FFI phase uses:

```bash
rustup target add x86_64-apple-darwin --toolchain nightly
```

Standalone cargo targets, for checking a cross break without a full app build:

```bash
make core-build-x86_64   # core workspace for x86_64-apple-darwin (release)
make ffi-build-x86_64    # ffi crate for x86_64-apple-darwin (release)
```

**The Rust FFI phase.** `apps/macos/build-rust-ffi.sh` builds the ffi crate for whatever `$ARCHS` Xcode is building and leaves the result at `RustBuild/liblook_ffi.a`. When `$ARCHS` is a single arch and it matches your host, which is every Debug dev loop, it builds without `--target` and keeps `bridge/ffi/target/<profile>` as its only cargo cache, so an Apple Silicon dev loop needs no cross target installed and no second cache. Anything else builds per slice with `--target` and `lipo`s them, which is what an Xcode GUI build against "Any Mac" asks for.

A failed cross slice is fatal. Only a host-arch Debug build falls back to reusing the previous `liblook_ffi.a`, and it says so:

```
warning: Rust build skipped; using existing RustBuild/liblook_ffi.a
```

If you see that during an `xcodebuild` run, find the real cargo error by running the phase standalone:

```bash
cd apps/macos && CONFIGURATION=Debug PROJECT_DIR="$PWD" ./build-rust-ffi.sh
```

**Release builds are stripped.** Release sets `STRIP_INSTALLED_PRODUCT` with `STRIP_STYLE = all`, which takes the binary from 28.1 MB to 14.4 MB because `__LINKEDIT` alone was 14.7 MB of local and debug symbols from the Rust staticlib. `DEBUG_INFORMATION_FORMAT` is `dwarf-with-dsym`, so the dSYM in DerivedData is what symbolicates a crash report. Debug builds are not stripped, and `make symbols` inspects the Debug build, so it keeps working. Stripping invalidates a code signature, so Xcode strips before it signs and the release workflow re-signs afterwards.

**FFI phase sandboxing.** The Look target sets `ENABLE_USER_SCRIPT_SANDBOXING = NO`: Xcode's user-script sandbox refuses to read repo files, including the phase script itself, so cargo cannot run sandboxed. The FFI phase is the only shell phase in the project, so the opt-out is scoped to it.

Linows dev run: `cd apps/linows && cargo run` (release: `cargo run --release`; on NixOS prefix with `nix develop -c`). Per-distro and Windows `vcvars` specifics are in [apps/linows/BUILDING.md](apps/linows/BUILDING.md).

Run the local dev app, macOS/Windows (from repo root):

```bash
make app-run
```

`make app-run` behavior (macOS):

- builds a local Debug app bundle with Xcode
- stops any running `Look` process (including a Homebrew-installed instance)
- launches with `LOOK_CONFIG_PATH=$HOME/.look/config.dev`
- shows a red `TEST APP` badge so the dev run is visually distinct

`make app-run` behavior (Windows):

- stops any running `lookapp` process
- runs `cargo run -p lookapp` for the linows app (`apps/linows/`) under the VS 2022 `vcvars` environment
- `make app-run-release` runs the release build instead; `make tauri-run` starts the Tauri backup

Install a side-by-side test build (`Look Dev`) without replacing the normal install (macOS only):

```bash
make app-run-dev
```

`make app-run-dev` (macOS) builds a local Debug bundle, installs `/Applications/Look Dev.app` with bundle id `noah-code.Look.Dev`, leaves the Homebrew `/Applications/Look.app` untouched, then launches `Look Dev` with `LOOK_CONFIG_PATH=$HOME/.look/config.dev`. On Windows there is no separate dev install; use `make app-run` (hot reload) or `make app-run-release`.

`lookapp` is a symlink to the **installed** app (`scripts/install-look.sh`), so it always runs the release binary no matter what you just built. `make app-install-dev` installs `lookdev` beside it as the same handle for the dev build:

```bash
lookdev                 # launch it with the dev config
lookdev clipboard       # open it in a mode
lookdev --list-modes
```

Install it on its own with `make dev-cli`. It reads `LOOK_DEV_APP` and `LOOK_DEV_CONFIG` if you keep them elsewhere.

On Linux there is no `lookdev`. The debug binary is the handle, and it already points at the dev config and database:

```bash
cd apps/linows
nix develop -c cargo build          # NixOS; elsewhere plain cargo build
./target/debug/lookapp --list-modes    # prints and exits, no window
./target/debug/lookapp clipboard       # cold start
./target/debug/lookapp files report    # run again while it is up: the warm path
```

Both routes end at the same parked query (`take_launch_query`), but they reach it differently, so a mode is worth trying from cold and from a running instance.

A debug build separates its config and database (`setup_dev_env`). A second launch hands its arguments to the running instance over the control socket (`$XDG_RUNTIME_DIR/lookapp.sock`, or `lookapp.sock` in the temp dir when `XDG_RUNTIME_DIR` is unset; port 47811 on Windows), and on Linux a Look that owns `com.look.Desktop` without a socket is toggled instead, as a second instance would be on macOS. So quit the installed app before testing a dev build, or run the dev build as a measurement instance: `LOOK_PACE_PROBE=1` with its own `LOOK_CONTROL_SOCKET` (`LOOK_CONTROL_PORT` on Windows), which takes neither the bus name nor the hotkey.

Override the macOS dev config path:

```bash
make app-run-dev DEV_CONFIG_PATH="$HOME/.look.qa.config"
```

`make help` lists every target available on the current host (macOS or Windows).

## Benchmarks

All benches live in a separate `tools/perf` crate. Nothing in `apps/` or `bridge/` depends on it, so they never end up in a shipped binary.

```bash
cd tools/perf
cargo run --release --bin query_engine_bench     # query throughput + fuzzy scoring micro-bench
cargo run --release --bin scoped_refresh_bench   # per-call latency: ALL / APPS_ONLY / FILES_ONLY
cargo run --release --bin watcher_stress         # simulated event streams, BEFORE vs AFTER
cargo run --release --bin real_fs_stress         # real notify watcher + worker doing real disk I/O
```

Watcher / index-refresh methodology, scenarios, and a side-by-side report live at [tools/perf/WATCHER_PERF.md](tools/perf/WATCHER_PERF.md).

Benchmark snapshots land under [docs/bench-notes/](docs/bench-notes/). Add a new snapshot when scoring, matching, or indexing changes.

## Releasing (maintainers)

Build release artifacts. Each call produces one architecture; release CI runs both in parallel:

```bash
./scripts/build-release.sh 1.0.0                # arm64
ARCHS=x86_64 ./scripts/build-release.sh 1.0.0   # Intel
```

The Homebrew cask lives in [homebrew/cask](https://github.com/Homebrew/homebrew-cask/blob/main/Casks/l/look.rb). BrewTestBot bumps it automatically after a GitHub release; if it misses one, run `brew bump-cask-pr --version <version> look`. Other changes (artifact names, minimum macOS, zap paths) need a regular pull request editing the cask.

The cask serves both architectures through an `arch intel: "-x86_64"` stanza: one `url` interpolates `#{arch}` into the asset name, and `sha256` takes an `arm:` and an `intel:` value (the hashes are in `Look-<version>-manifest.txt` and `Look-<version>-manifest-x86_64.txt`). Keep the Intel asset name in step with that suffix, or the cask breaks for Intel users. BrewTestBot's auto-bump updates both hashes.

The Nix package for macOS pins that same asset by hash, so it needs the same bump. `update-nix-darwin.yml` does it for you: it waits for the macOS release workflow to finish, rewrites the version and hash, and opens a PR that the `build-darwin` check verifies before merge. To do it by hand, or to backfill an older release:

```bash
./scripts/nix-bump-darwin.sh 1.0.0
```

That rewrites `version` and every packaged architecture's hash in one pass, because a bumped version with a stale hash fails the build while a stale version with a fresh hash silently keeps Nix users on the old release. It exits 3 when a release carries an asset for an architecture the file does not package yet, which is the signal to add an `x86_64-darwin` entry to `sources`. The Linux Nix package builds from the tree rather than a release asset, so it needs nothing at release time.

Signing and notarization:

- a paid Apple Developer membership is required for Developer ID signing and notarization
- strict release runs require signing and notary secrets
- non-strict test runs can still build artifacts when secrets are missing

Signing/notarization walkthrough: [docs/apple-developer-release-guide.md](docs/apple-developer-release-guide.md).

## Contribution flow

- every PR targets `main`, maintainer and external alike; there is no long-lived staging branch
- external contributions: branch from `main` in your fork and open the PR into `main`
- run local checks before opening a PR:
  ```bash
  cargo test --workspace --manifest-path core/Cargo.toml
  cargo test --manifest-path bridge/ffi/Cargo.toml
  # if a Cargo.lock changed (needs `cargo install cargo-audit`):
  make audit
  # if touching linows:
  cargo clippy --workspace --all-targets --manifest-path apps/linows/Cargo.toml -- -D warnings
  cargo fmt --all --manifest-path apps/linows/Cargo.toml -- --check
  ```
- update docs when user-visible behavior changes
- see [CONTRIBUTING.md](CONTRIBUTING.md) and the issue templates under [.github/ISSUE_TEMPLATE/](.github/ISSUE_TEMPLATE/)

## Further reading

- [docs/architecture.md](docs/architecture.md) - canonical architecture reference
- [docs/backend-guide.md](docs/backend-guide.md) - backend edit targets and verification
- [docs/user-guide.md](docs/user-guide.md) - user guide
- [docs/features.md](docs/features.md) - feature status
- [apps/linows/BUILDING.md](apps/linows/BUILDING.md) - linows build, packaging, and install methods
