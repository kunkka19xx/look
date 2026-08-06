import Foundation

/// The instant, no-model action path. Handles ONLY the explicit delimited form
/// where the user separates title and time with `@`:
///   >add <title> @ <when>       ->  calendar.add_event
///   >remind <title> @ <when>    ->  reminder.add
/// Anything without `@` is natural language and returns nil, so the model
/// normalizes it into a clean, finalized output (no brittle heuristics here).
nonisolated enum ExplicitActionParser {
    static func parse(_ input: String, modelAvailable: Bool = true) -> ToolCall? {
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
        // Accepts both `title @ 3pm` and `title @3pm`; requiring whitespace
        // before the `@` keeps email-like titles safe.
        guard let sep = rest.range(of: " @") else { return nil }
        let title = String(rest[..<sep.lowerBound]).trimmingCharacters(in: .whitespacesAndNewlines)
        let whenText = String(rest[sep.upperBound...]).trimmingCharacters(in: .whitespacesAndNewlines)
        guard !title.isEmpty else { return nil }
        // A day word in the title half ("call mom on sunday @ 5pm") means the
        // user's date intent is NOT all after the `@`; taking only the after-`@`
        // time would silently schedule the wrong day.
        var when = whenText.isEmpty ? nil : whenText
        if containsDayWord(title) {
            // With a model: defer, it produces a clean title AND the right day.
            guard !modelAvailable else { return nil }
            // Without one: keep the verbatim title but resolve the date from the
            // whole phrase, so the day is still correct. Deterministic, no
            // surgery on the title.
            when = rest
        }

        return call(verb: verb, title: title, when: when)
    }

    private static func call(verb: String, title: String, when: String?) -> ToolCall? {
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

    private static let dayWords: Set<String> = [
        "today", "tomorrow", "tonight", "tmr", "tmrw", "tmw", "2moro",
        "monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday",
        "mon", "tue", "tues", "wed", "thu", "thur", "thurs", "fri", "sat", "sun",
    ]

    static func containsDayWord(_ text: String) -> Bool {
        text.lowercased()
            .split(whereSeparator: { !$0.isLetter && !$0.isNumber })
            .contains { dayWords.contains(String($0)) }
    }
}
