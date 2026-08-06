import Foundation

/// Creates a calendar event. Talks only to `EventStoring`, so it lives in the
/// package and is tested against a fake.
nonisolated struct CalendarAddEventTool: ActionTool {
    let id = "calendar.add_event"
    let title = "Add event"
    let store: EventStoring
    var defaultDurationMinutes = 60

    var paramsSchema: AIValue {
        .schema(
            properties: [
                "title": .schemaType("string"),
                "when": .schemaType("string"),
                "duration_minutes": .schemaType("number"),
            ],
            required: ["title", "when"])
    }

    func plan(_ params: [String: AIValue], now: Date) -> PlanResult {
        guard let title = params["title"]?.stringValue, !title.isEmpty else {
            return .invalid("Need a title for the event.")
        }
        guard let whenPhrase = params["when"]?.stringValue, !whenPhrase.isEmpty else {
            return .invalid("Need a time for the event.")
        }
        guard let start = DatePhrase.resolve(whenPhrase, now: now) else {
            return .invalid("Could not understand the time \"\(whenPhrase)\".")
        }
        let minutes = Int(params["duration_minutes"]?.numberValue ?? Double(defaultDurationMinutes))
        guard minutes > 0 else { return .invalid("Duration must be positive.") }
        let end = start.addingTimeInterval(TimeInterval(minutes * 60))

        let preview = ActionPreview(
            title: "Add event",
            detail: "\"\(title)\"  \(DatePhrase.format(start: start, end: end))")
        let store = self.store
        return .planned(PlannedAction(toolID: id, preview: preview, perform: {
            let eventID = try store.addEvent(title: title, start: start, end: end)
            return ActionReceipt(summary: "Added \"\(title)\"", undo: {
                try store.removeEvent(id: eventID)
            })
        }))
    }
}
