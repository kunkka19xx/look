# The AI Session (`>`) - as built

The `>` prefix is Look's explicit "talk to AI" surface. It owns the panel area
the way command mode and translation do: one coherent screen where actions,
questions, and answers stack as a session. Everything here is macOS, Swift-side,
powered by the user's Ollama model (see `ai-vision.md` for provider strategy).

## The screen

- Typing `>` switches the panel to the session screen (search and the web answer
  card are suppressed). The empty state teaches the three uses: add an event,
  add a reminder, ask a question.
- Session items stack chronologically and auto-scroll: completed actions
  ("Added \"Lunch\"" with Undo), user questions ("› ..."), and streaming
  answers (markdown-rendered).
- Footer: `Enter run · Esc leave · Cmd+Z undo · @ sets exact time`.

## Input paths (three producers, one spine)

1. **Explicit `@` form** - `>add lunch @ 1pm`, `>remind call mom @3pm`.
   Deterministic parser (`ExplicitActionParser`), instant, no model. The parser
   handles ONLY this delimited form; it never guesses at natural language.
2. **Natural-language action** - `>add walk my dog on sat 9am`. The model
   planner classifies + normalizes the title; dates are extracted in code (see
   Speed below). While typing, a plan runs on a 300ms idle so the confirm bar
   appears without pressing Enter; non-actions stay quiet.
Provider degradation: with no capable planner (e.g. Apple Intelligence
selected), the `@` form runs the parser in lenient mode (verbatim title, date
resolved from the whole phrase so the day stays correct) and chat falls back to
the selected provider's single-turn answer stream. `>` never dead-ends.

3. **Chat** - anything the planner declines becomes a chat turn on Enter: the
   question and a streamed answer join the session. Context is the last ~10
   session items, including performed actions (as `[Done: ...]`), so follow-ups
   like "what did I just add?" work. The chat prompt states that modifying or
   deleting calendar items is not supported yet.

All actions flow through the same spine regardless of producer:
`ToolCall -> registry.plan -> preview -> confirm (Enter) -> receipt -> undo`
(see `ai-action-contracts.md`).

## AI mode and lifecycle

`>` is a MODE, not a per-message prefix: typing `>` consumes the prefix and
enters AI mode (sparkles icon + its own placeholder in the input bar); every
message after is AI input until Esc leaves.

- **Enter** on a pending bar confirms; the input clears and the mode continues.
- **Esc** is two-step: with a pending confirm it cancels just that; otherwise it
  saves the conversation and leaves the mode for home.
- **Cmd+Space (hide/recall)** suspends: mode and conversation survive.
- **Cmd+Z** undoes the last action while its session item is undoable; otherwise
  it passes through to normal text-field undo.

## Conversations (bounded memory)

Stored in one human-readable JSON file,
`~/Library/Application Support/Look/ai-conversations.json`, upserted
incrementally as items complete (quit-safe). Bounds: 20 conversations, last 60
items each; the model context per turn stays capped at the last 10 items, so
continuing an old conversation carries reasonable, bounded weight.

Empty AI mode shows the recent conversations: typing searches them (title +
content), **number + Enter** (or click) continues one - the full transcript
restores and the chat picks up with context - and typing a real prompt starts a
fresh conversation. While browsing the list, no model calls fire; Enter drives
everything (instant `@` forms still preview live).

## Markdown in answers

`ChatMarkdown.segments` (package, tested) splits fenced code blocks from prose;
code renders monospaced in a darker card (works mid-stream: an unclosed fence
renders as code), prose gets inline markdown (bold/italic/`code`/links) via
Apple's `AttributedString(markdown:)`. Deliberately not a full markdown engine;
no third-party dependency.

## Speed design (the latency contract)

Speed is priority one. The levers, measured on qwen2.5-coder:7b:

- **The model emits only intent + a clean title** (~15-20 tokens). Tool ids are
  1-token aliases ("event"/"reminder") mapped to real ids in the planner. The
  `when` date is extracted from the raw query in code by `NSDataDetector`
  (its date *value* is robust; only its text range ever was not).
- **Static system prompt** (never includes the date), so Ollama's prompt-prefix
  cache eliminates reprocessing (~2.3s -> ~0.15s).
- **Warm-up primes model + prompt cache** when a `>` query starts, throttled.
- **Single-shot planning** (no repair loop): the fields a repair round could fix
  no longer come from the model.
- **300ms idle before planning**; an in-flight call cancels cleanly on the next
  keystroke (client disconnect aborts Ollama generation).
- `keep_alive: 30m` holds the model resident; `num_predict` caps runaway output.

Measured: plan ~0.9-1.0s warm, non-action decline ~0.4s, `@` form 0ms.

## All-day and undated (never invent a time)

- Day but no clock time ("sarah's birthday march 5") -> all-day event that day.
- No date at all -> all-day today (events) / undated (reminders).
- Clock-time detection is lexical (`DatePhrase.hasClockTime`), not a guess.

## Future (rides this design)

- Move/cancel/complete tools: session context makes "remove it" unambiguous,
  shrinking the ambiguity-gate problem (see `ai-eventkit-connector.md`).
- Recall hits and read-queries render as session item kinds in the same panel.
- Per-verb models (small fast planner model, larger chat model) if wanted.
