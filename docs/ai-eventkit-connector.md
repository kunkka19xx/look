# EventKit Connector: build-level spec

> **Status: core connector SHIPPED.** Add (`calendar.add_event`, `reminder.add`,
> all-day), read (schedule answers with grammar-parsed windows), and mutate
> (`calendar.cancel_event`, `calendar.move_event`, `reminder.complete`) with the
> ambiguity gate: `TitleMatcher.resolve` plans only a confident winner, near-ties
> render a numbered `.needsChoice` list, no match is an honest error. Undo is
> faithful (cancel recreates from a snapshot, move restores times, complete
> uncompletes). "it"/"that" resolves via the last receipt's `subjectID`, skipping
> matching. As-built deltas from this spec: the planner emits tool alias +
> title/match (+ verbatim NEW time for move); other dates are extracted in code.
> Also SHIPPED: `reminder.snooze` and `calendar.block_time` (free-slot
> search over working hours, resolved in `core/ai::resolve`). Still open:
> recurring-event spans (cancel/move one occurrence vs the series), Windows/Linux.

Scope: macOS. EventKit is an Apple system framework, so this connector lives
entirely on the Swift side (`apps/macos`), not in the Rust core. Windows/Linux
are out of scope here (see `ai-vision.md` for the platform matrix).

Parent: `ai-vision.md` (Connectors section). This doc is the implementation spec
for the Calendar + Reminders primitives and the plan/preview/confirm/undo spine
around them.

Nothing here is committed until frameworks and the date-parsing approach are
confirmed (see Open decisions). No third-party dependency is proposed; EventKit,
NSDataDetector, and Foundation are all system frameworks.

## 1. Placement and flow

Matches existing conventions: commands live under `Views/Commands/`, the confirm
UI mirrors `KillConfirmationBar`, native side-effects go through a service type
(as `KillCommand` routes through `ProcessService`), and structured model output
uses `@Generable` for Apple Intelligence with a JSON-schema equivalent for
Ollama/cloud.

Flow for one query:

```
query text
  -> intent router: is this a calendar/reminder act?          (classify)
  -> CalendarActionPlanner: model returns a CalendarActionPlan (structured)
  -> CalendarActionResolver: parse dates, resolve matches      (validate)
       -> ambiguous or unparseable? show candidates / error, never guess
  -> preview: CalendarConfirmationBar shows the literal plan   (confirm)
  -> CalendarActionExecutor: EventKit writes + ActionReceipt   (execute)
  -> undo available from the receipt                           (reverse)
```

Read-only ops (`find_free_slot`, "what's next") skip confirm and render as normal
rows. Every write goes through preview + confirm.

## 2. New files

```
Support/Calendar/EventKitService.swift          # EKEventStore wrapper, permission, CRUD, protocol seam
Support/Calendar/CalendarActionResolver.swift   # plan -> resolved action; date parse; match resolve
Support/Calendar/CalendarActionExecutor.swift   # execute resolved action; build ActionReceipt; undo
Support/AI/Actions/CalendarActionPlan.swift      # model-facing types + JSON schema + @Generable
Support/AI/Actions/CalendarActionPlanner.swift   # provider call -> CalendarActionPlan
Views/Commands/CalendarCommand.swift             # routing + preview state (struct, like KillCommand)
Views/Commands/CalendarConfirmationBar.swift     # confirm UI (mirrors KillConfirmationBar)
```

Info.plist additions:

```
NSCalendarsFullAccessUsageDescription   = "look creates and edits events in your calendar when you ask it to."
NSRemindersFullAccessUsageDescription   = "look creates and completes reminders when you ask it to."
NSCalendarsUsageDescription             = (legacy, pre-macOS 14)
NSRemindersUsageDescription             = (legacy, pre-macOS 14)
```

## 3. Permission and availability

One `EKEventStore` instance, created lazily and reused (creation is expensive;
same reasoning as the Apple Intelligence warmer keeping one session).

Availability mirrors `AIProviderAvailability`:

```swift
enum CalendarAccess: Equatable {
    case authorized          // full access
    case writeOnly           // macOS 14+ can grant add-only; enough for add_*, not for read/move/cancel
    case notDetermined       // never asked
    case denied              // user said no; deep-link to System Settings
    case restricted          // MDM/parental
}
```

