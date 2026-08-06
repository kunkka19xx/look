import Foundation

/// What every producer (the `>` parser, the model planner) emits. Tiny and
/// stable: nothing downstream can tell which producer made it.
nonisolated struct ToolCall: Equatable {
    let toolID: String
    let params: [String: AIValue]
}

/// The model's wire format. `steps` is present from day one; Step A executes only
/// length 1, so chaining later is a controller change, never a wire change.
nonisolated struct ActionPlan: Codable, Equatable {
    let steps: [PlanStep]
}

nonisolated struct PlanStep: Codable, Equatable {
    let tool: String
    let params: [String: AIValue]

    var toolCall: ToolCall { ToolCall(toolID: tool, params: params) }
}

/// Result of resolving a `ToolCall`. Wider than Optional so the future
/// move/cancel ambiguity gate slots in with no signature change.
nonisolated enum PlanResult {
    case planned(PlannedAction)
    case invalid(String)
    case needsChoice([ActionCandidate])
}

/// A resolved, ready-to-run action. `perform`/`undo` close over the tool's own
/// typed data, so the registry stays generic-free.
nonisolated struct PlannedAction {
    let toolID: String
    let preview: ActionPreview
    let perform: () throws -> ActionReceipt
}

/// Structured preview so the confirm UI can grow (icons, multi-line, diffs)
/// without changing every tool.
nonisolated struct ActionPreview: Equatable {
    let title: String
    let detail: String
}

nonisolated struct ActionReceipt {
    let summary: String
    let undo: () throws -> Void
}

nonisolated struct ActionCandidate: Equatable {
    let id: String
    let label: String
}

/// A capability that describes itself and knows how to plan.
nonisolated protocol ActionTool {
    var id: String { get }
    var title: String { get }
    var paramsSchema: AIValue { get }
    /// One line for the model planner: what the tool does and its params, so the
    /// system prompt can list it. New tools document themselves here.
    var planningDescription: String { get }
    func plan(_ params: [String: AIValue], now: Date) -> PlanResult
}

/// Seam over EventKit so tools stay in the package and are tested with a fake.
nonisolated protocol EventStoring {
    func addEvent(title: String, start: Date, end: Date, isAllDay: Bool) throws -> String
    func removeEvent(id: String) throws
    func addReminder(title: String, due: Date?) throws -> String
    func removeReminder(id: String) throws
}
