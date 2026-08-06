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
3. **Chat** - anything the planner declines becomes a chat turn on Enter: the
   question and a streamed answer join the session. Context is the last ~10
   session items, including performed actions (as `[Done: ...]`), so follow-ups
   like "what did I just add?" work. The chat prompt states that modifying or
   deleting calendar items is not supported yet.

All actions flow through the same spine regardless of producer:
`ToolCall -> registry.plan -> preview -> confirm (Enter) -> receipt -> undo`
(see `ai-action-contracts.md`).

## Lifecycle (Esc ends, everything else suspends)

- **Enter** on a pending bar confirms; the query resets to a bare `>` and the
  session continues.
- **Esc** is two-step: with a pending confirm it cancels just that; otherwise it
  ends the session (archives, clears) and returns home.
- **Cmd+Space (hide/recall)** suspends: the session and the query text survive,
  so recall lands back in the conversation.
- **A normal (non-`>`) query** yields the screen to search but keeps the session
  alive in the background; `>` returns to it. Moving on closes the undo window.
- **Cmd+Z** undoes the last action while its session item is undoable; otherwise
  it passes through to normal text-field undo.

## Persistence

Incremental JSONL transcript, one line per item the moment it completes
(quit-safe), at `~/Library/Application Support/Look/ai-sessions.jsonl`:

```json
{"ts":"2026-08-06T12:00:00Z","session":"<uuid>","kind":"user","text":"..."}
```

Kinds: `action`, `user`, `answer`, `undo`. Lines share a `session` id so a
conversation can be reassembled. Esc sweeps anything unarchived (e.g. a partial
answer cut off mid-stream). Placeholder-only answers are skipped. No browsing UI
yet; the file is human-readable.

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
