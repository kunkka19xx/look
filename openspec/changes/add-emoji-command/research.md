# Research & Technical Tradeoffs

## 1. Intent & Ground Truth
Add an `/emoji` command panel, like `/calc` and `/pomo`, on macOS and linows.

Verified in the codebase:
- Commands are per-platform panels over shared Rust logic. `/calc` uses `core/calc`, consumed by macOS via `bridge/ffi/src/calc_api.rs` and by linows via `apps/linows/src-tauri/src/calc.rs`.
- Command registry lives in `core/engine/src/modes.rs`. macOS registers in `AppConstants.swift` (`Command.*`, line ~94 and ~748) and dispatches in `LauncherView+CommandMode.swift`. linows registers in `apps/linows/src/js/catalog.js` and `screens/commands/{name}.js|html`.
- Insert-into-previous-app already exists: macOS `Support/Launcher/FrontmostAppPaste.swift` (Accessibility check + synthesized Cmd+V); Windows `src-tauri/src/platform/windows/keysynth.rs` (`SendInput`).
- No existing emoji search; "emoji" only appears in source-block icons.

Confirmed requirements:
- Both macOS and linows in v1.
- Enter copies the emoji to the clipboard.
- Shift+Enter inserts into the previously focused app; falls back to copy if insertion is unavailable (e.g. Accessibility denied).
- Search over names and keywords.

Open (proposed defaults): persisted recents; panel-only (no inline results in main search); skin tones and categories deferred.

## 2. Options Explored
**A. Shared `core/emoji` crate with bundled CLDR data.**
Pros: offline, identical results on both platforms, mirrors `core/calc`, unit-testable in Rust. Cons: dataset must be bundled and refreshed on Unicode releases.

**B. Native OS emoji APIs / pickers.**
Pros: nothing to bundle. Cons: no clean public search API on macOS; results diverge per platform; hard to test.

**C. User-defined emoji only.**
Pros: tiny. Cons: not a usable emoji picker.

## 3. Tradeoff Matrix

| Criteria | Option A | Option B | Option C |
| :--- | :--- | :--- | :--- |
| **Complexity** | Medium: new crate + 2 thin panels | High: two divergent native integrations | Low |
| **New Dependencies** | One data source (CLDR-derived data or an emoji crate) | Platform APIs | None |
| **Performance & Latency** | In-memory search, fast | Variable | Fast |
| **Maintenance Burden** | Periodic dataset refresh | Per-OS API drift | Low, but feature incomplete |
| **Blast Radius** | New crate, FFI/Tauri bindings, two panels | Platform code on both apps | Config only |

## 4. Recommendation
Option A: a shared `core/emoji` crate, matching how `core/calc` is shared. Keep v1 to search, copy, insert and recents; defer skin tones and categories (YAGNI).

## 5. Breaking Changes & Deprecations
None. `openspec/specs/` contains no specs, so no `REQ-*` / `NFR-*` are modified, superseded or deprecated.

## Review Gate
- [x] option 1
