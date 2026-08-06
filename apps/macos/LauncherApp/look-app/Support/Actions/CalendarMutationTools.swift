import Foundation

/// The mutate tools: cancel/move an existing event, complete a reminder. All
/// three share the ambiguity gate (`TitleMatcher.resolve`): a confident match
/// plans, near-ties become `.needsChoice`, no match is an honest `.invalid`.
/// A `chosen_id` param (set after the user picks from a choice list, or by
/// pronoun resolution for "it") bypasses matching entirely.

nonisolated struct CalendarCancelEventTool: ActionTool {
    let id = "calendar.cancel_event"
    let title = "Cancel event"
    let store: EventStoring
    var searchDays = 30

    var planningDescription: String {
        "calendar.cancel_event: remove an existing event. params: match "
            + "(words identifying the event)."
    }

    var paramsSchema: AIValue {
        .schema(properties: ["match": .schemaType("string")], required: ["match"])
    }

    func plan(_ params: [String: AIValue], now: Date) -> PlanResult {
        let candidates = store.eventCandidates(
            from: now, to: now.addingTimeInterval(Double(searchDays) * 86_400))
        switch resolveEvent(params, candidates: candidates, searchDays: searchDays) {
        case .planned(let event):
            let store = self.store
            let preview = ActionPreview(
                title: "Cancel event", detail: eventDetail(event))
            return .planned(PlannedAction(toolID: id, preview: preview, perform: {
                try store.removeEvent(id: event.id)
                // Undo recreates from the snapshot taken before deletion.
                return ActionReceipt(summary: "Cancelled \"\(event.title)\"", undo: {
                    _ = try store.addEvent(
                        title: event.title, start: event.start, end: event.end,
                        isAllDay: event.isAllDay)
                })
            }))
        case .other(let result):
            return result
        }
    }
}

nonisolated struct CalendarMoveEventTool: ActionTool {
    let id = "calendar.move_event"
    let title = "Move event"
    let store: EventStoring
    var searchDays = 30

    var planningDescription: String {
        "calendar.move_event: reschedule an existing event. params: match "
            + "(words identifying the event), when (the NEW time phrase verbatim)."
    }

    var paramsSchema: AIValue {
        .schema(
            properties: ["match": .schemaType("string"), "when": .schemaType("string")],
            required: ["match", "when"])
    }

    func plan(_ params: [String: AIValue], now: Date) -> PlanResult {
        guard let whenPhrase = params["when"]?.stringValue, !whenPhrase.isEmpty else {
            return .invalid("When should it move to?")
        }
        guard let resolved = DatePhrase.resolve(whenPhrase, now: now) else {
            return .invalid("Could not understand the time \"\(whenPhrase)\".")
        }
        let candidates = store.eventCandidates(
            from: now, to: now.addingTimeInterval(Double(searchDays) * 86_400))
        switch resolveEvent(params, candidates: candidates, searchDays: searchDays) {
        case .planned(let event):
            // A day-only phrase ("friday") keeps the event's clock time.
            let newStart = DatePhrase.hasClockTime(whenPhrase)
                ? resolved
                : preservingClock(from: event.start, onDayOf: resolved)
            let newEnd = newStart.addingTimeInterval(
                event.end.timeIntervalSince(event.start))
            let store = self.store
            let preview = ActionPreview(
                title: "Move event",
                detail: "\"\(event.title)\"  \(DatePhrase.format(event.start)) -> "
                    + DatePhrase.format(start: newStart, end: newEnd))
            return .planned(PlannedAction(toolID: id, preview: preview, perform: {
                try store.moveEvent(id: event.id, start: newStart, end: newEnd)
                return ActionReceipt(
                    summary: "Moved \"\(event.title)\" to \(DatePhrase.format(newStart))",
                    subjectID: event.id,
                    undo: { try store.moveEvent(id: event.id, start: event.start, end: event.end) })
            }))
        case .other(let result):
            return result
        }
    }

    private func preservingClock(from old: Date, onDayOf day: Date) -> Date {
        let cal = Calendar.current
        var comps = cal.dateComponents([.year, .month, .day], from: day)
        let time = cal.dateComponents([.hour, .minute], from: old)
        comps.hour = time.hour
        comps.minute = time.minute
        return cal.date(from: comps) ?? day
    }
}

nonisolated struct ReminderCompleteTool: ActionTool {
    let id = "reminder.complete"
    let title = "Complete reminder"
    let store: EventStoring

    var planningDescription: String {
        "reminder.complete: mark an existing reminder done. params: match "
            + "(words identifying the reminder)."
    }

    var paramsSchema: AIValue {
        .schema(properties: ["match": .schemaType("string")], required: ["match"])
    }

    func plan(_ params: [String: AIValue], now: Date) -> PlanResult {
        let candidates = store.reminderCandidates()

        let picked: ReminderCandidateData?
        if let chosen = params["chosen_id"]?.stringValue {
            guard let match = candidates.first(where: { $0.id == chosen }) else {
                return .invalid("That reminder is gone.")
            }
            picked = match
        } else {
            guard let match = params["match"]?.stringValue, !match.isEmpty else {
                return .invalid("Which reminder?")
            }
            switch TitleMatcher.resolve(candidates, query: match, title: \.title) {
            case .none:
                return .invalid("No open reminder matching \"\(match)\".")
            case .several(let options):
                return .needsChoice(options.map {
                    ActionCandidate(id: $0.id, label: $0.title)
                })
            case .one(let winner):
                picked = winner
            }
        }
        guard let reminder = picked else { return .invalid("Which reminder?") }

        let store = self.store
        let preview = ActionPreview(
            title: "Complete reminder", detail: "\"\(reminder.title)\"")
        return .planned(PlannedAction(toolID: id, preview: preview, perform: {
            try store.completeReminder(id: reminder.id)
            return ActionReceipt(
                summary: "Completed \"\(reminder.title)\"",
                subjectID: reminder.id,
                undo: { try store.uncompleteReminder(id: reminder.id) })
        }))
    }
}

/// Shared event resolution: `chosen_id` bypass, then the match gate.
nonisolated private enum EventResolution {
    case planned(EventCandidateData)
    case other(PlanResult)
}

nonisolated private func resolveEvent(
    _ params: [String: AIValue],
    candidates: [EventCandidateData],
    searchDays: Int
) -> EventResolution {
    if let chosen = params["chosen_id"]?.stringValue {
        guard let event = candidates.first(where: { $0.id == chosen }) else {
            return .other(.invalid("That event is gone."))
        }
        return .planned(event)
    }
    guard let match = params["match"]?.stringValue, !match.isEmpty else {
        return .other(.invalid("Which event?"))
    }
    switch TitleMatcher.resolve(candidates, query: match, title: \.title) {
    case .none:
        return .other(.invalid(
            "No event matching \"\(match)\" in the next \(searchDays) days."))
    case .several(let options):
        return .other(.needsChoice(options.map {
            ActionCandidate(id: $0.id, label: "\($0.title)  ·  \(DatePhrase.format($0.start))")
        }))
    case .one(let event):
        return .planned(event)
    }
}

nonisolated private func eventDetail(_ event: EventCandidateData) -> String {
    event.isAllDay
        ? "\"\(event.title)\"  \(DatePhrase.formatDay(event.start)) (all day)"
        : "\"\(event.title)\"  \(DatePhrase.format(start: event.start, end: event.end))"
}