Request path, gated by OS:

- macOS 14+: `store.requestFullAccessToEvents()` and
  `store.requestFullAccessToReminders()` (async).
- pre-14: `store.requestAccess(to: .event)` / `.reminder`.

Status source: `EKEventStore.authorizationStatus(for: .event)` /
`.reminder`. Map the new 14+ cases (`.fullAccess`, `.writeOnly`) and legacy
`.authorized`.

Rules:

- Never trigger a permission prompt on keystroke. Request only when the user
  confirms the first write, or from a Settings "Connect Calendar" button.
- `.denied` surfaces a one-line "Grant calendar access in System Settings" with
  a deep link, never a silent failure.
- `find_free_slot`, `move_event`, `cancel_event`, and any read need full access;
  `.writeOnly` supports only `add_event` / `add_reminder`.

## 4. The plan the model emits

`CalendarActionPlan` is a discriminated union over `op`. The model returns 1..n
steps; look validates and previews all of them, executes as one confirmable
plan.

> **As built:** the shape below was the design sketch. What ships is smaller -
> see `ai-action-contracts.md` for the authoritative wire format. The
> differences: tool ids are 1-token aliases (`event`, `move`, `block`, ...) not
> `op` verbs; params are a flat `[String: String]` limited to
> `title`/`match`/`when`/`duration`/`terms`/`types`/`location`/`instruction`;
> durations are phrases ("2 hours") rather than `duration_minutes`; and every
> step of a plan executes, confirmed and undone as a unit (see
> `ai-action-contracts.md` §5). `find_free_slot` never shipped as a separate op -
> `block` does find-then-block in one step. The rest of this section documents
> the intent, which held.

Design choice (see Section 5): the model returns time **phrases as written**, not
ISO datetimes. Date math is where weak models fail, so `NSDataDetector` resolves
phrases against `now` in code. This keeps the model's job to slot extraction,
which mid-size local models handle reliably.

`match` is a free-text description the resolver turns into a concrete event/
reminder id (Section 6). The model never sees or invents ids.

Provider integration: add one method to the planning path, kept generic so other
connectors reuse it later.

```swift
protocol CalendarActionPlanning {
    func planCalendarAction(query: String, now: Date, tz: TimeZone) async -> CalendarActionPlan?
}
```

Ollama (7-8B+) and cloud (BYO key) implement it with a JSON-schema forced
response. Apple Intelligence is **not** a planning provider (its ~3B on-device
model is too weak for reliable multi-step planning and disambiguation); it stays
on the understand/answer paths only. Returns nil when the query is not a calendar
action, so routing stays best-effort and never blocks search.

Model-capability requirement: the planner's job is deliberately small (classify,
pick op, extract slots), so a mid-size local model suffices. The "act" verb is
unavailable until a capable provider (Ollama or cloud key) is configured, with a
hint to connect one. No capable model, no agentic actions. This keeps the
local-first promise intact: Apple-Intelligence-only users still get find/answer,
just not act.

## 5. Date and time parsing

The model returns time **phrases verbatim** (`start_phrase`, `due_phrase`, ...),
never ISO datetimes. Date math is where weak models fail, so resolution happens
in deterministic code, not the model. This is the main lever that makes the
connector robust regardless of model quality.

Resolution: `NSDataDetector(types: .date)` (a Foundation system API) resolves
"tuesday 10am", "next friday", "in 2 hours" against `now`. No third-party date
library. If a phrase is unparseable, show an error, never a guessed time.

Every resolution uses a single injected `now` and `TimeZone.current`, so it is
deterministic and unit-testable (do not read the clock inside the resolver; pass
`now` in, matching how the codebase already injects time where testable).

## 6. Resolving `match` to a concrete item (the ambiguity gate)

For `move_event`, `cancel_event`, `complete_reminder`, `snooze_reminder`:

1. Fetch candidates in a bounded window:
   - events: `store.predicateForEvents(withStart:end:calendars:)` over a default
     window (today .. +14 days, widened if `match` names a further date), then
     `store.events(matching:)`.
   - reminders: `store.fetchReminders(matching: store.predicateForIncompleteReminders(...))`.
