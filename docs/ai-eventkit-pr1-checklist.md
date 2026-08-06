# EventKit PR 1: add-only slice, registry-based - checklist

> **Status: SHIPPED**, with deltas discovered during implementation. This doc is
> kept as the plan of record; the as-built truth is `ai-session.md` and
> `ai-action-contracts.md`. Deltas:
> - Act lives in the `>` AI session screen (owns the panel area), not bars over
>   results. Enter confirms, Esc leaves; the session survives hide/recall.
> - The confirm UI is Enter/Esc (no Y/N); Cmd+Z undoes via a session item row.
> - The keyword gate was removed; `>` itself is the gate.
> - The repair retry was removed; the planner is single-shot with a title-only
>   wire schema (dates extracted in code) for latency.
> - Added beyond plan: all-day events, chat turns, incremental JSONL archive,
>   markdown answers.

Goal: the first shippable slice of the Act pillar. Create calendar events and
reminders from natural language, previewed and confirmed, with undo. Built on a
tool registry so later connectors cost almost nothing. No move/cancel yet, so no
ambiguity gate in this PR.

Parent spec: `ai-eventkit-connector.md`. Supersedes the earlier command-mode
sketch of this checklist.

Branch: cut off `macos/ai-ollama-provider` (Step B's planner uses the Ollama
provider from PR 0). Step A alone does not need it.

## Architecture decision: tool registry, not command mode

Act does not live in command mode. Command mode is a palette (pick a named
command, then act), which fights the natural-language premise and grows a
hardcoded switch + UI branch per capability. That does not scale to many
connectors.

Instead:

- **One surface**: the main query box. The intent router classifies find /
  answer / act / recall.
- **A tool registry**: each capability registers a tool (`calendar.add_event`,
  `reminder.add`, ...) with an id, a params schema, and a `plan()` that resolves
  and validates. Adding a connector means registering tools, not editing a
  switch or the UI.
- **One generic confirm surface**: `PendingActionBar` renders any tool's preview
  and takes Y/N. Not per-command UI.
- **Two producers of a tool call**: an explicit `>` prefix (deterministic, no
  model) and the model planner (Step B). Both feed the same spine.

Command mode stays as the explicit, no-AI fallback for the existing commands
(`kill`, `calc`, ...). It is not where act lives.

## Core types (the registry spine)

Canonical definitions: `ai-action-contracts.md`. Params are JSON-shaped
`AIValue`; the wire format carries `steps` from day one. Foundation-only, added
to the `LauncherLogic` package `sources` so they are unit-tested (same approach
as `OllamaCodec`). The sketch below is the shape; the contract doc is the source
of truth.

```swift
struct ToolCall { let toolID: String; let params: [String: String] }

struct PlannedAction {                 // what the confirm bar renders + runs
    let toolID: String
    let previewText: String            // "Add \"Dentist\"  Tue Aug 5, 10:00-11:00"
    let perform: () throws -> ActionReceipt
}

struct ActionReceipt {                 // what undo needs
    let summary: String                // "Added \"Dentist\""
    let undo: () throws -> Void
}

protocol ActionTool {
    var id: String { get }
    var title: String { get }
    var paramsSchema: [String: Any] { get }        // fed to the planner in Step B
    func plan(params: [String: String], now: Date) -> PlannedAction?
}

protocol EventStoring {                 // seam over EventKit; FakeStore in tests
    func addEvent(title: String, start: Date, end: Date) throws -> String
    func removeEvent(id: String) throws
    func addReminder(title: String, due: Date?) throws -> String
    func removeReminder(id: String) throws
}
```

`plan()` captures its own typed data inside the `perform`/`undo` closures, so the
registry only ever handles `PlannedAction` and never needs generics or type
erasure. Because the tools talk to `EventStoring` (not EventKit directly), the
tools live in the package and are fully testable with a `FakeStore`.

`ActionRegistry` (package): `register(_:)`, `tool(id:)`, `all`, and
`plan(_ call: ToolCall, now: Date) -> PlannedAction?` (looks up the tool, calls
`plan`).

## Step A: spine + 2 tools + `>` trigger (deterministic, no model)

### A1. Core types + registry
- [ ] `Support/Actions/ActionTypes.swift` (types above), `ActionRegistry.swift`,
      `EventStoring.swift`. Add all to the package `sources`.
- [ ] Tests: registry register/lookup; `plan(call:)` routes to the right tool and
      returns nil for an unknown tool id.

### A2. EventKit service (app target)
- [ ] `Support/Calendar/EventKitService.swift`: one reused `EKEventStore`,
      conforms to `EventStoring`, imports EventKit. App target only (not the
      package).
- [ ] Permission: `requestFullAccessToEvents()` / `requestFullAccessToReminders()`
      (macOS 14+; target is 15 so no pre-14 branch). Map
      `authorizationStatus(for:)` to a `CalendarAccess` enum.
