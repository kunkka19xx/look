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

    /// Timeframe extraction for schedule questions. Not a phrase list: a small
    /// grammar of modifiers (this/next/last, "in N") composed with calendar
    /// units (day/week/month/year/weekend), weekday names, and month names;
    /// `Calendar.dateInterval(of:)` does the math. "next month", "this year",
    /// "in 3 weeks", "in august" all fall out of composition; adding a unit is a
    /// table entry. Returns nil when no frame is named (caller picks a default).
    static func queryWindow(for query: String, now: Date) -> (start: Date, end: Date, label: String)? {
        let cal = Calendar.current
        let words = query.lowercased()
            .split(whereSeparator: { !$0.isLetter && !$0.isNumber })
            .map(String.init)

        func singleDay(_ date: Date, _ label: String) -> (Date, Date, String)? {
            let start = cal.startOfDay(for: date)
            guard let end = cal.date(byAdding: .day, value: 1, to: start) else { return nil }
            return (start, end, label)
        }
        // "this X" starts now (no point listing the past); shifted windows keep
        // their natural bounds.
        func window(_ interval: DateInterval, shifted: Bool, _ label: String) -> (Date, Date, String) {
            (shifted ? interval.start : max(interval.start, now), interval.end, label)
        }

        let units: [String: Calendar.Component] = [
            "day": .day, "days": .day,
            "week": .weekOfYear, "weeks": .weekOfYear,
            "month": .month, "months": .month,
            "year": .year, "years": .year,
        ]
        let weekdays: [String: Int] = [
            "sunday": 1, "monday": 2, "tuesday": 3, "wednesday": 4,
            "thursday": 5, "friday": 6, "saturday": 7,
        ]
        let months: [String: Int] = [
            "january": 1, "february": 2, "march": 3, "april": 4, "may": 5,
            "june": 6, "july": 7, "august": 8, "september": 9, "october": 10,
            "november": 11, "december": 12,
        ]
        // "may" is usually a verb; only read it as the month after these.
        let monthGuards: Set<String> = ["in", "this", "next", "coming", "for", "during", "of", "early", "late"]

        for (index, word) in words.enumerated() {
            let previous = index > 0 ? words[index - 1] : ""

            if word == "today" || word == "tonight" {
                return singleDay(now, "today")
            }
            if ["tomorrow", "tmr", "tmrw", "tmw"].contains(word),
               let tomorrow = cal.date(byAdding: .day, value: 1, to: now) {
                return singleDay(tomorrow, "tomorrow")
            }

            if word == "weekend" {
                guard var saturday = cal.nextDate(
                    after: now, matching: DateComponents(weekday: 7),
                    matchingPolicy: .nextTime) else { continue }
                let currentWeekday = cal.component(.weekday, from: now)
                if currentWeekday == 7 || currentWeekday == 1, previous != "next" {
                    saturday = now  // mid-weekend: list what's left of it
                } else if previous == "next",
                          let shifted = cal.date(byAdding: .day, value: 7, to: saturday) {
                    saturday = shifted
                }
                guard let monday = cal.nextDate(
                    after: cal.startOfDay(for: saturday),
                    matching: DateComponents(weekday: 2),
                    matchingPolicy: .nextTime) else { continue }
                let label = previous == "next" ? "next weekend" : "this weekend"
                return (max(cal.startOfDay(for: saturday), min(now, saturday)), monday, label)
            }

            if let component = units[word] {
                var offset = 0
                var label = "this \(word)"
                if previous == "next" || previous == "coming" {
                    offset = 1
                    label = "next \(word)"
                } else if let n = Int(previous), n > 0 {
                    offset = n
                    label = "in \(n) \(word)"
                }
                guard
                    let base = cal.date(byAdding: component, value: offset, to: now),
                    let interval = cal.dateInterval(of: component, for: base)
                else { continue }
                return window(interval, shifted: offset != 0, label)
            }

            if let weekdayNumber = weekdays[word],
               let date = cal.nextDate(
                   after: now, matching: DateComponents(weekday: weekdayNumber),
                   matchingPolicy: .nextTime) {
                return singleDay(date, word.capitalized)
            }

            if let monthNumber = months[word] {
                if word == "may", !monthGuards.contains(previous) { continue }
                if cal.component(.month, from: now) == monthNumber,
                   let current = cal.dateInterval(of: .month, for: now) {
                    return window(current, shifted: false, word.capitalized)
                }
                if let start = cal.nextDate(
                       after: now, matching: DateComponents(month: monthNumber, day: 1),
                       matchingPolicy: .nextTime),
                   let interval = cal.dateInterval(of: .month, for: start) {
                    return window(interval, shifted: true, word.capitalized)
                }
            }
        }
        return nil
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