2. Rank by fuzzy title match (reuse `EngineBridge.fuzzyScore`, exactly as
   `KillCommand` ranks processes) blended with time proximity when `match`
   implies a time ("3pm", "tomorrow").
3. Resolve:
   - exactly one strong match -> that item's `eventIdentifier` /
     `calendarItemIdentifier`.
   - multiple plausible -> show a disambiguation list (reuse the
     `KillCommandView` row pattern); user picks, then confirm.
   - none -> error row ("No event matching '3pm sync' in the next 2 weeks").
4. Never mutate on an ambiguous or empty match. This is the single most important
   safety rule in the connector.

## 7. Primitives (per-op contract)

Each op: model params -> resolver output -> EventKit call -> preview -> undo.

### add_event
- Params: `title, start_phrase, duration_minutes, calendar?`.
- Resolve: `start` from `start_phrase` via NSDataDetector; `end = start + duration`; validate (`end > start`); pick calendar (`store.defaultCalendarForNewEvents` if null; else match calendar by title, error if not found).
- Execute: `let e = EKEvent(eventStore: store); e.title/startDate/endDate/calendar; try store.save(e, span: .thisEvent, commit: true)`. Capture `e.eventIdentifier`.
- Preview: `Add "Dentist"  Tue Jul 28, 10:00-11:00  (Home)`.
- Undo: remove the created event by id.

### move_event
- Params: `match, new_start_phrase` (duration preserved).
- Resolve: match -> event id (Section 6); `new_start` from phrase; compute `new_end = new_start + (old_end - old_start)`.
- Execute: mutate `startDate`/`endDate`, `try store.save(_:span:.thisEvent, commit:true)`.
- Preview: `Move "Sync"  3:00pm -> 4:00pm  Tue Jul 28`.
- Undo: restore captured old start/end (snapshot before mutation).

### cancel_event
- Params: `match`.
- Resolve: match -> event id.
- Execute: snapshot the event first (title, dates, calendar, notes), then
  `try store.remove(event, span: .thisEvent, commit: true)`.
- Preview: `Cancel "Standup"  Wed Jul 29, 9:30am`.
- Undo: recreate from snapshot (new id; acceptable, content identical).

### block_time
- Params: `title, duration_minutes, window`.
- Resolve: run `find_free_slot` internally over `window`; if a slot is found,
  becomes an `add_event(title, slotStart, slotStart+duration)`. If none, error
  row ("No free 2h slot Fri").
- Preview / undo: same as `add_event`.

### find_free_slot (read-only, no confirm)
- Params: `duration_minutes, window`.
- Compute: fetch events in `window`, subtract busy intervals from working hours
  (working-hours default configurable later; start with 9:00-18:00 local), return
  first gap >= duration.
- Output: a normal result row ("Free: Fri 2:00-4:00pm"), Enter can chain into
  block_time. No mutation, no permission prompt beyond read access.

### add_reminder
- Params: `title, due?`.
- Execute: `EKReminder(eventStore: store)`, `title`, `calendar = store.defaultCalendarForNewReminders()`, optional `dueDateComponents` from `due`; `try store.save(_:commit:true)`.
- Preview: `Add reminder "Call plumber"  due Wed 9:00am`.
- Undo: remove by id.

### complete_reminder
- Params: `match`.
- Resolve: match -> reminder id (incomplete reminders only).
- Execute: `reminder.isCompleted = true` (sets completionDate), save.
- Preview: `Complete reminder "Call plumber"`.
- Undo: `isCompleted = false`, save.

### snooze_reminder
- Params: `match, new_due`.
- Resolve: match -> reminder id.
- Execute: replace `dueDateComponents` with `new_due`, save (snapshot old due).
- Preview: `Snooze "Call plumber"  -> Thu 9:00am`.
- Undo: restore old `dueDateComponents`.

## 8. Undo

`ActionReceipt` captures, per executed step, the inverse operation and any
pre-mutation snapshot needed to reproduce it faithfully.

```swift
struct ActionReceipt {
    let steps: [StepReceipt]      // one per executed step, in order
}
enum StepReceipt {
    case created(id: String)                        // undo: remove(id)
    case moved(id: String, oldStart: Date, oldEnd: Date)
    case removed(snapshot: EventSnapshot)           // undo: recreate(snapshot)
    case reminderCompleted(id: String)
    case reminderRescheduled(id: String, oldDue: DateComponents?)
}
```

