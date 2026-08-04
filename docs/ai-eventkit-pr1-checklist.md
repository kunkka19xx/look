# EventKit PR 1: add-only slice - checklist

Goal: the first shippable slice of the Act pillar. Create calendar events and
reminders from natural language, previewed and confirmed, with undo. No
move/cancel yet, so no ambiguity gate (the hardest part) in this PR.

Parent spec: `ai-eventkit-connector.md` (full connector). This checklist is the
add-only first slice, decomposed spine-first.

Branch: cut off `macos/ai-ollama-provider` (Step B's planner uses the Ollama
provider from PR 0). Step A alone does not need it.

Placement: macOS only, Swift side. EventKit is an Apple system framework.
Deployment target is macOS 15, so the 14+ permission APIs are always available
(no pre-14 branch needed).

## Step A: the spine (deterministic, no model)

Prove EventKit writes, permission, date parsing, and undo with a plain typed
command `cal add <title> <when>` before the model is involved.

### A1. EventKitService
New `Support/Calendar/EventKitService.swift`:

- [ ] One lazily-created, reused `EKEventStore` (like the AI warmer keeps one
      session).
- [ ] `EventStoring` protocol seam so tests use a fake, no live store:
      `func addEvent(title:start:end:) throws -> String`,
      `func removeEvent(id:) throws`,
      `func addReminder(title:due:) throws -> String`,
      `func removeReminder(id:) throws`.
- [ ] Permission: `requestFullAccessToEvents()` / `requestFullAccessToReminders()`
      (macOS 14+, always available at target 15). Map
      `authorizationStatus(for:)` to a `CalendarAccess` enum
      (authorized / writeOnly / notDetermined / denied / restricted), mirroring
      `AIProviderAvailability`.
- [ ] Info.plist: `NSCalendarsFullAccessUsageDescription`,
      `NSRemindersFullAccessUsageDescription`.
- [ ] Never trigger the permission prompt except on a confirmed write or the
      Settings button.

### A2. Date resolution
New `Support/Calendar/CalendarActionResolver.swift` (add-only subset):

- [ ] `resolve(startPhrase:durationMinutes:now:) -> DateInterval?` using
      `NSDataDetector(types: .date)` against an injected `now`. Reject
      unparseable phrases (return nil, surfaced as an error, never a guess).
- [ ] `end = start + duration`; validate `end > start`.
- [ ] Deterministic: `now` injected, never read from the clock inside.
- [ ] Put this logic in a Foundation-only file added to the `LauncherLogic`
      package `sources` so it is unit-tested (same approach as `OllamaCodec`).

### A3. Executor + undo
New `Support/Calendar/CalendarActionExecutor.swift`:

- [ ] Execute a resolved add via `EventStoring`, capture the returned id into an
      `ActionReceipt` (`.created(id:)` / `.reminderCreated(id:)`).
- [ ] `undo(_ receipt:)` removes the created item(s).
- [ ] Keep the last receipt in memory; expose to the command for the undo row.

### A4. Command + confirm UI
New `Views/Commands/CalendarCommand.swift` and
`Views/Commands/CalendarConfirmationBar.swift`:

- [ ] `CalendarCommand` parses `cal add <title> <when>` / `remind <title> <when>`
      (Step A entry point), resolves via A2, holds preview state.
- [ ] `CalendarConfirmationBar` mirrors `KillConfirmationBar` (Y/N, Enter/Esc,
      theme styling). Shows the literal preview: `Add "Dentist"  Tue Aug 5,
      10:00-11:00`.
- [ ] After execute, show an "Added. Undo?" row; `:undo` also reverses.
- [ ] Route the typed prefixes through the launcher the same way other commands
      are dispatched.

### A5. Settings
- [ ] "Connect Calendar & Reminders" button in the AI/Advanced settings that
      triggers the permission request and shows current `CalendarAccess`.

### A6. Tests (Step A)
- [ ] Resolver: phrase parsing with fixed `now`, `end > start` rejection,
      unparseable-phrase returns nil.
- [ ] Executor undo: `.created` / `.reminderCreated` inverses restore a
      `FakeStore` to its prior state (round-trip).
- [ ] Command parsing: `cal add`/`remind` split into title + phrase.

Step A definition of done: typing `cal add lunch tomorrow 12pm` previews,
confirms, creates a real event visible in Calendar.app, and `:undo` removes it.

## Step B: the brain (model planner)

Feed the same spine from natural language via the Ollama provider.

### B1. Plan types + schema
New `Support/AI/Actions/CalendarActionPlan.swift`:

- [ ] `CalendarActionPlan` with add-only steps: `add_event(title, start_phrase,
      duration_minutes, calendar?)` and `add_reminder(title, due_phrase)`.
      Codable, Foundation-only, added to the package for decode tests.
- [ ] JSON schema builder (like `OllamaCodec.intentJSONSchema`) for the `format`
      field.

### B2. Planner
New `Support/AI/Actions/CalendarActionPlanner.swift`:

- [ ] `planCalendarAction(query:now:tz:) async -> CalendarActionPlan?` using the
      Ollama provider's chat endpoint with `format` = B1 schema (reuse the
      `OllamaCodec.chatRequestBody` pattern). Apple Intelligence is not a
      planner; if the active provider is not capable, return nil.
- [ ] Returns nil when the query is not a calendar action (best-effort, never
      blocks search).

### B3. Routing
- [ ] Intent classification: a cheap keyword gate (add/schedule/remind + a
      date/time or reminder noun), then the planner confirms by returning a plan
      or nil. Mirrors `AIAnswerController.isQuestionLike`.
- [ ] A matched plan flows into the Step A spine: resolve -> preview -> confirm
      -> execute -> undo. Same UI, same safety.
- [ ] "Act" is unavailable unless a capable provider (Ollama) is configured, with
      a one-line hint. No capable model, no actions.

### B4. Tests (Step B)
- [ ] `CalendarActionPlan` JSON decode round-trips; unknown `op` rejected.
- [ ] Keyword gate: positive/negative classification samples.

Step B definition of done: typing `add dentist tuesday 10am` (no `cal` prefix)
produces a previewed event via the model, confirmed and undoable.

## Out of scope (later EventKit PRs)

- move / cancel / complete / snooze and the ambiguity gate (Section 6 of the
  spec).
- find_free_slot / block_time.
- Reminders beyond add (complete/snooze).
- Windows/Linux.

## Confirm before coding

- [ ] PR 1 scope = add_event + add_reminder only. Confirmed.
- [ ] Support macOS 14+ write-only access for add-only. Recommended yes.
- [ ] Undo surface: post-action row + `:undo`. Recommended.
- [ ] Permission requested on first confirmed write and via a Settings button.
