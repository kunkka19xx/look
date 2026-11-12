# Backend Guide

This is a contributor-focused guide for the Rust backend.

Architecture shape and diagrams live in `docs/architecture.md`. This file is intentionally narrower: what to edit, where to edit it, and how to verify changes safely.

## What this guide is for

Use this guide when you are changing:

- indexing scope and discovery behavior,
- matching/ranking quality,
- SQLite persistence semantics,
- web answers / translation lookups (`core/answers`),
- user-declared sources, connectors, and lookbook installs (`core/sources`, `core/engine`),
- FFI payloads between Swift and Rust.

## Module map (edit targets)

### `core/engine`

- `core/engine/src/config.rs`: tunables, weights, hints, limits.
- `core/engine/src/query.rs`: query parsing and prefixes (`a"`, `f"`, `d"`, `r"`).
- `core/engine/src/search.rs`: search orchestration and result flow.
- `core/engine/src/scoring.rs`: score composition, biases, top-k helpers.
- `core/engine/src/index/mod.rs`: index orchestration.
- `core/engine/src/index/apps.rs`: app discovery and app excludes.
- `core/engine/src/index/files.rs`: file/folder discovery and excludes.
- `core/engine/src/index/settings.rs`: curated System Settings catalog.
- `core/engine/src/lib.rs`: bootstrap, cache refresh, integration surface.
- `core/engine/src/index/sources.rs`: indexing declared source blocks.
- `core/engine/src/sources.rs`: what a declared row does (its block, Enter, `then` targets, preview, the Extend inventory).
- `core/engine/src/connectors.rs`: the long-lived processes behind `connect` blocks.
- `core/engine/src/lookbook.rs`: reading a lookbook entry before installing it, and installing it. No network: the shell fetches.
- `core/engine/src/refcon.rs`: the reference connector (`refcon` binary), also the connector test fixture.

### `core/sources`

- `core/sources/src/def.rs` / `load.rs`: parsing a declaration and reading the sources and connectors directories.
- `core/sources/src/connect.rs`: the connector wire protocol (frames only; the engine owns the process).
- `core/sources/src/check.rs`: whether a declared block can work on this machine.
- `core/sources/src/install.rs`, `setup.rs`, `toggle.rs`: writing a declaration into place, filling its `[[setup]]` values, turning it off.
- `core/sources/src/detail.rs`: the JSON blocks a `preview` may print.

### `core/markdown`

- `core/markdown/src/lib.rs`: markdown blocks for the preview pane and connector previews, and segments for AI answers. Not CommonMark; unrecognised syntax stays literal.

### `core/answers`

- `core/answers/src/lib.rs`: public entry points (`instant_answer`, `has_match`, suggestions, `translate`).
- `core/answers/src/sources/`: per-provider lookups (`currency.rs`, `weather.rs`, `crypto.rs`, `knowledge.rs`, `suggest.rs`).
- `core/answers/src/http.rs`: blocking `curl` subprocess transport (no async runtime).
- `core/answers/src/translate.rs`: translation logic behind the FFI and linows backend translate endpoints.
- Keep entry points best-effort and panic-free: return "no answer" on any failure.

### `core/storage`

- `core/storage/src/lib.rs`:
  - schema/migrations,
  - candidate upsert/load APIs,
  - usage event persistence,
  - index state persistence.

### `core/todo`

- `core/todo/src/lib.rs`: `TodoStore` (open/list/save/prune) over the `todo_tasks` table in the app's `look.db`; the JSON task shape is pinned by tests since both app shells decode it.
- `core/todo/examples/seed.rs`: seeds demo task history into a database (`dev` target by default: the `look.dev.db` file that dev builds read automatically).

### `bridge/ffi`

- `bridge/ffi/src/lib.rs`: exported C ABI.
- `bridge/ffi/src/state.rs`: engine cache and cstring lifecycle.
- `bridge/ffi/src/search_api.rs`: search endpoints.
- `bridge/ffi/src/usage_api.rs`: usage recording endpoint.
- `bridge/ffi/src/translate_api.rs`: translation endpoint + typed errors.
- `bridge/ffi/src/answers_api.rs`: instant/web answer endpoints (C ABI over `look_answers`).
- `bridge/ffi/src/seed_api.rs`: seed externally-discovered candidates (e.g. Windows UWP apps) into storage.
- `bridge/ffi/src/todo_api.rs`: todo list/save endpoints (JSON over C ABI, backed by `core/todo`).
- `bridge/ffi/src/runtime_config.rs`: config loading + runtime toggles.
- `bridge/ffi/src/sources_api.rs`, `connectors_api.rs`, `lookbook_api.rs`, `markdown_api.rs`: transport over the engine's sources, connectors, lookbook and markdown entry points.

