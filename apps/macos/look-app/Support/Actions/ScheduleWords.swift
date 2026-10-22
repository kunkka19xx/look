import Foundation

/// Pure word gates deciding whether a question is about the user's schedule,
/// and which store it means. Foundation-only so it lives in the `LauncherLogic`
/// package and is unit-tested without EventKit: the cost of a wrong answer here
/// is either a missed calendar answer or a pointless EventKit fetch, so the
/// exact vocabulary is worth pinning in tests.
nonisolated enum ScheduleWords {
    static let reminderWords: Set<String> = [
        "reminder", "reminders", "todo", "todos", "tasks",
    ]

    static let eventWords: Set<String> = [
        "event", "events", "meeting", "meetings", "calendar", "appointment",
        "appointments",
    ]

    /// Cheap gate for "should this question see the calendar at all".
    static let scheduleWords: Set<String> = [
        "meeting", "meetings", "event", "events", "calendar", "schedule",
        "scheduled", "free", "busy", "appointment", "appointments", "reminder",
        "reminders", "today", "tomorrow", "tonight", "week", "weekend", "next",
        "monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday",
    ]

    static func mentionsReminders(_ query: String) -> Bool { hasWord(query, reminderWords) }
    static func mentionsEvents(_ query: String) -> Bool { hasWord(query, eventWords) }
    static func mentionsSchedule(_ query: String) -> Bool { hasWord(query, scheduleWords) }

    /// A reminder-specific question lists reminders, not events.
    static func prefersReminders(_ query: String) -> Bool {
        mentionsReminders(query) && !mentionsEvents(query)
    }

    private static func hasWord(_ query: String, _ set: Set<String>) -> Bool {
        query.lowercased()
            .split(whereSeparator: { !$0.isLetter })
            .contains { set.contains(String($0)) }
    }
}
