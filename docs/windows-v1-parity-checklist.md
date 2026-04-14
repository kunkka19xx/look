# Windows v1 Parity Checklist

This checklist defines the required user-visible behavior for the first Windows release.
Scope is derived from current macOS behavior in `README.md` and `docs/user-guide.md`.

## 1) Release scope split

Parity required for Windows v1:

- global hotkey launcher toggle and keyboard-first flow
- app/file/folder search from indexed local sources
- query prefixes: `a"`, `f"`, `d"`, `r"`, `c"`
- core actions: open target (`Enter`), reveal in Explorer, copy selected path/content, web handoff (`Cmd/Ctrl+Enter` equivalent)
- command mode: `calc`, `shell`, `kill`, `sys`
- clipboard history mode (`c"`) with session-local history
- config load/reload parity for indexing and ranking behavior
- stable candidate ID conventions (`app:*`, `file:*`, `folder:*`, `setting:*`)

Can ship after Windows v1 (patch release):

- full translation/dictionary parity (`t"`, `tw"`) when shell UX and networking behavior are finalized
- complete visual/theme parity with every macOS preset variant
- advanced UX polish items that do not change core search/action semantics

## 2) Behavior contracts (must not drift)

### Query behavior

- `a"term` filters to apps only
- `f"term` filters to files only
- `d"term` filters to folders only
- `r"pattern` enables regex search
- `c"term` switches to clipboard history search space
- non-prefixed query keeps blended ranking behavior

### Action semantics

- `Enter`: execute selected result action
- web handoff: open browser search URL using current query
- reveal action opens parent location and selects target in Explorer
- copy action writes selected path/content to clipboard

### Keyboard model

- selection navigation via `Up`/`Down` and `Tab`/`Shift+Tab`
- mode transitions preserve keyboard-only flow (search <-> command <-> clipboard)
- hide/close behavior mirrors launcher expectations on focus loss and explicit dismiss

## 3) Windows-specific mapping notes

- system settings candidates must use `ms-settings:` targets with `setting:*` IDs
- file path handling must support separators (`/` and `\\`) and case-insensitive comparisons where appropriate
- app discovery should prioritize Start Menu entries, with install-root fallback and dedupe

## 4) Performance budgets for Windows release candidate

The project priority is speed. Windows work is accepted only if these targets remain healthy.

- launcher open latency (hot): p50 <= 120ms, p95 <= 180ms
- query latency (local index): p50 <= 35ms, p95 <= 90ms
- startup to first usable result: <= 900ms
- idle CPU: <= 2%
- idle memory envelope: <= 260MB

Notes:

- measure on representative non-dev hardware and real user datasets
- fail parity QA if p95 query latency regresses >10% week-over-week

## 5) Validation checklist

- run fixed smoke query set for apps/files/folders/settings and check top-5 relevance
- validate each query prefix contract with unit/integration tests
- validate result action behavior from keyboard-only flow
- verify duplicate-candidate suppression across app discovery sources
- verify no FFI ABI breaks for `look_search_json_compact`, `look_record_usage_json`, `look_reload_config`, `look_translate_json`, `look_free_cstring`