- [ ] Permission strings as build settings (Info.plist is generated,
      `GENERATE_INFOPLIST_FILE = YES`): add
      `INFOPLIST_KEY_NSCalendarsFullAccessUsageDescription` and
      `INFOPLIST_KEY_NSRemindersFullAccessUsageDescription` (same mechanism as
      the existing `INFOPLIST_KEY_NSBluetoothAlwaysUsageDescription`).
- [ ] No sandbox entitlement needed: the app is not sandboxed (the entitlements
      file has only `com.apple.security.automation.apple-events`).
- [ ] Never prompt except on a confirmed write or the Settings button.

### A3. The two tools (package)
- [ ] `Support/Actions/CalendarAddEventTool.swift` (id `calendar.add_event`,
      params `title`, `when`, optional `duration_minutes`). `plan()` resolves
      `when` via `NSDataDetector` against `now`, derives `end`, validates
      `end > start`, builds `previewText`, and closes over
      `store.addEvent`/`removeEvent` for perform/undo. Injected `EventStoring`.
- [ ] `Support/Actions/ReminderAddTool.swift` (id `reminder.add`, params `title`,
      optional `when`). Same shape over `addReminder`/`removeReminder`.
- [ ] Tests (FakeStore, fixed `now`): valid input -> correct `previewText`;
      perform -> store has the item; `receipt.undo` -> store empty again;
      unparseable `when` -> `plan` returns nil; `end <= start` rejected.

### A4. Action controller (app)
- [ ] `Support/Actions/ActionController.swift`, `@MainActor ObservableObject`:
      `@Published pending: PlannedAction?`, `lastReceipt`, `feedback`.
      `propose(_ call: ToolCall)` -> `registry.plan` -> `pending`. `confirm()`
      runs `perform`, stores receipt, clears pending. `cancel()`. `undoLast()`.

### A5. Confirm UI (app)
- [ ] `Views/Commands/PendingActionBar.swift`, mirrors `KillConfirmationBar`
      (Y/N capsules, Enter/Esc, theme styling). Shows `pending.previewText`.
- [ ] Mount at the launcher top level (near the AI answer card), shown whenever
      `actionController.pending != nil`, independent of how it was triggered.

### A6. Explicit `>` trigger (main box, deterministic)
- [ ] Recognize a leading `>` in the main search path (intercept like the `t"`
      translation prefix does), suppress normal search.
- [ ] Pure parser in the package: `>add <title> <when>` ->
      `ToolCall("calendar.add_event", ...)`, `>remind <title> <when>` ->
      `ToolCall("reminder.add", ...)`. Tested.
- [ ] Hand the `ToolCall` to `actionController.propose`.

### A7. Settings
- [ ] "Connect Calendar & Reminders" button showing current `CalendarAccess` and
      triggering the permission request.

### A8. Undo surface
- [ ] Post-action "Added. Undo?" row and a `:undo` command, both call
      `actionController.undoLast()`.

Step A definition of done: typing `>add lunch tomorrow 12pm` previews, confirms,
creates a real event visible in Calendar.app, and undo removes it.

## Step B: planner as the second producer

The spine is unchanged; the model becomes a second way to make a `ToolCall`.

- [ ] `Support/AI/Actions/ActionPlanner.swift`: builds a JSON schema from the
      registry (tool ids + each tool's `paramsSchema`) and asks the Ollama
      provider (via the `OllamaCodec.chatRequestBody` `format` pattern) to return
      `{ toolID, params }`. Returns nil when not an action, or when no capable
      provider is configured (Apple Intelligence is not a planner).
- [ ] Intent classification: cheap keyword gate (add/schedule/remind + a
      date/time or reminder noun), then the planner confirms. Mirrors
      `AIAnswerController.isQuestionLike`.
- [ ] Route the planned `ToolCall` into `actionController.propose`. Same resolve
      -> preview -> confirm -> execute -> undo.
- [ ] Gate act off unless a capable provider is configured, with a one-line hint.
- [ ] Tests: schema built from a fake registry; keyword gate positive/negative.

Step B definition of done: `add dentist tuesday 10am` (no `>`) produces a
previewed event via the model, confirmed and undoable.

## Adding the next connector (the payoff)

Contacts / files / photos later: implement `ActionTool`s, register them, done.
No new confirm UI, no new switch case, and the planner picks them up
automatically because it reads the registry. This is why the registry is worth
the small upfront cost.

## Confirm before coding

- [ ] `>` is the explicit-act prefix in the main box (deterministic, no model).
- [ ] PR 1 scope = `calendar.add_event` + `reminder.add` only.
- [ ] Support macOS 14+ write-only access for add-only. Recommended yes.
- [ ] Undo surface: post-action row + `:undo`.

## Out of scope (later EventKit PRs)

- move / cancel / complete / snooze and the ambiguity gate.
- find_free_slot / block_time.
- Windows/Linux.
