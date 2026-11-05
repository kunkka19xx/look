# Proposal

## Why
There is no quick way to find and use an emoji from the launcher; the OS picker's search quality and availability differ per platform.

## Intent
Add an `/emoji` command panel, alongside `/calc` and `/pomo`, that lets users search for an emoji and copy or insert it, on both macOS and linows. Search logic is shared through a new `core/emoji` crate.

## Problem Statement
There is no quick way to find and use an emoji from the launcher. Users must open the OS picker, whose search quality and availability differ per platform. The launcher already has the patterns needed (shared Rust core, per-platform command panels, clipboard copy, and paste into the frontmost app) but no emoji feature.

## Capabilities

### New Capabilities
- `emoji-command`: `/emoji` panel with name/keyword search, copy, insert into the previously focused app, and recents.

### Modified Capabilities
- None (`openspec/specs/` is empty).

## Scope

### In-Scope
- New `core/emoji` crate with a bundled CLDR-derived dataset (names, keywords) and a search API.
- Bindings: FFI bridge for macOS, Tauri command for linows.
- `/emoji` command registered in the engine mode registry, macOS (`AppConstants`, command-mode dispatch, panel view) and linows (`catalog.js`, `screens/commands/emoji.{js,html}`).
- Enter copies the selected emoji to the clipboard.
- Shift+Enter inserts it into the previously focused app via the existing `FrontmostAppPaste.swift` and `keysynth.rs` helpers; falls back to copy when insertion is unavailable (e.g. macOS Accessibility denied).
- Recents list, persisted across launches.
- Unit tests for the search crate; tests for command registration where existing patterns have them.

### Out-of-Scope (YAGNI)
- Skin-tone selection and category browsing.
- Emoji results in the main (non-command) search.
- Native OS emoji APIs or pickers.
- User-defined or custom emoji and stickers.
- Runtime dataset downloads or updates.
- Emoji insertion on platforms other than macOS and Windows.

## Assumptions
- **ASM-GEN-1**: The dataset is bundled at build time and refreshed manually on Unicode releases.
- **ASM-GEN-2**: Search is case-insensitive substring/prefix matching over names and keywords; fuzzy ranking beyond that is not required.
- **ASM-GEN-3**: Recents use the same persistence mechanism other commands (e.g. `/todo`, `/pomo`) use on each platform.
- **ASM-GEN-4**: Insertion reuses existing paste helpers without new permissions beyond those already requested.

## Success Metrics
- Typing in the `/emoji` panel returns relevant emoji on both macOS and linows from the same shared crate.
- Enter places the emoji on the clipboard; Shift+Enter inserts it into the prior app, or copies with a visible notice when insertion is unavailable.
- Recents survive an app restart.
- `core/emoji` unit tests pass; existing tests remain green.

## Review Gate
- [x] **Gate 1 Approval**: Proposal reviewed and approved by human before drafting specs.
