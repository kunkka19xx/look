import Foundation

/// Cheap pre-gate: does a query even look like an action, so we avoid calling the
/// model on every keystroke? First word must be an action verb and there must be
/// something after it. The model is the second, authoritative gate (it returns no
/// steps for a non-action).
nonisolated enum ActionKeywordGate {
    static let verbs: Set<String> = [
        "add", "remind", "reminder", "schedule", "block", "create", "event",
    ]

    static func looksLikeAction(_ text: String) -> Bool {
        let words = text
            .trimmingCharacters(in: .whitespacesAndNewlines)
            .lowercased()
            .split(whereSeparator: { $0 == " " || $0 == "\n" })
        guard words.count >= 2 else { return false }
        return verbs.contains(String(words[0]))
    }
}
