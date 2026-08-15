# The AI Session (`>`) - as built

The `>` prefix is Look's explicit "talk to AI" surface. It owns the panel area
the way command mode and translation do: one coherent screen where actions,
questions, and answers stack as a session. Everything here is macOS, Swift-side,
powered by the user's Ollama model (see `ai-vision.md` for provider strategy).

## The screen

- Typing `>` switches the panel to the session screen (search and the web answer
  card are suppressed). The empty state teaches the three uses: add an event,
  add a reminder, ask a question.
- Turns stack newest-first (the latest exchange is always on top without
   scrolling), while a question stays glued above its own answer inside a turn.
   Items are completed actions ("Added \"Lunch\"" with Undo), user questions,
   and streaming answers.
- Footer adapts: `Esc leave · ⌘Z undo · @ sets exact time`, and while an
   answer streams, `⌘. stop · Esc leave · ⌘Z undo`.

## Input paths (three producers, one spine)

1. **Explicit `@` form** - `>add lunch @ 1pm`, `>remind call mom @3pm`.
   Deterministic parser (`core/ai/src/explicit.rs`, reached via
   `look_ai_parse_explicit`), instant, no model. It handles ONLY this delimited
   form; it never guesses at natural language.
2. **Natural-language action** - `>add walk my dog on sat 9am`. The model
   planner classifies + normalizes the title; dates are extracted in code (see
   Speed below). While typing, a plan runs on a 300ms idle so the confirm bar
   appears without pressing Enter; non-actions stay quiet.
Provider degradation: with no capable planner (e.g. Apple Intelligence
selected), the `@` form runs the parser in lenient mode (verbatim title, date
resolved from the whole phrase so the day stays correct) and chat falls back to
the selected provider's single-turn answer stream. `>` never dead-ends.

3. **Chat** - anything the planner declines becomes a chat turn on Enter: the
   question and a streamed answer join the session. Context is the newest turns
   that fit the token budget, including performed actions (as `[Done: ...]`), so
   follow-ups like "what did I just add?" work. Mutations (move/cancel/complete/snooze/
   block) DO ship: they are typed as instructions and confirmed on the preview
   bar, and the chat prompt says so rather than refusing.

All actions flow through the same spine regardless of producer:
`ToolCall -> resolve (Rust core) -> preview -> confirm (Enter) -> receipt ->
undo` (see `ai-action-contracts.md`). Routing between the producers is itself
shared code (`core/ai/src/route.rs`), so precedence cannot drift between
shells.

## AI mode and lifecycle

`>` is a MODE, not a per-message prefix: typing `>` consumes the prefix and
enters AI mode (sparkles icon + its own placeholder in the input bar); every
message after is AI input until Esc leaves.

- **Enter** on a pending bar confirms; the input clears and the mode continues.
- **Esc** is a three-step ladder: a pending confirm (or disambiguation) cancels
  first; an open chat saves and drops to the sessions list, staying in AI mode;
  from the list it leaves AI mode for home. **Shift+Esc** skips the ladder and
  goes straight home from anywhere in AI mode.
- **Cmd+Space (hide/recall)** suspends: mode and conversation survive.
- **Cmd+Z** undoes the last action while its session item is undoable; otherwise
  it passes through to normal text-field undo.

## Conversations (bounded memory)

Stored in one human-readable JSON file,
`~/Library/Application Support/Look/ai-conversations.json`, upserted
incrementally as items complete. Crash-safe for real: writes go to a temp file
and rename, and a file that fails to parse is moved aside as `.corrupt` rather
than being silently overwritten with an empty list. Bounds: 20 conversations, last 60
items each. The model context per turn is a TOKEN budget, not a fixed count:
the newest turns that fit ~2500 tokens (`core/ai/src/context.rs`), so a long
chat stays coherent without a summarizer and an old conversation resumes with
bounded weight.

Empty AI mode shows the **10 most recent** conversations: typing searches them
(title + content), and **⌘ + a digit** (⌘1 … ⌘9 then ⌘0 for the tenth, matching
the chip on each row) opens one - as does highlighting it with Tab/↑↓ and
pressing Enter, or clicking. The full transcript restores and the chat picks up
with context; typing a real prompt starts a fresh conversation instead.

Ten is a ceiling, not a preference: a ⌘ chord is one keypress, so there is no
⌘10 and no eleventh chip to hand out. The list is capped at the same number
(`AppConstants.Launcher.AISessions.jumpKeyLimit`), so it never grows a row no
chord can reach. Older conversations stay stored and stay findable by typing,
then Tab/↑↓ and Enter.

The digits are free because AI mode hides the running-apps strip that owns ⌘1-9
everywhere else; both handlers gate themselves, so only one can claim the chord.
⌘0 is the one real collision: while the sessions list is on screen it opens the
tenth row instead of the "Actual Size" zoom reset, which stays available
everywhere else.

