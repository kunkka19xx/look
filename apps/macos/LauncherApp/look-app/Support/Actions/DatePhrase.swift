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

    /// Day-only preview for all-day events, e.g. "Fri Aug 7".
    static func formatDay(_ date: Date) -> String {
        let f = DateFormatter()
        f.dateFormat = "EEE MMM d"
        return f.string(from: date)
    }

    /// Whether a phrase names a clock time (vs just a day). Decides all-day vs
    /// timed: "march 5" / "friday" -> false (all-day); "3pm" / "15:00" / "noon"
    /// -> true (timed). A lexical check, not a semantic guess.
    static func hasClockTime(_ phrase: String) -> Bool {
        let p = phrase.lowercased()
        if p.contains("noon") || p.contains("midnight") || p.contains("o'clock") {
            return true
        }
        let patterns = [
            #"\d{1,2}\s?(am|pm)"#,   // 3pm, 10 am
            #"\d{1,2}:\d{2}"#,        // 15:00, 3:30
        ]
        return patterns.contains {
            p.range(of: $0, options: .regularExpression) != nil
        }
    }
}
