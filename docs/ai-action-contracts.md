# Action contracts and models

The stable core the whole Act pillar is built on. Get these right and adding the
Nth tool is a one-file change. Everything else (EventKit, Ollama, SwiftUI)
depends on these types; these types depend on nothing but Foundation.

Placement: the currency types, the `ActionTool` protocol, the registry, and the
`EventStoring` seam are Foundation-only and live in the `LauncherLogic` package
(unit-tested, no app/UI/EventKit imports). Concrete tools that only talk to
`EventStoring` also live in the package. Only the concrete backends
(`EventKitService`), the `ActionController` (ObservableObject), and the confirm
UI are app-target.

## 1. The currency types

Tiny and stable on purpose. Producers emit these; the pipeline moves them.

```swift
// Model-agnostic value shaped like JSON. Params and JSON Schema both speak this,
// so model output maps 1:1 to what tools consume. Needs a custom Codable impl
// (single-value container that branches on the JSON type); that codec is itself
// unit-tested.
enum AIValue: Equatable {
    case string(String)
    case number(Double)
    case bool(Bool)
    case array([AIValue])
    case object([String: AIValue])
    case null
}

// What every producer emits (the `>` parser and the model planner both).
struct ToolCall: Equatable {
    let toolID: String
    let params: [String: AIValue]
}

// The model's wire format. `steps` is present from day one: Step A executes only
// length 1, but chaining later is a controller change, never a wire change.
struct ActionPlan: Codable { let steps: [PlanStep] }
struct PlanStep: Codable { let tool: String; let params: [String: AIValue] }
```

## 2. The tool contract

A tool describes itself and knows how to plan. The registry is just a map.

```swift
protocol ActionTool {
    var id: String { get }                 // "calendar.add_event"
    var title: String { get }              // "Add event"
    var paramsSchema: AIValue { get }      // JSON Schema (AIValue.object)
    func plan(_ params: [String: AIValue], now: Date) -> PlanResult
}

// Wider than Optional so the model can grow with no signature change.
enum PlanResult {
    case planned(PlannedAction)
    case invalid(String)                   // missing/bad params, unresolvable date
    case needsChoice([ActionCandidate])    // reserved for the move/cancel gate
}

struct PlannedAction {
    let toolID: String
    let preview: ActionPreview
    let perform: () throws -> ActionReceipt   // closes over the tool's typed data
}

// Structured, not a raw String, so the confirm UI can grow (icons, multi-line,
// diffs) without touching every tool.
struct ActionPreview: Equatable {
    let title: String                      // "Add event"
    let detail: String                     // "\"Dentist\"  Tue Aug 5, 10:00-11:00"
}

struct ActionReceipt {
    let summary: String                    // "Added \"Dentist\""
    let undo: () throws -> Void
}

struct ActionCandidate: Equatable {        // for a future disambiguation list
    let id: String
    let label: String
}
```

Why closures on `PlannedAction`: each tool captures its own typed resolved data
inside `perform`/`undo`, so the registry handles only `PlannedAction` and never
needs generics or type erasure. New tools never widen a shared type.

## 3. The registry

```swift
final class ActionRegistry {
    func register(_ tool: ActionTool)
    func tool(id: String) -> ActionTool?
    var all: [ActionTool] { get }

    // Look up the tool, validate + resolve via its plan(). Unknown id -> invalid.
    func plan(_ call: ToolCall, now: Date) -> PlanResult
}
```

## 4. The pipeline (stable, testable seams)

```
Producer -> ToolCall -> registry.plan -> PlanResult -> PlannedAction
         -> confirm -> perform -> ActionReceipt -> undo
```

Each arrow is an independent boundary. Swap a producer (add the cloud planner),
a backend (EventKit -> CalDAV), or the confirm UI without touching the others.

`ActionController` (app, `@MainActor ObservableObject`) owns the runtime state:

```swift
@Published var pending: PlannedAction?
@Published var lastReceipt: ActionReceipt?
@Published var feedback: String

func propose(_ call: ToolCall)   // registry.plan; .planned -> pending, else feedback
func confirm()                   // perform -> lastReceipt; clear pending
func cancel()
func undoLast()                  // lastReceipt.undo
```

As built it also owns the AI session (item stack, chat turns, incremental
archive); see `ai-session.md` for that layer.

## 5. Producers, one currency

- **Explicit `@` form** (deterministic, no model): a pure parser turns
  `>add <title> @ <when>` into a `ToolCall`. It handles ONLY this delimited
  form; anything without `@` is natural language and defers to the planner.
  Tested in the package.
- **Model planner**: asks Ollama for an `ActionPlan` (via `format` JSON Schema)
  and maps each `PlanStep` to a `ToolCall`. Latency-driven contract (see
  `ai-session.md`): the model emits only a 1-token tool alias
  ("event"/"reminder", mapped to real ids in the planner) plus a clean `title`;
  the planner injects `when` from the raw query in code (NSDataDetector's date
  value is robust). Single-shot, static cached prompt, temperature 0. There is
  no repair retry: the fields a repair round could fix no longer come from the
  model.
- Queries the planner declines become chat turns in the AI session
  (`ai-session.md`), not `ToolCall`s.

All action producers converge on `ActionController.propose`. The rest of the
system cannot tell which produced the call.

## 6. Schema is the single source of truth (and where validation lives)

Each tool's `paramsSchema` drives param validation and help/docs; the planner's
wire schema is deliberately narrower (alias enum + title-only params) for
latency, so what the model emits stays minimal.

Division of validation:

- The planner's wire format constrains `tool` to an enum of aliases, so the
  model cannot invent one, and `params` to `{ title }`. Everything else (dates,
  durations, all-day) is computed in code.
- Authoritative per-tool validation happens in `tool.plan()`: required fields,
  types, date resolvability, invariants (`end > start`). Bad input -> `.invalid`.

So the model's job stays tiny (classify + title) and the strict checks live in
deterministic, testable code.

## 7. Evolution rules

- **Add a tool:** new file + `register`, plus one alias entry and one prompt
  line in `ActionPlanner` (its wire schema is deliberately narrow for latency).
  No switch statements or UI changes, ever; a new OS surface additionally needs
  a service seam + a permission chip.
- **Add a param:** add an optional field to the schema. Old calls stay valid; the
  model fills it when relevant. Additive only, no renames.
- **Change a backend:** reimplement that tool's `plan`/`perform`. Its `id` and
  schema are unchanged, so nothing upstream notices.
- **Tolerate the unknown:** unknown tool id -> `.invalid`, never a crash. Extra
  params the model invented -> ignored. Keeps updated/weaker models from breaking
  execution.
- **Add composition:** the wire format already carries `steps`. The controller
  moves from executing `steps[0]` to looping. No contract change.
- **Swap provider/prompt:** only the planner changes; it depends solely on the
  registry schemas and returns `ToolCall`s.

## 8. What is testable without EventKit or a model

Because tools talk to `EventStoring` and produce `PlannedAction`, almost
everything is unit-tested against a `FakeStore` with a fixed `now`:

- `AIValue` JSON round-trip (encode/decode).
- Registry register/lookup and unknown-id handling.
- Each tool's `plan()`: valid -> preview + working perform/undo against
  `FakeStore`; invalid params/date -> `.invalid`.
- The `>` parser: text -> `ToolCall`.
- (Step B) `ActionPlan` decode; planner schema assembled from a fake registry.

Only the concrete `EventKitService` and the SwiftUI confirm bar need manual
smoke testing.
