# Windows CI/CD Plan

Tracks the work to take the Windows shell from "builds locally" to "signed, downloadable
release on every tag." Phased so each step ships value independently. Companion to
`docs/windows-port-plan.md` (covers Phase 7 of that plan).

## Decisions (locked 2026-04-28)

- [x] **Package format**: portable zip
- [x] **Architectures**: x64 + arm64 (x86 dropped)
- [x] **Signing**: secret-gated, mirroring macOS — sign if `WINDOWS_PFX_BASE64` is configured, ship unsigned otherwise. `strict` in the ref name forces signed-or-fail.
- [x] **Distribution**: GitHub Releases only (winget deferred to Phase E, post first signed release)
- [x] **Trigger**: tag push (`v*`) + `workflow_dispatch`

MSIX, arm64, and Microsoft Store land in later phases without rework.

---

## Phase A — Windows CI build gate (PR + push)

Goal: every PR that touches Windows code builds on a clean GitHub-hosted Windows runner
before merge. Catches "works on my machine" regressions.

- [x] Add `windows` filter to the existing `dorny/paths-filter` step in `.github/workflows/ci.yml` covering `apps/windows/**`, `bridge/ffi/**`, `core/**`, `scripts/windows/**`, the workflow file
- [x] New `windows-build` job, runner `windows-latest`, gated on the `windows` filter and on PR/push to main+dev
- [x] Set up .NET 10 SDK via `actions/setup-dotnet@v4`
- [x] Set up Rust stable via `dtolnay/rust-toolchain@stable`
- [x] Cache: `actions/cache` for `~/.nuget/packages`, `Swatinem/rust-cache@v2` for cargo target
- [x] Run `dotnet restore apps/windows/LauncherApp/LauncherApp.csproj -p:Platform=${{ matrix.platform }} -r ${{ matrix.rid }}`
- [x] Run `dotnet build ... -c Debug -p:Platform=${{ matrix.platform }} -r ${{ matrix.rid }} --no-restore` (the `BuildRustFfi` MSBuild target wires in the cargo build automatically — see `LauncherApp.csproj:90`)
- [ ] Confirm both arches build on the next PR that touches a `windows`-filtered path
- [x] Matrix `[x64, ARM64]` with corresponding rust target triples
- [x] On failure, upload the binlog (`/bl:msbuild.binlog`) as an artifact for debugging (per-arch)

Exit criteria: PR with intentional XAML/C# error fails the `windows-build` check; clean
PR passes in under 10 minutes.

---

## Phase B — Release artifact pipeline (tag-driven)

Goal: cutting a `v*` tag produces downloadable artifacts attached to a GitHub Release.

- [x] Create `.github/workflows/release-windows.yml` mirrored on `release-macos.yml` (workflow_dispatch with version input + push tags `v*`, calls a reusable workflow, `secrets: inherit`)
- [x] Create `.github/workflows/reusable-release-windows.yml` taking `release_ref`, `trigger_event`, `attach_release`, `concurrency_ref` inputs
- [x] Inside the reusable workflow:
  - [x] Matrix `[x64, ARM64]` with concurrency keyed on `inputs.concurrency_ref` + matrix arch so cells don't cancel each other
  - [x] `dotnet publish -c Release -p:Platform=${{ matrix.platform }} -r ${{ matrix.rid }} -p:PublishProfile=${{ matrix.rid }}.pubxml` (pubxml sets `SelfContained=true`; csproj sets `PublishReadyToRun=true` + `PublishTrimmed=true` for non-Debug)
  - [x] Defensive copy of `look_ffi.dll` from `bridge/ffi/target/${rust_target}/release/` into the publish dir (existing `CopyRustFfiDll` target only copies to `$(TargetDir)`, not `$(PublishDir)`)
  - [x] Zip the publish output as `Look-${version}-windows-${arch}.zip` (`arch` = `x64` or `arm64`)
  - [ ] (Deferred) MSIX packaging — folds in when MSIX is in scope
  - [x] Generate `Look-${version}-windows-${arch}-manifest.txt` with `version=`, `arch=`, `artifact=`, `sha256=`