## Common change recipes

### Tune ranking quality

1. Start in `core/engine/src/config.rs` for weights/hints.
2. Adjust scoring in `core/engine/src/scoring.rs` only if config knobs are insufficient.
3. Keep heuristics centralized; avoid ad-hoc constants in unrelated files.

### Change indexing scope

1. Update defaults and limits in `core/engine/src/config.rs`.
2. Update source-specific logic in `core/engine/src/index/apps.rs`, `core/engine/src/index/files.rs`, or `core/engine/src/index/settings.rs`.
3. Verify behavior with a realistic local config.

### Change persistence behavior

1. Update schema/API behavior in `core/storage/src/lib.rs`.
2. Keep migration compatibility explicit.
3. Re-check bootstrap and cache refresh flow in `core/engine/src/lib.rs`.

### Change FFI contract

1. Add/update endpoint in `bridge/ffi/src/*_api.rs`.
2. Keep error payloads structured and stable.
3. Ensure string allocation/free paths remain balanced (`look_free_cstring`).
4. Coordinate corresponding Swift bridge updates.

## Runtime config keys (backend-relevant)

Runtime file: `~/.look/config` (or `LOOK_CONFIG_PATH`).

- `app_scan_roots`, `app_scan_depth`, `app_exclude_paths`, `app_exclude_names`
- `file_scan_roots`, `file_scan_extra_roots`, `file_scan_depth` (default: 4, range: 1-12), `file_scan_limit` (default: 8000, range: 500-50000), `file_exclude_paths`
- `skip_dir_names`
- `lazy_indexing_enabled` (default: true) - when true, launcher-open refresh runs only when the index is dirty
- `localized_app_names` (default: false, macOS 15.4+) - when true, resolves localized display names for apps and System Settings entries at index time via Foundation bundle metadata. Apps take the localized name as their title; settings panes render as `<localized> (<English>)` so English queries keep reaching them. Older macOS versions and other platforms keep the non-localized names.
  - Cost: measured on a 134-app machine, the app+settings index pass goes from ~3 ms to ~35 ms on the first run, then ~10 ms once Foundation's bundle cache is warm. The per-bundle Info.plist reads are spread over a small thread pool, which is what keeps the first run cheap. This runs at index time, not per keystroke. Foundation retains that metadata for the process lifetime, so memory grows with the size of the app catalog.
- `backend_log_level`
- `launch_at_login`
- `add_to_path` (Windows only, no default) - registers the install directory in `HKCU\Environment` PATH so `lookapp <mode>` runs from a terminal. Synced on every launch, so an update that lands elsewhere re-registers. An absent key leaves PATH untouched.

Behavior:

- one `key=value` per line,
- `#` comments supported,
- unknown keys ignored,
- invalid values fall back to defaults.

## Verification checklist

### Build checks

```bash
cd core && cargo check --workspace
cd ../bridge/ffi && cargo check
```

### Test checks

```bash
cargo test --workspace --manifest-path core/Cargo.toml
cargo test --manifest-path bridge/ffi/Cargo.toml
```

### Dependency audit

After a `Cargo.lock` changes, run `make audit` (needs `cargo install cargo-audit`). It runs `cargo audit` in `core`, `bridge/ffi` and `apps/linows`, as CI does.

### App-level smoke check

Run the app with `make app-run` (see [DEVELOPMENT.md](../DEVELOPMENT.md#building-and-running) for per-platform behavior and the macOS side-by-side `make app-run-dev` build), then validate:

- search returns expected results,
- usage events update ranking after opening items,
- config reload (`Cmd+Shift+;`) applies expected runtime changes,
- lazy indexing mode behavior:
  - `lazy_indexing_enabled=true`: `Cmd+Space` refreshes only when dirty,
  - `lazy_indexing_enabled=false`: `Cmd+Space` always requests background refresh.

## Reliability rules

- keep tunables in config, not scattered literals,
- keep indexing logic inside `index/*`,
- keep FFI narrow and backward-safe,
- propagate actionable errors across boundaries,
- prefer predictable bounded work in query-time paths.
