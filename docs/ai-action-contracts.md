# AI contracts - as built

The stable core the AI surface is built on: how input is routed, how an intent
becomes an executable plan, and where each piece lives. Get these right and
adding the Nth capability is a small change in one place.

> Supersedes the original Swift `ActionTool`/`ActionRegistry`/`AIValue` design,
> which was deleted when resolution moved to Rust (see `ai-rust-core-plan.md`).
> This document describes what actually ships.

## The doctrine: three tiers, one typed output

Every AI surface follows the same ladder, and the important rule is that all
three tiers produce the *same* typed intent, so the model is just another
parser and can never reach an execution path the deterministic parser can't.

1. **Deterministic grammar** - instant, runs per keystroke, works with AI off.
   The `@` form, file recall, text-op verbs, memory commands, the window
   grammar, calc, instant answers.
2. **Graceful relaxation** - when tier 1 triggered but found nothing, loosen
   deterministically instead of showing an empty panel (file search widens the
   time window, then drops unmatched terms; schedule questions fall back to a
   7-day window).
3. **Model interpretation** - schema-forced, cancellable, and emitting the same
   typed intent as tier 1. Never free-form text that gets re-parsed. Action
   planning runs while typing (on a 300ms idle) so the preview appears without
   pressing Enter; the *utility* intents (`recall`, `textop`) and the
   disambiguation list are Enter-only, because jumping the panel to file results
   or transforming the clipboard mid-keystroke would be hostile.

Two deliberate exceptions:

- **Memory is tier-1 only.** The model never writes durable facts, so a weak
  planner cannot pollute them. A "remember ..." phrasing the parser misses
  becomes chat, not a memory write.
- **Mutations keep the confirm gate.** Widening tier 3 is safe precisely
  because everything destructive still lands on a preview the user confirms,
  with undo after.

## 1. Routing (`core/ai/src/route.rs`)

ONE ladder, shared by every shell so precedence cannot drift:

```text
memory -> textop -> files -> explicit -> plan -> chat
```

Deterministic tiers run first, most precise first. `plan` appears only when a
capable model is configured; otherwise the ladder ends at `chat`. The shell
calls `look_ai_route(memory_path, input, model_available, now)` and switches on
the returned decision. The memory tier has already executed when it answers
(it is the handler, not a preview).

## 2. The planner wire format (`core/ai/src/plan.rs`, `planner.rs`)

The model emits JSON forced by a schema in Ollama's `format` field, so an
invalid shape is impossible rather than merely unlikely:

```json
{"steps": [{"tool": "<alias>", "params": {...}}]}
```

`steps` is an array from day one, so multi-step plans are a consumer change,
never a wire change (today only `steps[0]` executes). Tool ids are 1-token
aliases mapped to real ids in the planner, which keeps generation short:

| alias | tool id | params |
| --- | --- | --- |
| `event` | `calendar.add_event` | `title` |
| `reminder` | `reminder.add` | `title` |
| `cancel` | `calendar.cancel_event` | `match` |
| `move` | `calendar.move_event` | `match`, `when` |
| `complete` | `reminder.complete` | `match` |
| `delete` | `reminder.remove` | `match` |
| `snooze` | `reminder.snooze` | `match`, `when` |
| `block` | `calendar.block_time` | `duration`, `when?`, `title?` |
| `recall` | `files.recall` | `terms?`, `types?`, `when?`, `location?` |
| `textop` | `clipboard.textop` | `instruction` |

`resolve_step` validates the per-tool requirements and returns `{tool, params}`
with real ids, or nothing when the step is unusable. Two tolerances worth
knowing: the mutate tools (`cancel`, `complete`, `delete`) accept `title` as a
stand-in when the model puts the subject there instead of `match`, and `recall`
is rejected unless at least one of its four facets is present (all four are
individually optional, but an empty recall is not a query).

Planning runs as a **cancellable session** (`look_ai_plan_start` / `_poll` /
`_cancel`) on the same curl transport as chat, so a superseded request is
actually killed rather than left to queue inside Ollama.

## 3. Resolution (`core/ai/src/resolve.rs`)

The shell passes platform data in; the core validates, matches through the
ambiguity gate, computes dates and previews, and returns a data-only outcome.
No closures cross the boundary.

```text
ResolveRequest { tool, params, now, events[], reminders[], resolved_when?, window_* }
   -> ResolveOutcome::Planned { preview_title, preview_detail, summary, subject, execute, undo }
                    | ::Choice { candidates[] }     // ambiguous match
                    | ::Invalid { message }         // missing/unusable input
```

`Execute` and `Undo` are data (`AddEvent`, `MoveEvent`, `SetReminderDue`, ...),
so the shell can perform and reverse an action without the core knowing
anything about EventKit.

Notable rules encoded here: never invent a clock time (a day with no time is an
all-day event), all-day events don't block `block_time` slots, and adding an
event whose title already sits on that day appends "already on your calendar"
to the preview (warn, never block).

## 4. The date seam

The core never does date math on natural phrases. The shell resolves them
(macOS: `NSDataDetector`, which is excellent at day+time combinations) and
passes `resolved_when` back in. Two refinements:

- `window::day_phrase` is the shared-lexicon fallback for phrases the detector
  can't read ("this week wed", "last fri"), so abbreviations mean the same day
  on every shell.
- When both resolve, the **named day wins and the detector supplies the clock
  time**: "tue 9am" typed on a Monday is Tuesday 09:00, not today.

`window::future_leaning` nudges a bare clock time that already passed to the
next day; a phrase naming a day or month is respected as-is.

## 5. The execution spine

Every producer converges on the same path:

```text
ToolCall -> resolve (Rust) -> preview -> confirm -> receipt -> undo
```

Two confirm surfaces, one spine:

- **AI panel**: a pending bar; Enter confirms, Esc cancels.
- **Main bar**: the plan renders as the first, selected result row, and the
  visible row *is* the confirmation - one Enter runs it, ⌘Z undoes. Styling for
  each tool (icon, type badge, verb) comes from `AIActionAppearance`, so a new
  tool is one table entry, not new row code.

## 6. Placement

| Layer | Home | Why |
| --- | --- | --- |
| Routing, planning, resolution, matching, window grammar, lexicon, markdown, chat transport | `core/ai` (Rust) | Shared by every shell; unit-tested without a UI |
| C boundary | `bridge/ffi` | JSON in, JSON out; every export panic-caught |
| Pure Swift helpers (`DatePhrase`, `ScheduleWords`, `LocalHostCheck`, `OllamaCodec`) | `LauncherLogic` package | Foundation-only, unit-tested |
| EventKit, providers, controllers, SwiftUI | app target | Platform-bound |
