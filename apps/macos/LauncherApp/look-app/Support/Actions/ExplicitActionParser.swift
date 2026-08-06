import Foundation

/// The instant, no-model action path. Handles ONLY the explicit delimited form
/// where the user separates title and time with `@`:
///   >add <title> @ <when>       ->  calendar.add_event
///   >remind <title> @ <when>    ->  reminder.add
/// Anything without `@` is natural language and returns nil, so the model
/// normalizes it into a clean, finalized output (no brittle heuristics here).
nonisolated enum ExplicitActionParser {
    static func parse(_ input: String) -> ToolCall? {
        let trimmed = input.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.hasPrefix(">") else { return nil }
        let body = trimmed.dropFirst().trimmingCharacters(in: .whitespacesAndNewlines)
        guard let spaceIdx = body.firstIndex(where: { $0.isWhitespace }) else { return nil }

        let verb = body[..<spaceIdx].lowercased()
        let rest = String(body[body.index(after: spaceIdx)...])
            .trimmingCharacters(in: .whitespacesAndNewlines)
        guard !rest.isEmpty else { return nil }

        // Deterministic parsing ONLY for the explicit `title @ when` form, where
        // the user delimits the spec themselves - no guessing. Natural language
        // (no `@`) returns nil so the model normalizes it into a clean output.
        guard let sep = rest.range(of: " @ ") else { return nil }
        let title = String(rest[..<sep.lowerBound]).trimmingCharacters(in: .whitespacesAndNewlines)
        let whenText = String(rest[sep.upperBound...]).trimmingCharacters(in: .whitespacesAndNewlines)
        guard !title.isEmpty else { return nil }
        let when = whenText.isEmpty ? nil : whenText

        switch verb {
        case "add", "event", "cal":
            return ToolCall(
                toolID: "calendar.add_event",
                params: ["title": .string(title), "when": .string(when ?? "")])
        case "remind", "reminder":
            var params: [String: AIValue] = ["title": .string(title)]
            if let when { params["when"] = .string(when) }
            return ToolCall(toolID: "reminder.add", params: params)
        default:
            return nil
        }
    }
}
