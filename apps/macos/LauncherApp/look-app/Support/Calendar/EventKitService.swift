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

/// A calendar event / reminder as passed to the Rust-core resolver and the
/// session's referent targets.
nonisolated struct EventCandidateData: Equatable {
    let id: String
    let title: String
    let start: Date
    let end: Date
    let isAllDay: Bool
}

nonisolated struct ReminderCandidateData: Equatable {
    let id: String
    let title: String
    let due: Date?
}

/// EventKit backend: the same system store Calendar.app and Reminders.app use.
/// Look only touches the local store; any account sync is the OS's job. One
/// reused `EKEventStore`. Resolution lives in the Rust core; this executes.
nonisolated final class EventKitService: @unchecked Sendable {
    static let shared = EventKitService()

    private let store = EKEventStore()

    private init() {
        // Keep the reminder cache live: EventKit changes (adds in Reminders.app,
        // completions, etc.) force a refresh so mutate tools never see a stale
        // list. Fetching reminders is async-only, hence the cache.
        // On the main queue: both refreshes read/write cache timestamps that
        // every other caller touches from main.
        NotificationCenter.default.addObserver(
            forName: .EKEventStoreChanged, object: store, queue: .main
        ) { [weak self] _ in
            self?.refreshReminderCache(force: true)
            self?.refreshEventCache(force: true)
        }
    }

    var calendarAccess: CalendarAccess {
        Self.map(EKEventStore.authorizationStatus(for: .event))
    }

    var reminderAccess: CalendarAccess {
        Self.map(EKEventStore.authorizationStatus(for: .reminder))
    }

    /// Requests full access to both - for the explicit Settings "connect"
    /// button, where asking for both IS the user's intent. An action running on
    /// one store must use the granular calls below instead: a TCC prompt is
    /// one-shot per store, so an unasked-for prompt the user denies can never
    /// be retried from the app.
    func requestAccess() async {
        await requestCalendarAccess()
        await requestReminderAccess()
    }

    func requestCalendarAccess() async {
        _ = try? await store.requestFullAccessToEvents()
    }

    func requestReminderAccess() async {
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

    // MARK: Write operations

    func addEvent(title: String, start: Date, end: Date, isAllDay: Bool) throws -> String {
        let event = EKEvent(eventStore: store)
        event.title = title
        event.startDate = start
        event.endDate = end
        event.isAllDay = isAllDay
        event.calendar = store.defaultCalendarForNewEvents
        try store.save(event, span: .thisEvent, commit: true)
        // `eventIdentifier` is implicitly unwrapped, so a nil would trap here.
        // It is also the id undo needs, so a missing one must surface as an
        // error rather than a crash or an event that cannot be taken back.
        guard let identifier = event.eventIdentifier as String? else {
            throw EventKitServiceError("The calendar did not return an id for that event.")
        }
        return identifier
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

    /// Events in a window, flattened for the meeting core: the three fields a
    /// join link hides in, plus what it takes to choose between them.
    ///
    /// `refreshSourcesIfNecessary` first, because an invite that arrived
    /// moments ago may not have synced down yet and "join my next meeting" is
    /// asked precisely when a meeting is about to start. It is a hint, not a
    /// blocking fetch, so it costs nothing when the store is already current.
    func meetingEventPayloads(from: Date, to: Date) -> [MeetingEventPayload] {
        guard calendarAccess == .authorized else { return [] }
        store.refreshSourcesIfNecessary()
        let predicate = store.predicateForEvents(withStart: from, end: to, calendars: nil)
        return store.events(matching: predicate).map { event in
            MeetingEventPayload(
                title: event.title ?? "Untitled",
                startUnixS: Int64(event.startDate.timeIntervalSince1970),
                endUnixS: Int64(event.endDate.timeIntervalSince1970),
                url: event.url?.absoluteString,
                location: event.location,
                notes: event.notes,
                allDay: event.isAllDay)
        }
    }

    /// Event cache mirroring the reminder cache, so the per-keystroke `@` mutate
    /// preview resolves without a live EventKit fetch each stroke. Warmed while
    /// composing (throttled) and forced on `.EKEventStoreChanged`. Main-thread
    /// only: `store.events` is synchronous, so no async hop is needed.
    private var cachedEvents: [EventCandidateData] = []
    private var lastEventFetch = Date.distantPast
    private let eventCacheTTL: TimeInterval = 2
    private let eventCacheDays = 31

    func refreshEventCache(force: Bool = false) {
        guard calendarAccess == .authorized else { return }
        if !force, Date().timeIntervalSince(lastEventFetch) < eventCacheTTL { return }
        lastEventFetch = Date()
        let from = Date()
        guard let to = Calendar.current.date(byAdding: .day, value: eventCacheDays, to: from) else { return }
        cachedEvents = eventCandidates(from: from, to: to)
        syncAITargets()
    }

    /// Push the cached events + reminders into the Rust core once, so the mutate
    /// resolve carries only `{tool, params}`. Called when a cache updates (mode
    /// entry, throttled refresh, or an external change), never per keystroke.
    func syncAITargets() {
        let events = cachedEvents.map { e -> [String: Any] in
            [
                "id": e.id, "title": e.title,
                "start": Int64(e.start.timeIntervalSince1970),
                "end": Int64(e.end.timeIntervalSince1970),
                "all_day": e.isAllDay,
            ]
        }
        let reminders = cachedReminders.map { r -> [String: Any] in
            var entry: [String: Any] = ["id": r.id, "title": r.title]
            if let due = r.due { entry["due"] = Int64(due.timeIntervalSince1970) }
            return entry
        }
        guard
            let evData = try? JSONSerialization.data(withJSONObject: events),
            let remData = try? JSONSerialization.data(withJSONObject: reminders),
            let evStr = String(data: evData, encoding: .utf8),
            let remStr = String(data: remData, encoding: .utf8)
        else { return }
        EngineBridge.shared.aiLoadTargets(eventsJSON: evStr, remindersJSON: remStr)
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
    private var lastReminderFetch = Date.distantPast
    private let reminderCacheTTL: TimeInterval = 2

    /// Kicks an async reminder fetch into the cache. Throttled so the live
    /// preview path can call it per-keystroke; `force` bypasses the throttle
    /// (used by the change observer, where the store genuinely changed).
    func refreshReminderCache(force: Bool = false) {
        guard reminderAccess == .authorized else { return }
        if !force, Date().timeIntervalSince(lastReminderFetch) < reminderCacheTTL { return }
        lastReminderFetch = Date()
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
            self?.applyReminderCache(items)
        }
    }

    /// Cache writes land on the main queue, where every reader (tool planning,
    /// UI) runs. Kept out of the fetch callback so `self` is not carried across
    /// isolation domains.
    private func applyReminderCache(_ items: [ReminderCandidateData]) {
        DispatchQueue.main.async {
            self.cachedReminders = items
            self.syncAITargets()
        }
    }

    func reminderCandidates() -> [ReminderCandidateData] { cachedReminders }

    /// Compact listing of open reminders for chat context / a direct answer.
    /// Nil without read access; empty text when there are none.
    func remindersSummary() -> (text: String, reminders: [ReminderCandidateData])? {
        guard reminderAccess == .authorized else { return nil }
        let reminders = cachedReminders
        guard !reminders.isEmpty else { return ("No open reminders.", []) }
        let df = DateFormatter()
        df.dateFormat = "EEE MMM d"
        let text = reminders.prefix(30).map { reminder -> String in
            if let due = reminder.due {
                return "\(reminder.title)  (due \(df.string(from: due)))"
            }
            return reminder.title
        }.joined(separator: "\n")
        return (text, Array(reminders.prefix(30)))
    }

    func completeReminder(id: String) throws {
        guard let reminder = store.calendarItem(withIdentifier: id) as? EKReminder else {
            throw EventKitServiceError("That reminder no longer exists.")
        }
        reminder.isCompleted = true
        try store.save(reminder, commit: true)
    }

    func setReminderDue(id: String, due: Date?) throws {
        guard let reminder = store.calendarItem(withIdentifier: id) as? EKReminder else {
            throw EventKitServiceError("That reminder no longer exists.")
        }
        if let due {
            reminder.dueDateComponents = Calendar.current.dateComponents(
                [.year, .month, .day, .hour, .minute], from: due)
        } else {
            reminder.dueDateComponents = nil
        }
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
