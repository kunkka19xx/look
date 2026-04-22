# Backend Guide

This is a contributor-focused guide for the Rust backend.

Architecture shape and diagrams live in `docs/architecture.md`. This file is intentionally narrower: what to edit, where to edit it, and how to verify changes safely.

## What this guide is for

Use this guide when you are changing:

- indexing scope and discovery behavior,
- matching/ranking quality,
- SQLite persistence semantics,
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

### `core/storage`

- `core/storage/src/lib.rs`:
  - schema/migrations,
  - candidate upsert/load APIs,
  - usage event persistence,
  - index state persistence.

### `bridge/ffi`

- `bridge/ffi/src/lib.rs`: exported C ABI.
- `bridge/ffi/src/state.rs`: engine cache and cstring lifecycle.
- `bridge/ffi/src/search_api.rs`: search endpoints.
- `bridge/ffi/src/usage_api.rs`: usage recording endpoint.
- `bridge/ffi/src/translate_api.rs`: translation endpoint + typed errors.
- `bridge/ffi/src/runtime_config.rs`: config loading + runtime toggles.

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

Runtime file: `~/.look.config` (or `LOOK_CONFIG_PATH`).

- `app_scan_roots`, `app_scan_depth`, `app_exclude_paths`, `app_exclude_names`
- `file_scan_roots`, `file_scan_extra_roots`, `file_scan_depth` (default: 4, range: 1-12), `file_scan_limit` (default: 4000, range: 500-50000), `file_exclude_paths`
- `skip_dir_names`
- `backend_log_level`
- `launch_at_login`

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

### App-level smoke check

```bash
make app-run
```

For Launchpad-style testing without replacing Homebrew `Look`, use:

```bash
make app-run-dev
```

This installs and runs `/Applications/Look Dev.app` (bundle id `noah-code.Look.Dev`) side-by-side with the normal app.

After launch, validate:

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