**`@name` attaches a file.** The suggestion popup is two columns: matches with
their abbreviated paths on the left, a preview of the HIGHLIGHTED file on the
right (`FilePreview`, the same text/Quick Look pair the result pane uses). It
follows the highlight rather than the top match, because with nothing
highlighted Enter still sends the message, and previewing a file the keyboard is
not pointing at would suggest otherwise. Six files called `main.go` is a normal
result, so the path is on every row, and the attachment capsule in the
transcript carries its folder for the same reason: a transcript outlives the
moment it was written in.

**Shift+Enter** inserts a line break instead of sending. The input is the same
`SmoothCaretTextField` as the search bar, so multiline is switched on only for AI
mode (`allowsMultiline`): it wraps and grows to 6 lines and stops there, with the
caret scrolled into view past that. Three things follow from that and must stay
true - a field editor routes Shift+Return to `insertNewline:` as well, so the
delegate decides from the EVENT's modifiers, never from the selector alone (the
selector-only version sent the message instead of breaking the line); the field
editor takes its line mode at begin-editing, so flipping the mode mid-edit
restarts editing and restores the caret; and the caret layer measures x from the
start of the CARET'S line, not from glyph 0, or it walks off the right edge on
every line but the first.

**⌥↑/⌥↓** walks the prompt history (shell style). It used to be ⇧↑/⇧↓, which the
multiline composer needs for selecting text. Two constraints pinned the
replacement: ⌃↑/⌃↓ are Mission Control and Application Windows at the
WindowServer level, so the app never receives them; and the handler must sit
above the monitor's modifier passthrough, which hands every ⌘/⌥/⌃ combo straight
to the system. AI mode consumes the chord even at a boundary, so a press at the
oldest entry stays put instead of moving the caret by paragraph.

**⌘D** deletes the highlighted session (same delete as ⌘⌫ and the row's trash
button, undoable from the banner). In AI mode the chord stops there rather than
falling through to the main bar's "trash the selected file": with a conversation
open there is simply no delete target. **⌘H** opens the help screen from
anywhere in AI mode - the panel branch puts help ahead of the session screen, so
the mode is paused rather than left, and ⌘H again (or Esc, or typing) puts the
conversation straight back. Help is filtered by topic capsules (All / Main / AI
/ Prefixes / Command); arriving from AI mode opens on the **AI** capsule, so the
assistant's keys are the first thing on screen rather than something to scroll
for. The capsules are clickable from any topic, and the screen re-opens on the
topic it was entered from, not the last one clicked.

A TYPED bare number is still not a session shortcut - typed numbers answer the
disambiguation list only. While browsing the list, no model calls fire; Enter
drives everything (instant `@` forms still preview live).

## Markdown in answers

The Rust core's markdown segmenter (`core/ai/src/markdown.rs`, tested) splits
fenced code blocks from prose; code renders monospaced in a darker card, prose
gets inline markdown (bold/italic/`code`/links) via Apple's
`AttributedString(markdown:)`. Deliberately not a full markdown engine; no
third-party dependency.

Parsed ONCE, when the answer settles: the parse is an FFI call plus a full-text
scan, so running it per streamed token was quadratic work on the main thread.
Mid-stream the answer renders as plain text, which also avoids styling
half-written code fences.

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
- **300ms idle before planning**; an in-flight call is genuinely killed on the
   next keystroke (a cancellable plan session, not a blocking call whose
   cancellation is only noticed after it returns), so superseded requests never
   queue ahead of the one the user is waiting for.
- `keep_alive: 30m` holds the model resident; `num_predict` caps runaway output.

Measured: plan ~0.9-1.0s warm, non-action decline ~0.4s, `@` form 0ms.

## All-day and undated (never invent a time)

- Day but no clock time ("sarah's birthday march 5") -> all-day event that day.
- No date at all -> all-day today (events) / undated (reminders).
- Clock-time detection is lexical (`has_clock_time` in `core/ai/src/resolve.rs`),
  not a guess.

## Beyond the session panel

The same ladder now backs the **main search bar**: a dead-end Enter escalates to
the AI surface, and an action-shaped phrase renders as the first selected result
row (one Enter runs it, ⌘Z undoes) instead of forcing the `>` prefix on people
who never learned it. File recall shows its results in the main panel, labeled
when the model interpreted the phrasing or the query had to be relaxed.

Still ahead:

- Per-verb models (small fast planner model, larger chat model) if wanted.
- Semantic recall over clipboard history (the vision doc's Recall pillar).
- linows parity: the transport and router were built poll-based for exactly
  this, but the Tauri shell has no AI yet.
