# Contributing to look

Thanks for contributing.

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
    - macOS: Xcode run, zip install, Homebrew tap
    - Windows: zip release, `make app-run-dev` side-by-side install
  - architecture (`arm64` / `x86_64` on macOS; `x64` / `ARM64` on Windows)
- logs or screenshots if available

If crash related, include:

- crash dialog text
- macOS: stack trace or Xcode console output
- Windows: contents of `%LOCALAPPDATA%\look\look-crash.log`, plus `Get-WinEvent -LogName Application -MaxEvents 10` filtered to `LauncherApp`
- whether it happens on clean launch

## Feature requests

Please include:

- problem statement (what pain exists today)
- proposed behavior
- alternatives considered
- impact/risk (perf, UX, safety)

User-facing feature documentation lives in `docs/` (`features.md`, `user-guide.md`) and is enough for end users and most contributors. Maintainers may also keep internal design specs under `specs/` (purpose, behavior, edge cases, non-goals); that directory is `.gitignore`d and not required reading.

## Development setup

Prerequisites (common):

- Rust stable toolchain
- GNU Make (the top-level `Makefile` dispatches to `Makefile.mac` or `Makefile.win`)

macOS:

- Xcode (for the app shell)

Windows:

- .NET 10 SDK + Visual Studio Build Tools (C++ workload)
- Git Bash — every `make` target must be run from Git Bash, not cmd or PowerShell
- `winget install GnuWin32.Make` if `make` is missing

See [DEVELOPMENT.md](DEVELOPMENT.md) for the full setup walkthrough including the common Windows gotchas.

Checks (cross-platform):

```bash
cargo test --workspace --manifest-path core/Cargo.toml
cargo test --manifest-path bridge/ffi/Cargo.toml
```

## Branch and PR flow

- default contributor target branch is `dev`
- open PRs to `main` only for hotfixes or release-critical patches coordinated with maintainers
- keep `main` stable/releasable; regular feature and refactor work should merge through `dev`

Suggested local flow:

```bash
git fetch origin
git checkout dev
git pull --ff-only origin dev
git checkout -b feat/short-description
```

Before opening PR:

- rebase/merge latest `dev`
- run local checks from the Development setup section
- ensure docs are updated when behavior changes

## Git guidelines for contributors

- always branch from `dev`; do not develop directly on `dev`
- keep branches short-lived and focused to reduce merge conflicts
- update your branch from `dev` before requesting review:

```bash
git fetch origin
git checkout <your-branch>
git rebase origin/dev
```

- if your branch is shared and you want to avoid rewriting history, merge instead of rebase:

```bash
git fetch origin
git checkout <your-branch>
git merge origin/dev
```

- if GitHub shows "This branch has conflicts": resolve locally, run checks, then push the resolution commit
- avoid force-pushing shared branches unless maintainers explicitly agree
- when adding limits/thresholds for safety or validation, prefer named constants over magic numbers

## CI behavior

CI runs for pushes and pull requests targeting `dev` and `main`.

- Rust jobs (`lint`, `test`, `cargo-audit`, release `build`) run only when Rust-related paths change
- secrets scanning (`gitleaks`) always runs
- macOS app build runs only for PRs to `dev`/`main` when Swift files change
- Windows app build (x64 + ARM64) runs when `apps/windows/**`, `bridge/ffi/**`, `core/**`, `scripts/windows/**`, or the workflow itself changes
- release-style Rust build artifacts run only on push to `main`

## Pull request checklist

- scope is focused and minimal
- base branch is `dev` (unless maintainer requested `main`)
- docs updated when behavior changes
- no unrelated formatting-only changes
- tests/checks pass locally
- PR description explains why this change is needed

## Commit style

Keep commits small and descriptive.

Recommended prefixes:

- `fix:` bug fix
- `feat:` new behavior
- `docs:` documentation
- `refactor:` internal cleanup without behavior change
- `test:` tests only