- Keep the last receipt in memory; undo reverses steps in reverse order.
- Optionally persist the last N receipts (session-scoped is enough for v1).
- Surface undo as a `:undo` command and/or a post-execution row ("Undone" on
  Enter). Destructive undo (recreate) is faithful because `cancel_event`
  snapshots before removing.

## 9. Confirm UI

`CalendarConfirmationBar` mirrors `KillConfirmationBar`: same Y/N capsule
buttons, Enter/Esc keys, theme-driven styling. For a multi-step plan it lists
each step's preview line; a single Y confirms the whole plan (all steps execute
in order, or none if the first fails validation).

Disambiguation (Section 6) reuses the `KillCommandView` list rows: numbered
candidates, arrow keys + Enter, `Cmd+1..9`.

## 10. Routing and grammar

- Classification lives in the intent router (extends `AIQueryRouter`). A query
  routes to the calendar planner when it looks like a calendar/reminder act
  (verbs: add/schedule/block/move/reschedule/cancel/remind, plus a date/time or a
  reminder noun). Cheap keyword gate first (like `AIAnswerController.isQuestionLike`),
  then the model confirms by returning a plan (or nil).
- Read queries ("what's my next meeting", "am I free at 3") route to a read path
  that renders rows, no confirm.
- No new prefix required for v1, but `cal ` / `remind ` explicit prefixes are a
  cheap opt-in escape hatch and align with existing prefix grammar.

## 11. Safety rules (non-negotiable)

1. No permission prompt on keystroke; only on confirmed write or from Settings.
2. Never mutate on ambiguous or unparseable input; show candidates or an error.
3. Every write is previewed with literal values and confirmed before execution.
4. Every write produces an undo receipt.
5. Look makes no network calls. Any account sync is the OS's job (see
   `ai-vision.md`).
6. Multi-step plans are all-or-nothing at the validation stage: if any step fails
   to resolve, nothing executes and the failing step is shown.

## 12. Test plan

EventKit cannot run unattended in CI (needs a granted permission + a live store),
so put a protocol seam in front of it and test against a fake.

```swift
protocol EventStoring {                 // EventKitService conforms; FakeStore for tests
    func save(event: PlannedEvent) throws -> String
    func remove(id: String) throws
    func events(in: DateInterval) throws -> [StoredEvent]
    // reminders equivalents
}
```

Unit tests (no EventKit, deterministic `now`):

- `CalendarActionResolver`: ISO parsing, `end > start` rejection, relative-date
  resolution via injected `now`, calendar-name resolution, `end` derivation in
  `move_event`.
- match resolution: single/multiple/none against a `FakeStore` fixture; assert it
  refuses to resolve on ambiguity.
- `find_free_slot`: gap computation against fixed busy intervals and working
  hours.
- `CalendarActionExecutor` undo: each `StepReceipt` inverse restores the fake
  store to its prior state (round-trip property test).
- `CalendarActionPlan` decoding: JSON schema round-trips; unknown `op` rejected.

Plan decoding and resolver logic are the high-value tests; keep EventKit itself
behind the seam and smoke-test it manually.

## 13. Open decisions (confirm before building)

1. Frameworks: EventKit (`EKEventStore`, `EKEvent`, `EKReminder`) and
   NSDataDetector for dates. Both Apple system frameworks, no third-party
   dependency. Confirm the app's min deployment target so the pre-14 vs 14+
   permission branch is written correctly.
2. Planning provider: the connector requires a capable model (Ollama 7-8B+ or a
   cloud key). Apple Intelligence is not sufficient and is excluded from
   planning. Confirm which provider is the default and that "act" is gated off
   until one is configured.
3. `.writeOnly` support: ship add-only under write-only access, or require full
   access for everything. Recommend supporting write-only for `add_*`.
4. Working-hours default for `find_free_slot` (proposed 9:00-18:00 local), and
   whether it is configurable in v1 or later.
5. Undo surface: `:undo` command vs post-action row vs both.
6. Reminders scope in v1: full (add/complete/snooze) or add-only first.
