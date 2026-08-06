import Foundation

/// Creates a reminder, optionally with a due time. Talks only to `EventStoring`.
nonisolated struct ReminderAddTool: ActionTool {
    let id = "reminder.add"
    let title = "Add reminder"
    let store: EventStoring

    var planningDescription: String {
        "reminder.add: create a reminder. params: title, when (optional time phrase)."
    }

    var paramsSchema: AIValue {
        .schema(
            properties: [
                "title": .schemaType("string"),
                "when": .schemaType("string"),
            ],
            required: ["title"])
    }

    func plan(_ params: [String: AIValue], now: Date) -> PlanResult {
        guard let title = params["title"]?.stringValue, !title.isEmpty else {
            return .invalid("Need something to be reminded of.")
        }
        var due: Date?
        if let phrase = params["when"]?.stringValue, !phrase.isEmpty {
            guard let resolved = DatePhrase.resolve(phrase, now: now) else {
                return .invalid("Could not understand the time \"\(phrase)\".")
            }
            due = resolved
        }

        let detail = due.map { "\"\(title)\"  due \(DatePhrase.format($0))" } ?? "\"\(title)\""
        let preview = ActionPreview(title: "Add reminder", detail: detail)
        let store = self.store
        let capturedDue = due
        return .planned(PlannedAction(toolID: id, preview: preview, perform: {
            let reminderID = try store.addReminder(title: title, due: capturedDue)
            return ActionReceipt(summary: "Added reminder \"\(title)\"", subjectID: reminderID, undo: {
                try store.removeReminder(id: reminderID)
            })
        }))
    }
}
