import Foundation

/// What every producer (the `>` parser, the model planner) emits. Params are
/// plain strings; typed resolution happens in the Rust core (core/ai).
nonisolated struct ToolCall: Equatable {
    let toolID: String
    var params: [String: String]
}

/// Structured preview so the confirm UI can grow without changing producers.
nonisolated struct ActionPreview: Equatable {
    let title: String
    let detail: String
}

/// A resolved, ready-to-run action. `perform` wraps the executor + the
/// Rust-provided undo recipe; the registry-free shell only ever handles this.
nonisolated struct PlannedAction {
    let toolID: String
    let preview: ActionPreview
    let perform: () throws -> ActionReceipt
}

nonisolated struct ActionReceipt {
    let summary: String
    /// Identifier of the item this action created or touched, so a follow-up
    /// "move it" / "remove it" can resolve the pronoun. Nil when gone (cancel).
    var subjectID: String? = nil
    let undo: () throws -> Void
}

nonisolated struct ActionCandidate: Equatable {
    let id: String
    let label: String
}
