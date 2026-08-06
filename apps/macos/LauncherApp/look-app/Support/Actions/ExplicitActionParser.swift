import Foundation

/// The explicit, no-model action path: turns a `>` line typed in the main box
/// into a `ToolCall`. Grammar:
///   >add <title> @ <when>        /  >add <title> <when>
///   >remind <title> [@ <when>]   /  >remind <title> <when>
/// The `@` separator is the reliable way to split title from time; without it,
/// a trailing date is detected best-effort. Returns nil for non-action input.
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

        let (title, when) = splitTitleWhen(rest)
        guard !title.isEmpty else { return nil }

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

    static func splitTitleWhen(_ text: String) -> (title: String, when: String?) {
        if let sep = text.range(of: " @ ") {
            let title = String(text[..<sep.lowerBound])
                .trimmingCharacters(in: .whitespacesAndNewlines)
            let when = String(text[sep.upperBound...])
                .trimmingCharacters(in: .whitespacesAndNewlines)
            return (title, when.isEmpty ? nil : when)
        }
        if let split = DatePhrase.splitTrailingDate(text) {
            return (split.title, split.when)
        }
        return (text, nil)
    }
}
