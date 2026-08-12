# Porting the AI core to Rust

Decision: port the shareable AI logic from Swift into a new `core/ai` crate,
now, before the feature count doubles. Goal: linows gets AI parity from the
same brain, and prompts/matchers/parsers can never drift between shells. This
follows Look's standing doctrine ("every shell talks to the same Rust core"),
which the AI layer currently violates.

The Swift side was deliberately built with a Foundation-only package layer, so
the portable boundary is already drawn; this plan is mostly a translation, not
a redesign. The contract docs (`ai-action-contracts.md`, `ai-session.md`) are
the binding spec for the port.

## Target architecture

```
                 core/ai (new crate)
   planner (Ollama), matcher, referents, window grammar,
   markdown segmentation, conversation store, tool RESOLUTION
        |                                   |
   bridge/ffi (look_ai_* C ABI)      Tauri commands/events
        |                                   |
   macOS shell (Swift)               linows shell (JS)
   EventKit backend + EXECUTION      Win/Linux calendar backend + EXECUTION
   session panel UI                  session panel UI
   NSDataDetector date resolve       Rust date-grammar resolve (fallback)
   Apple Intelligence provider       (no on-device provider)
```

## The boundary rule: resolution in Rust, execution in shells

Closures cannot cross FFI, and today's `PlannedAction.perform/undo` are
closures. So the contract at the boundary becomes data-only:

- Shell gathers platform data (calendar candidates, reminder candidates) and
  passes it in as JSON.
- Rust resolves: classify (model call), match (gate), validate, compute dates,
  build preview text. Returns one JSON outcome:

```jsonc
{ "outcome": "planned",
  "action": { "tool": "calendar.move_event", "preview": {"title": "...", "detail": "..."},
              "target_id": "...",              // what to execute against
              "start_epoch": 1754... , "end_epoch": ...,  // resolved values
              "undo": { "kind": "restore_times", "start_epoch": ..., "end_epoch": ... } } }
// or { "outcome": "choice", "candidates": [{"id": "...", "label": "..."}], "params": {...} }
// or { "outcome": "invalid", "message": "..." }
// or { "outcome": "chat" }   // not an action; shell runs the chat path
```

- Shell executes the resolved action against its calendar API and performs undo
  from the receipt data. Tools stop being Swift structs with closures and
  become Rust resolution functions plus a small shell-side executor switch
  keyed by tool id (one per platform backend, which exists anyway).

This is the one real contract change. Everything else translates 1:1.

## Inventory: port / seam / per-shell

**Port to `core/ai` (delete from the Swift package as each lands):**

| Swift file | Rust fate |
|---|---|
| `OllamaCodec` + `OllamaProvider` (plan/tags/warm) | Ollama client on the existing `curl` transport (`core/answers/src/http.rs` precedent: blocking, no async runtime). POST via `curl -d`; streaming later via child-stdout lines. |
| `ActionPlanner` (prompt, aliases, resolveCall) | planner module; prompts live in Rust, single source |
| `ActionPlanSchema` / `ActionPlanParser` | serde types (wire format unchanged: `steps:[]`) |
| `TitleMatcher` | replaced by the real `core/matching` scorer (an upgrade, macOS already uses it over FFI for kill-targets) + the tier gate |
| `ReferentPhrase` | direct port |
| `DatePhrase.queryWindow` / `normalizeShorthand` / `hasClockTime` | direct port (pure grammar) |
| `ChatMarkdown` | direct port |
| `ConversationStore` | Rust owns the JSON file (same format/caps), exposed to both shells |
| `AIValue` | deleted entirely: `serde_json::Value` is native in Rust |
| Add/mutate tool logic (validation, gate, previews) | resolution functions (see boundary rule) |

**Seams (per-platform behind a shared interface):**
- Calendar/reminder backends: EventKit stays Swift; Windows/Linux backends are
  new work regardless of this port. They feed candidates in and execute
  resolved actions out.
- Natural-language date RESOLUTION ("tomorrow 3pm" -> instant): NSDataDetector
  on macOS (better quality, keep it), a Rust grammar fallback for linows.
  Shells resolve phrases and pass epochs in; the ported window grammar covers
  the listing/read cases in Rust everywhere.

**Stays per-shell (never ports):**
- Session panel UI, AI mode, keyboard handling (SwiftUI vs Tauri JS).
- Apple Intelligence provider (macOS-only by nature).
- Streaming chat consumption UI.

## Surface sketch

FFI (macOS), following the `look_*_json` conventions in `bridge/ffi`:

As shipped (names are the source of truth; see `bridge/ffi/src/lib.rs`):

```
look_ai_route(memory_path, input, model_available, now) -> routing decision JSON
look_ai_plan_start/poll/cancel(...)    -> cancellable planning session
look_ai_resolve(request_json)          -> ResolveOutcome JSON
look_ai_chat_start/poll/cancel(...)    -> streamed chat (curl child + polling)
look_ai_query_window(query, now_epoch) -> window JSON or null
look_ai_day_phrase(phrase, now_epoch)  -> local-midnight epoch, or 0
look_ai_future_leaning(phrase, resolved, now) -> epoch
look_ai_markdown_segments_json(text)   -> segments JSON
look_ai_conversations_json/upsert/delete(path, ...)
look_ai_memory_command/context(path, ...)
look_ai_parse_explicit(input, model_available) -> {tool, params} or null
look_ai_is_referent(phrase)            -> bool
look_ai_is_file_query(query, now)      -> bool
look_search_files_json / look_search_files_params_json -> results JSON
```

