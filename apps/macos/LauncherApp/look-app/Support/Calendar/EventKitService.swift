import EventKit
import Foundation

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
