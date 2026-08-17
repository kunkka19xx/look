import Foundation

/// The EventKit half of the P4 contract: performs and reverses the operations
/// `ActionResolution` decoded. Split out because it binds to EventKit, while
/// the decode side is pure Foundation and lives in the tested package.

/// Executes a resolved operation against the system calendar, returning the new
/// item id for adds (nil otherwise).
enum ActionExecutor {
    static func perform(_ spec: ActionExecuteSpec) throws -> String? {
        let service = EventKitService.shared
        switch spec {
        case .addEvent(let title, let start, let end, let allDay):
            return try service.addEvent(title: title, start: start, end: end, isAllDay: allDay)
        case .addReminder(let title, let due):
            return try service.addReminder(title: title, due: due)
        case .removeEvent(let id):
            try service.removeEvent(id: id)
            return nil
        case .moveEvent(let id, let start, let end):
            try service.moveEvent(id: id, start: start, end: end)
            return nil
        case .completeReminder(let id):
            try service.completeReminder(id: id)
            return nil
        case .removeReminder(let id):
            try service.removeReminder(id: id)
            return nil
        case .setReminderDue(let id, let due):
            try service.setReminderDue(id: id, due: due)
            return nil
        }
    }

    static func undoClosure(for undo: ActionUndoSpec, newID: String?) -> () throws -> Void {
        let service = EventKitService.shared
        switch undo {
        case .removeEventByNewId:
            return { if let newID { try service.removeEvent(id: newID) } }
        case .removeReminderByNewId:
            return { if let newID { try service.removeReminder(id: newID) } }
        case .recreateEvent(let title, let start, let end, let allDay):
            return { _ = try service.addEvent(title: title, start: start, end: end, isAllDay: allDay) }
        case .moveEvent(let id, let start, let end):
            return { try service.moveEvent(id: id, start: start, end: end) }
        case .uncompleteReminder(let id):
            return { try service.uncompleteReminder(id: id) }
        case .recreateReminder(let title, let due):
            return { _ = try service.addReminder(title: title, due: due) }
        case .setReminderDue(let id, let due):
            return { try service.setReminderDue(id: id, due: due) }
        }
    }
}
