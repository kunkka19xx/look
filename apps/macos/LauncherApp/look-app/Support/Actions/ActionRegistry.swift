import Foundation

/// Holds the registered tools. Adding a capability is `register(_:)`; there is no
/// central switch. `plan` looks up the tool and delegates resolution to it.
nonisolated final class ActionRegistry {
    private var tools: [String: ActionTool] = [:]

    func register(_ tool: ActionTool) {
        tools[tool.id] = tool
    }

    func tool(id: String) -> ActionTool? { tools[id] }

    var all: [ActionTool] { Array(tools.values) }

    func plan(_ call: ToolCall, now: Date) -> PlanResult {
        guard let tool = tools[call.toolID] else {
            return .invalid("Unknown action: \(call.toolID)")
        }
        return tool.plan(call.params, now: now)
    }
}
