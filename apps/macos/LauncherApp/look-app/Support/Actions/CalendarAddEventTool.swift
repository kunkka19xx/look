import Foundation

/// Creates a calendar event. Talks only to `EventStoring`, so it lives in the
/// package and is tested against a fake.
nonisolated struct CalendarAddEventTool: ActionTool {
    let id = "calendar.add_event"
    let title = "Add event"
    let store: EventStoring
    var defaultDurationMinutes = 60

    var planningDescription: String {
        "calendar.add_event: add a calendar event. params: title, when "
            + "(time phrase; a day with no clock time becomes all-day; omit if no date)."
    }

    var paramsSchema: AIValue {
        .schema(
            properties: [
                "title": .schemaType("string"),
                "when": .schemaType("string"),
                "duration_minutes": .schemaType("number"),
            ],
            required: ["title"])
    }

    func plan(_ params: [String: AIValue], now: Date) -> PlanResult {
        guard let title = params["title"]?.stringValue, !title.isEmpty else {
            return .invalid("Need a title for the event.")
        }
        let whenPhrase = (params["when"]?.stringValue ?? "")
            .trimmingCharacters(in: .whitespacesAndNewlines)

        // No date at all -> all-day today. A day with no clock time -> all-day on
        // that day. A day with a clock time -> a timed event. Never invents a time.
        if whenPhrase.isEmpty {
            return allDayEvent(title: title, day: now)
        }
        guard let resolved = DatePhrase.resolve(whenPhrase, now: now) else {
            return .invalid("Could not understand the time \"\(whenPhrase)\".")
        }
        if !DatePhrase.hasClockTime(whenPhrase) {
            return allDayEvent(title: title, day: resolved)
        }

        let minutes = Int(params["duration_minutes"]?.numberValue ?? Double(defaultDurationMinutes))
        guard minutes > 0 else { return .invalid("Duration must be positive.") }
        let end = resolved.addingTimeInterval(TimeInterval(minutes * 60))
        let preview = ActionPreview(
            title: "Add event",
            detail: "\"\(title)\"  \(DatePhrase.format(start: resolved, end: end))")
        let store = self.store
        return .planned(PlannedAction(toolID: id, preview: preview, perform: {
            let eventID = try store.addEvent(title: title, start: resolved, end: end, isAllDay: false)
            return ActionReceipt(summary: "Added \"\(title)\"", undo: {
                try store.removeEvent(id: eventID)
            })
        }))
    }

    private func allDayEvent(title: String, day: Date) -> PlanResult {
        let start = Calendar.current.startOfDay(for: day)
        let end = Calendar.current.date(byAdding: .day, value: 1, to: start) ?? start
        let preview = ActionPreview(
            title: "Add event",
            detail: "\"\(title)\"  \(DatePhrase.formatDay(start)) (all day)")
        let store = self.store
        return .planned(PlannedAction(toolID: id, preview: preview, perform: {
            let eventID = try store.addEvent(title: title, start: start, end: end, isAllDay: true)
            return ActionReceipt(summary: "Added \"\(title)\"", undo: {
                try store.removeEvent(id: eventID)
            })
        }))
    }
}
