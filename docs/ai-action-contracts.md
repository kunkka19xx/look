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

Decoding is deliberately tolerant (`plan::parse_plan`). A schema does not stop
a small local model from adding a stray code fence after the value or dropping
the final brace, and a strict decode read both as declines: qwen3.5:4b scored
61% with one and 97% with the other. Junk after the first complete value is
ignored, unbalanced braces are closed, and a generation cut off mid-string is
still refused, because inventing the rest of a title is worse than declining.

### The prompt is sharded (`core/ai/src/domain.rs`)

One flat prompt listing every tool is what capped the vocabulary at ten: rules
added for one tool cost accuracy on the others, so nothing could be sharpened
and nothing new could be added. Instead, `domain::of` places a request into
`Calendar`, `Reminder`, or `Clipboard` from deterministic word signals, and the
prompt is then built from that domain's rows plus that domain's rules. The
prefilter is conservative: an unplaceable request returns None and sees the
whole table, exactly as before, because a wrong narrow is unrecoverable while a
missing one only forgoes accuracy we did not have. `Files` is never returned;
the strong file shapes never reach the planner (§1) and the rest has no signal.

Two table properties are load-bearing:

- **Order is prompt order.** Alphabetizing the rows cost 3 points of tool
  accuracy on a 7B model. New rows go next to their domain siblings.
- **Rows are self-contained.** A row cannot refer to another row ("same rules
  as above"), because the other row may not be in this shard.

A row may also declare `requires`, a signal the raw request must carry for the
tool to be offered at all. `block` requires `resolve::has_duration_phrase`, so
"add gym session tomorrow 6am" is never even shown the tool it used to be
misfiled under. What is absent from the schema cannot be emitted, which is
stronger than a precondition written in prose.

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

## 7. Measuring it (`core/ai/examples/plan_eval.rs`)

Prompt and lexicon changes are only safe if they are measured: adding a
`duration` param once destabilized classification badly enough that adds fell
through to chat, and nothing caught it. The eval runs a fixture corpus through
the real ladder (`route_json`, then the real planner body for whatever reaches
the model) and scores route, tool, and params separately.

```text
cargo run -p look-ai --example plan_eval                        # ~/.look.config model
cargo run -p look-ai --example plan_eval -- --model qwen3.5:9b
cargo run -p look-ai --example plan_eval -- --routes-only       # no model, instant
cargo run -p look-ai --example plan_eval -- --min 85            # exit 1 below the bar
```

Corpus: `core/ai/fixtures/planner_eval.jsonl`, one JSON case per line
(`input`, `route`, `tool`, `params`, `note`). A `~value` param asserts a
normalized substring, so defensible title phrasings pass. Fixtures state
**desired** behaviour, so a failing case is a real gap whether it sits in the
prompt or in the ladder. Adding a tool means adding its cases first.

Baseline, 77 cases (61 model), Aug 2026, before and after the shard work:

| Model | Tool (flat) | Tool (sharded) | Params | p50 |
| --- | --- | --- | --- | --- |
| qwen2.5-coder:7b | 77% | **88%** | 100% | 0.9s |
| qwen3.5:4b | 75% | **97%** | 100% | 2.0s |
| qwen3.5:9b | 90% | **91%** | 100% | 3.7s |

Params scoring 100% while tools miss is the standing shape: slot extraction is
easy, tool choice is not. Note that a general 4B instruct model beats both a
7B coder model and a 9B one here; the coder model's remaining misses are all
bare adds ("coffee with mark on thursday") that it declines.

Two prompt lessons the corpus paid for, both counterintuitive enough that they
would not have survived a vibe check: sharpening a TOOL LINE (spelling out what
"block" requires) lost 3 points by destabilizing unrelated tools, while the
same words as a DOMAIN RULE cost nothing, and a clock-time fallback in the
prefilter looked obviously right and moved nothing. Change one thing, measure,
keep or revert.

The first bug the corpus caught: `files::parse` triggered on any file-type
word, so "cancel the pdf review meeting" was claimed by file recall and the
planner never saw it. `files::is_scheduling` now vetoes a recall when a
schedule noun appears anywhere, when the opening verb is scheduling-only
(`is_schedule_verb`), or when a reschedule verb is paired with a named day.
File-capable verbs are deliberately excluded from that list, so "delete the
pdfs i downloaded yesterday" is still recall.