Health probing stayed Swift-side (`OllamaProvider.probe` + `OllamaHealthCache`)
rather than becoming `look_ai_ollama_health`: it is a plain `GET /api/tags` plus
a cache, and the shell already needs the availability type for its UI. linows
will want its own equivalent, not this FFI.

linows: the same functions as Tauri commands; streaming as Tauri events
(natural fit). Both shells consume identical JSON.

## Status

- **P0 SHIPPED**: `core/ai` crate; `markdown` + `referent` ported, Swift copies
  deleted, consumed via `look_ai_markdown_segments_json` / `look_ai_is_referent`.
- **P1 SHIPPED**: `window` (query-window grammar on `chrono`, canonical ISO
  Monday weeks for all shells) consumed via `look_ai_query_window`; `plan`
  (wire-format serde + chat_format schema) ready for P2. 18 cargo tests.
- **P2 SHIPPED**: planner (prompt, aliases, mapping) in `core/ai/src/planner.rs`
  on the curl transport; Swift `ActionPlanner` is a thin FFI shell + the
  NSDataDetector date seam. Live-verified against Ollama.
- **P3 SHIPPED**: conversation store in `core/ai/src/conversations.rs` (caps +
  format), shells pass the platform path.
- **P4 SHIPPED**: tool resolution in `core/ai/src/resolve.rs` (validation, the
  ambiguity gate on `core/matching`, dates, previews, undo recipes) behind
  `look_ai_resolve`; the explicit `@` parser in `explicit.rs`. Swift kept only
  the executor (`ActionResolution.swift`), slim types, and the seams. `AIValue`,
  the tool structs, registry, and matcher are deleted from Swift.
- **P5 SHIPPED**: streamed chat sessions in `core/ai/src/chat.rs` (curl child +
  reader thread) behind start/poll/cancel FFI; the macOS session chat polls
  ~12x/sec. Non-Ollama providers keep their native streams. Live-verified.
- **P5.1 SHIPPED**: ONE Ollama client. `chat::start` takes a per-surface options
  JSON (`num_predict`, `temperature`, `timeout_secs`), and the answer card moved
  off its own URLSession implementation onto the same transport, so cancellation,
  timeouts, and error surfacing behave identically everywhere.
- **P5.2 SHIPPED**: the routing ladder itself (`core/ai/src/route.rs`) and one
  shared date/word lexicon (`core/ai/src/lexicon.rs`), replacing four drifting
  word lists. The planner gained `recall` and `textop` aliases, so arbitrary
  phrasing reaches file search and clipboard ops through the same schema-forced
  output as actions.
- **P1 rescope**: `TitleMatcher` and `normalizeShorthand`/`hasClockTime` are
  called from inside the Swift package tools, which cannot reach FFI; they move
  in P4 together with tool resolution rather than growing drift-prone copies.
- **Build gotcha**: the Xcode run-script declares `RustBuild/liblook_ffi.a` as
  its output, so Xcode SKIPS the cargo rebuild whenever the file exists. After
  adding FFI symbols, `rm apps/macos/LauncherApp/RustBuild/liblook_ffi.a` (or
  the link fails with undefined `_look_ai_*`, or silently uses a stale brain).

## Migration order

- **P0** - `core/ai` crate + workspace member + one hello function through both
  FFI and Tauri. Freeze rule starts: no new features land in the Swift package;
  the Swift tests become the parity spec.
- **P1 (pure logic)** - window grammar, shorthand, clock-time check,
  ReferentPhrase, ChatMarkdown, matcher gate on `core/matching`, plan
  wire-format serde. Port the corresponding Swift package tests to `cargo test`
  (~40 tests). Swift files deleted as each is consumed via FFI.
- **P2 (planner)** - Ollama non-streaming client (plan/tags/warm) + prompts +
  `look_ai_plan` end-to-end. macOS switches `ActionPlanner` to the FFI call.
- **P3 (store)** - conversation store in Rust; both shells read/write one file.
- **P4 (resolution)** - tool resolution moves behind `look_ai_plan`'s context
  contract; Swift tools collapse into the executor switch. This is the contract
  change and the biggest single step.
- **P5 (streaming chat)** - chat stream through core (curl child stdout ->
  poll/callback for FFI, events for Tauri). Until then macOS keeps URLSession
  streaming; acceptable duplication because the *prompts/context assembly*
  already moved in P2-P4.
- **P6 (linows parity)** - Tauri session panel + a Windows/Linux calendar
  backend (or ship chat + conversations first, calendar when a backend lands).

Each phase ships independently; macOS behavior must be identical after every
phase (the Swift tests, then Rust tests, enforce it).

## Dependencies to confirm before coding (per repo rules)

- `serde`/`serde_json`: already in the workspace. No new crates required for
  P0-P4: HTTP rides the existing system-`curl` helper (move/share
  `answers::http` or lift it into `core/ai`).
- No async runtime is introduced anywhere (matches the existing doctrine).
- A Rust NL-date crate for the linows resolve fallback (P6): evaluate then, not
  now; the window grammar covers most read cases without it.

## Risks

- **P4 contract change**: closures -> data is the one redesign; do it last of
  the logic phases, after P1-P3 have proven the plumbing.
- **FFI streaming** (P5): the only genuinely fiddly plumbing; deferred, with a
  working URLSession fallback the whole time.
- **Transition duplication**: bounded by the freeze rule and by deleting Swift
  files the moment their Rust replacement is consumed.
