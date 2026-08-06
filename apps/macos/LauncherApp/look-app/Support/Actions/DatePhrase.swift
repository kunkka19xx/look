import Foundation

/// Resolves a natural time phrase ("tomorrow 10am", "next friday", "aug 5 3pm")
/// to a concrete `Date` using `NSDataDetector`, a Foundation system API. No
/// third-party date library. Relative phrases resolve against the system clock;
/// deterministic tests use absolute phrases.
nonisolated enum DatePhrase {
    /// Expands common shorthand day words NSDataDetector does not know ("tmr" ->
    /// "tomorrow"), so a time-plus-day phrase resolves to the right day instead of
    /// falling back to a time-only match on today.
    static func normalizeShorthand(_ text: String) -> String {
        let map: [String: String] = [
            "tmr": "tomorrow", "tmrw": "tomorrow", "tmw": "tomorrow",
            "tmoro": "tomorrow", "tomoro": "tomorrow", "2moro": "tomorrow",
            "tdy": "today", "tonite": "tonight",
        ]
        return text
            .split(separator: " ", omittingEmptySubsequences: false)
            .map { map[$0.lowercased()] ?? String($0) }
            .joined(separator: " ")
    }

    static func resolve(_ phrase: String, now: Date) -> Date? {
        let trimmed = normalizeShorthand(phrase.trimmingCharacters(in: .whitespacesAndNewlines))
        guard !trimmed.isEmpty else { return nil }
        guard let detector = try? NSDataDetector(
            types: NSTextCheckingResult.CheckingType.date.rawValue) else { return nil }
        let range = NSRange(trimmed.startIndex..<trimmed.endIndex, in: trimmed)
        return detector.firstMatch(in: trimmed, options: [], range: range)?.date
    }

    /// Human-readable preview of an interval, e.g. "Tue Aug 5, 10:00-11:00".
    static func format(start: Date, end: Date) -> String {
        let day = DateFormatter()
        day.dateFormat = "EEE MMM d"
        let time = DateFormatter()
        time.dateFormat = "HH:mm"
        return "\(day.string(from: start)), \(time.string(from: start))-\(time.string(from: end))"
    }

    /// Human-readable preview of a single instant, e.g. "Tue Aug 5, 09:00".
    static func format(_ date: Date) -> String {
        let f = DateFormatter()
        f.dateFormat = "EEE MMM d, HH:mm"
        return f.string(from: date)
    }

    /// Best-effort split of "lunch tomorrow 12pm" into ("lunch", "tomorrow 12pm").
    /// Finds the first date substring; text before it (minus a trailing "at"/"on")
    /// is the title. Returns nil when no date is present or nothing is left as a
    /// title.
    static func splitTrailingDate(_ input: String) -> (title: String, when: String)? {
        let text = normalizeShorthand(input)
        guard let detector = try? NSDataDetector(
            types: NSTextCheckingResult.CheckingType.date.rawValue) else { return nil }
        let range = NSRange(text.startIndex..<text.endIndex, in: text)
        guard let match = detector.firstMatch(in: text, options: [], range: range),
              let matchRange = Range(match.range, in: text) else { return nil }
        var title = String(text[..<matchRange.lowerBound])
            .trimmingCharacters(in: .whitespacesAndNewlines)
        for connective in [" at", " on", " @"] where title.lowercased().hasSuffix(connective) {
            title = String(title.dropLast(connective.count))
                .trimmingCharacters(in: .whitespacesAndNewlines)
        }
        let when = String(text[matchRange]).trimmingCharacters(in: .whitespacesAndNewlines)
        guard !title.isEmpty, !when.isEmpty else { return nil }
        return (title, when)
    }
}