- [x] Always upload artifacts to the workflow run (debugging) — also uploads `msbuild.binlog` on failure
- [x] When `attach_release == true`, create/update the GitHub Release for the tag and attach all artifacts + checksums
- [ ] Smoke test: cut `v0.0.1-test`, verify artifacts appear; delete the test release/tag

Exit criteria: tag `v1.0.0-beta1` produces a Release with x64 zip + checksum within 15
minutes, downloadable and runnable on a clean Windows VM.

---

## Phase C — Code signing (folded into Phase B)

Implemented as a conditional step inside `reusable-release-windows.yml`, mirroring
the macOS pattern (`Configure macOS signing keychain (optional)` → `Codesign app
bundle (optional)`).

- Required secrets (all optional unless `strict` is in the ref name):
  - `WINDOWS_PFX_BASE64` — base64-encoded `.pfx`
  - `WINDOWS_PFX_PASSWORD` — PFX password
  - `WINDOWS_TIMESTAMP_URL` — RFC 3161 timestamp server, defaults to `http://timestamp.digicert.com`
- Behavior: if PFX secret missing and not strict, skip signing and ship unsigned (matches macOS).
- Strict mode: if the ref name (tag or workflow_dispatch input) contains `strict`, missing secrets cause the job to fail.
- Verify with `signtool verify /pa /v` after signing.
- Release security summary step echoes signing/strict state to `$GITHUB_STEP_SUMMARY` (mirrors macOS).
- Future when MSIX lands: same step signs the `.msix` in addition to the unpackaged `.exe`.

Exit criteria: a release built with PFX secrets present produces a signed
artifact that `signtool verify /pa` accepts; same workflow without secrets
ships an unsigned zip without failing.

---

## Phase D — Versioning unification

Goal: one source of truth for version, driven by the git tag.

- [ ] Audit current version sources: `Package.appxmanifest:14` (`1.0.0.0`), any `<Version>` in csproj, Cargo.toml versions in `bridge/ffi` and `core/*`
- [ ] Pick the canonical source: most projects use `<Version>` in csproj + a build-time injection into the manifest
- [ ] Add `scripts/windows/bump-version.ps1` — takes `-Version 1.2.3`, updates manifest + csproj + (optionally) Cargo.toml versions, prints a diff
- [ ] Wire release workflow to extract version from `github.ref_name` (strip leading `v`), pass as `/p:VersionPrefix=$version` to msbuild, and patch the manifest
- [ ] Document the version bump flow in `docs/windows-release.md` (new file)

Exit criteria: tag `v1.2.3` produces an MSIX whose package version is `1.2.3.0` with no
manual file edits.

---

## Phase E — Distribution channels

Goal: users can install via familiar tooling, not just GitHub download.

- [ ] After first stable signed release: prepare a winget manifest (`Look.Look` package id, or similar) following the `microsoft/winget-pkgs` PR template
- [ ] Submit winget PR with the GitHub Release URL + sha256
- [ ] Update `README.md` and `docs/windows-install.md` with `winget install Look.Look` (after manifest accepted)
- [ ] (Optional, post-v1) Microsoft Store submission: requires Partner Center account, store listing assets, age rating questionnaire. Track separately — not blocking GA
- [ ] (Optional) Auto-update story for non-MSIX users via in-app update check (compare against latest GitHub Release tag)

Exit criteria: `winget install Look.Look` installs the latest signed release.

---

## Cross-cutting items

These don't fit a single phase but should land before declaring v1 done.

- [ ] `docs/windows-install.md` — install / upgrade / uninstall instructions for each format we ship
- [ ] `docs/windows-release.md` — how to cut a release, version bump, signing checks
- [ ] Release notes template under `.github/release-template-windows.md`
- [ ] Update `docs/windows-port-plan.md` Phase 7/8 status as each phase completes
- [ ] Set up a clean Windows test VM (or document the verification steps) for smoke-testing each release candidate
