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
            // NSDataDetector reads "sunday at 5pm" but trips on "sunday @ 5pm".
            "@": "at",
        ]
        return text
            .split(separator: " ", omittingEmptySubsequences: false)
            .map { token -> String in
                let lower = token.lowercased()
                if let mapped = map[lower] { return mapped }
                if lower.hasPrefix("@"), lower.count > 1 { return "at " + token.dropFirst() }
                return String(token)
            }
            .joined(separator: " ")
    }

    /// Resolves against the SYSTEM clock: `NSDataDetector` offers no way to
    /// inject a reference date, so this deliberately takes none rather than
    /// accepting one it would silently ignore.
    static func resolve(_ phrase: String) -> Date? {
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
}
