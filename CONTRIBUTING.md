# Contributing to look

Thanks for contributing.

## Contributor License Agreement

Code contributions require a signed [Contributor License Agreement](CLA.md). It
is a license grant, so you keep the copyright to your work and grant the project
maintainer the right to relicense it, which keeps the option of a commercially
licensed build open without having to track down every contributor later. On
your first pull request the CLA assistant bot will post a comment; reply to it
once and the signature covers everything you send afterwards.

## Open an issue before a big change

For anything beyond a small fix, open an issue first and wait for a maintainer
to agree on the shape of it. That covers:

- a new feature, or a new Quick Action control
- a new search source, provider, or external integration
- a UI change that alters existing layout or behavior
- a new dependency, or a change to packaging, release, or CI
- a refactor that moves or renames code across crates

A large PR that arrives with no prior issue may be asked to go back to an issue
first, so the discussion saves you the rework. Link the issue from the PR
description (`Closes #123`).

Open a PR directly for: bug fixes, docs, tests, and small self-contained
improvements.

## Before you open an issue

- search existing issues first to avoid duplicates
- use a clear title with area prefix when possible (`ui:`, `engine:`, `indexing:`, `ffi:`)
- include enough context so someone else can reproduce quickly

## Bug reports

A good bug report must include:

- expected behavior
- actual behavior
- exact reproduction steps (numbered)
- frequency (`always`, `sometimes`, `once`)
- environment details:
  - OS + version (macOS 15.x / Windows 11 24H2 / etc.)
  - look app version or commit SHA
  - install method:
    - macOS: Xcode run, zip install, Homebrew
    - Windows: NSIS installer (`.exe`), or a local `make app-run` dev build
    - Linux: `.deb`, AppImage, AUR, or a local `make app-run` dev build
  - architecture (`arm64` / `x86_64` on macOS; `x64` / `ARM64` on Windows)
- logs or screenshots if available

If crash related, include:

- crash dialog text
- macOS: stack trace or Xcode console output
- Windows: contents of `%LOCALAPPDATA%\look\look-crash.log`, plus `Get-WinEvent -LogName Application -MaxEvents 10` filtered to `lookapp`
- whether it happens on clean launch

## Feature requests

Please include:

- problem statement (what pain exists today)
- proposed behavior
- alternatives considered
- impact/risk (perf, UX, safety)

User-facing feature documentation lives in `docs/` (`features.md`, `user-guide.md`) and is enough for end users and most contributors. Maintainers may also keep internal design specs under `specs/` (purpose, behavior, edge cases, non-goals); that directory is `.gitignore`d and not required reading.

## Adding a Quick Action control

Quick Actions are the interactive toggles/buttons in the launcher's right panel (e.g. Bluetooth on/off). Adding one is designed to be small: one shared descriptor, one native adapter file, one registry line. See [docs/writing-controls.md](docs/writing-controls.md) for the step-by-step guide and the reference implementation.

## Development setup

[DEVELOPMENT.md](DEVELOPMENT.md) has the full per-platform prerequisites and build walkthrough. In short: Rust stable plus GNU Make everywhere, with Xcode on macOS, Visual Studio 2022 Build Tools (Desktop C++ workload) on Windows, and the xkbcommon/xcb/wayland/alsa/dbus/fontconfig/freetype dev packages on Linux.

### The two app shells

The Rust core in `core/` is shared. The UI is not:

- `apps/macos/` is a native SwiftUI app, talking to the core through the C ABI
  in `bridge/ffi/`
- `apps/linows/gpui/` is the shipping shell on Linux and Windows, GPU-drawn with
  [gpui](https://www.gpui.rs), talking to `apps/linows/backend/` in-process.
  Run it with `cd apps/linows && cargo run`
- `apps/linows/src-tauri/` is the old Tauri shell, kept only as a fallback
  (`cargo build -p lookapp-tauri`). Do not build new features on it

Before opening a PR, run the core checks:

```bash
cargo test --workspace --manifest-path core/Cargo.toml
cargo test --manifest-path bridge/ffi/Cargo.toml
```

Plus the checks for the shell you touched. The pull request template lists the
exact commands per area.

## Branch and PR flow

- `main` is the only long-lived branch; every PR targets it
- `main` must stay releasable at all times, since there is no staging branch behind it
- releases are cut by dispatching the release workflows with an explicit version, so nothing needs to be frozen on a branch

Suggested local flow:

```bash
git fetch origin
git checkout main
git pull --ff-only origin main
git checkout -b feat/short-description
```

Before opening PR:

- rebase onto latest `main`
- run local checks from the Development setup section
- ensure docs are updated when behavior changes

## Git guidelines for contributors

- always branch from `main`; do not develop directly on `main`
- keep branches short-lived and focused to reduce merge conflicts
- update your branch from `main` before requesting review:

```bash
git fetch origin
git checkout <your-branch>
git rebase origin/main
```

- if your branch is shared and you want to avoid rewriting history, merge instead of rebase:

```bash
git fetch origin
git checkout <your-branch>
git merge origin/main
```

- do not merge or cherry-pick individual `main` commits into your branch to catch up. Your PR then carries duplicate copies of commits that already exist upstream, and each one becomes a conflict. Rebase (or merge `origin/main` whole) instead
- if GitHub shows "This branch has conflicts": resolve locally, run checks, then push the resolution commit
- avoid force-pushing shared branches unless maintainers explicitly agree
- when adding limits/thresholds for safety or validation, prefer named constants over magic numbers

## CI behavior

CI runs for pushes to `main` and for pull requests targeting `main`.

- Rust jobs (`lint`, `test`, `rust-platform-check`, `security`, release `build`) run only when `core/**` or `bridge/ffi/**` changes
- secrets scanning (`gitleaks`) always runs
- macOS app build runs only for PRs to `main` when `apps/macos/**` or `bridge/ffi/**` changes
- macOS release smoke runs when the release scripts or the Xcode project change
- linows (gpui) builds on Linux and Windows, plus the JS tests, run when `apps/linows/**` or `core/**` changes
- the Nix build runs when the linows sources, packaging, or flake change
- release-style Rust build artifacts run only on push to `main`

## Pull request checklist

The template at `.github/pull_request_template.md` groups the Testing section
into one block per area, each with a comment saying when to delete it. Delete
the blocks your change does not touch, and tick only what you actually ran: a
ticked box is a claim that it passed on your machine. The final checklist comes
pre-ticked, so untick anything that is not true.

Beyond that:

- scope is focused and minimal
- base branch is `main`
- a new or large feature links the issue it was agreed in
- docs updated when behavior changes (`docs/features.md`, `docs/user-guide.md`)
- no unrelated formatting-only changes
- PR description explains why this change is needed

## Commit style

Keep commits small and descriptive.

Recommended prefixes:

- `fix:` bug fix
- `feat:` new behavior
- `docs:` documentation
- `refactor:` internal cleanup without behavior change
- `test:` tests only
