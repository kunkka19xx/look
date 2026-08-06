import EventKit
import Foundation

nonisolated struct EventKitServiceError: Error, LocalizedError {
    let message: String
    init(_ message: String) { self.message = message }
    var errorDescription: String? { message }
}

/// Access state for calendar or reminders, mirroring `AIProviderAvailability`.
nonisolated enum CalendarAccess: Equatable {
    case authorized
    case writeOnly
    case notDetermined
    case denied
    case restricted

    var canWrite: Bool { self == .authorized || self == .writeOnly }

    var message: String {
        switch self {
        case .authorized, .writeOnly: return "Connected."
        case .notDetermined: return "Not connected yet."
        case .denied: return "Denied. Enable in System Settings > Privacy."
        case .restricted: return "Restricted by system policy."
        }
    }
}

/// Concrete `EventStoring` backed by EventKit: the same system store Calendar.app
/// and Reminders.app use. Look only touches the local store; any account sync is
/// the OS's job. One reused `EKEventStore`.
nonisolated final class EventKitService: EventStoring, @unchecked Sendable {
    static let shared = EventKitService()

    private let store = EKEventStore()

    private init() {}

    var calendarAccess: CalendarAccess {
        Self.map(EKEventStore.authorizationStatus(for: .event))
    }

    var reminderAccess: CalendarAccess {
        Self.map(EKEventStore.authorizationStatus(for: .reminder))
    }

    /// Requests full access to both. Triggers the system prompt only when status
    /// is notDetermined; callers gate on that.
    func requestAccess() async {
        _ = try? await store.requestFullAccessToEvents()
        _ = try? await store.requestFullAccessToReminders()
    }

    /// Compact listing of the next `days` of events, for injection into chat
    /// context so schedule questions are answerable. Nil without full (read)
    /// access. Local only: this text goes to the local model, nowhere else.
    func upcomingEventsSummary(days: Int = 7) -> (text: String, events: [EventCandidateData])? {
        let start = Date()
        guard let end = Calendar.current.date(byAdding: .day, value: days, to: start) else {
            return nil
        }
        return eventsSummary(from: start, to: end, emptyText: "No events in the next \(days) days.")
    }

    /// Window variant, so "next week" / "tomorrow" questions list the right
    /// days. Returns the listed events too, so the session can remember them as
    /// referent targets ("remove this event" after a listing).
    func eventsSummary(from start: Date, to end: Date, emptyText: String) -> (text: String, events: [EventCandidateData])? {
        guard calendarAccess == .authorized else { return nil }
        let predicate = store.predicateForEvents(withStart: start, end: end, calendars: nil)
        let events = store.events(matching: predicate).prefix(30)
        guard !events.isEmpty else { return (emptyText, []) }

        let dayFormat = DateFormatter()
        dayFormat.dateFormat = "EEE MMM d"
        let timeFormat = DateFormatter()
        timeFormat.dateFormat = "HH:mm"
        let text = events.map { event in
            let title = event.title ?? "Untitled"
            let day = dayFormat.string(from: event.startDate)
            if event.isAllDay {
                return "\(day) (all day): \(title)"
            }
            return "\(day) \(timeFormat.string(from: event.startDate))-\(timeFormat.string(from: event.endDate)): \(title)"
        }.joined(separator: "\n")
        let candidates = events.compactMap { event -> EventCandidateData? in
            guard let id = event.eventIdentifier else { return nil }
            return EventCandidateData(
                id: id, title: event.title ?? "Untitled",
                start: event.startDate, end: event.endDate, isAllDay: event.isAllDay)
        }
        return (text, candidates)
    }

    // MARK: EventStoring

    func addEvent(title: String, start: Date, end: Date, isAllDay: Bool) throws -> String {
        let event = EKEvent(eventStore: store)
        event.title = title
        event.startDate = start
        event.endDate = end
        event.isAllDay = isAllDay
        event.calendar = store.defaultCalendarForNewEvents
        try store.save(event, span: .thisEvent, commit: true)
        return event.eventIdentifier
    }

    func removeEvent(id: String) throws {
        guard let event = store.event(withIdentifier: id) else { return }
        try store.remove(event, span: .thisEvent, commit: true)
    }

    func addReminder(title: String, due: Date?) throws -> String {
        let reminder = EKReminder(eventStore: store)
        reminder.title = title
        reminder.calendar = store.defaultCalendarForNewReminders()
        if let due {
            reminder.dueDateComponents = Calendar.current.dateComponents(
                [.year, .month, .day, .hour, .minute], from: due)
        }
        try store.save(reminder, commit: true)
        return reminder.calendarItemIdentifier
    }

    func removeReminder(id: String) throws {
        guard let reminder = store.calendarItem(withIdentifier: id) as? EKReminder else { return }
        try store.remove(reminder, commit: true)
    }

    // MARK: Mutation seams (Phase B)

    func eventCandidates(from: Date, to: Date) -> [EventCandidateData] {
        guard calendarAccess == .authorized else { return [] }
        let predicate = store.predicateForEvents(withStart: from, end: to, calendars: nil)
        return store.events(matching: predicate).compactMap { event in
            guard let id = event.eventIdentifier else { return nil }
            return EventCandidateData(
                id: id, title: event.title ?? "Untitled",
                start: event.startDate, end: event.endDate, isAllDay: event.isAllDay)
        }
    }

    func moveEvent(id: String, start: Date, end: Date) throws {
        guard let event = store.event(withIdentifier: id) else {
            throw EventKitServiceError("That event no longer exists.")
        }
        event.startDate = start
        event.endDate = end
        try store.save(event, span: .thisEvent, commit: true)
    }

    /// Reminders can only be fetched asynchronously, but `tool.plan()` is
    /// synchronous, so tools read this cache; `refreshReminderCache()` is fired
    /// when AI input starts (mode entry / submit), keeping it fresh enough.
    private var cachedReminders: [ReminderCandidateData] = []

    func refreshReminderCache() {
        guard reminderAccess == .authorized else { return }
        let predicate = store.predicateForReminders(in: nil)
        store.fetchReminders(matching: predicate) { [weak self] reminders in
            let items = (reminders ?? [])
                .filter { !$0.isCompleted }
                .map { reminder in
                    ReminderCandidateData(
                        id: reminder.calendarItemIdentifier,
                        title: reminder.title ?? "Untitled",
                        due: reminder.dueDateComponents.flatMap { Calendar.current.date(from: $0) })
                }
            DispatchQueue.main.async { self?.cachedReminders = items }
        }
    }

    func reminderCandidates() -> [ReminderCandidateData] { cachedReminders }

    func completeReminder(id: String) throws {
        guard let reminder = store.calendarItem(withIdentifier: id) as? EKReminder else {
            throw EventKitServiceError("That reminder no longer exists.")
        }
        reminder.isCompleted = true
        try store.save(reminder, commit: true)
    }

    func uncompleteReminder(id: String) throws {
        guard let reminder = store.calendarItem(withIdentifier: id) as? EKReminder else {
            throw EventKitServiceError("That reminder no longer exists.")
        }
        reminder.isCompleted = false
        try store.save(reminder, commit: true)
    }

    private static func map(_ status: EKAuthorizationStatus) -> CalendarAccess {
        switch status {
        case .fullAccess, .authorized: return .authorized
        case .writeOnly: return .writeOnly
        case .notDetermined: return .notDetermined
        case .denied: return .denied
        case .restricted: return .restricted
        @unknown default: return .denied
        }
    }
}
