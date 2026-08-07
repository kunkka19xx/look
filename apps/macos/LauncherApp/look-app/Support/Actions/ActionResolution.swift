import Foundation

/// The shell half of the P4 contract (core/ai/src/resolve.rs): decode the
/// Rust-core resolution outcome and execute/undo it against EventKit. Rust
/// resolves (validation, matching gate, dates, previews); this side only
/// performs the resulting operations.

enum ActionExecuteSpec: Decodable {
    case addEvent(title: String, start: Date, end: Date, allDay: Bool)
    case addReminder(title: String, due: Date?)
    case removeEvent(id: String)
    case moveEvent(id: String, start: Date, end: Date)
    case completeReminder(id: String)
    case removeReminder(id: String)

    private enum Keys: String, CodingKey {
        case kind, title, start, end, due, id
        case allDay = "all_day"
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: Keys.self)
        let kind = try container.decode(String.self, forKey: .kind)
        func date(_ key: Keys) throws -> Date {
            Date(timeIntervalSince1970: TimeInterval(try container.decode(Int64.self, forKey: key)))
        }
        switch kind {
        case "add_event":
            self = .addEvent(
                title: try container.decode(String.self, forKey: .title),
                start: try date(.start), end: try date(.end),
                allDay: try container.decodeIfPresent(Bool.self, forKey: .allDay) ?? false)
        case "add_reminder":
            let due = try container.decodeIfPresent(Int64.self, forKey: .due)
            self = .addReminder(
                title: try container.decode(String.self, forKey: .title),
                due: due.map { Date(timeIntervalSince1970: TimeInterval($0)) })
        case "remove_event":
            self = .removeEvent(id: try container.decode(String.self, forKey: .id))
        case "move_event":
            self = .moveEvent(
                id: try container.decode(String.self, forKey: .id),
                start: try date(.start), end: try date(.end))
        case "complete_reminder":
            self = .completeReminder(id: try container.decode(String.self, forKey: .id))
        case "remove_reminder":
            self = .removeReminder(id: try container.decode(String.self, forKey: .id))
        default:
            throw DecodingError.dataCorruptedError(
                forKey: .kind, in: container, debugDescription: "unknown execute kind \(kind)")
        }
    }
}

enum ActionUndoSpec: Decodable {
    case removeEventByNewId
    case removeReminderByNewId
    case recreateEvent(title: String, start: Date, end: Date, allDay: Bool)
    case moveEvent(id: String, start: Date, end: Date)
    case uncompleteReminder(id: String)
    case recreateReminder(title: String, due: Date?)

    private enum Keys: String, CodingKey {
        case kind, title, start, end, due, id
        case allDay = "all_day"
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: Keys.self)
        let kind = try container.decode(String.self, forKey: .kind)
        func date(_ key: Keys) throws -> Date {
            Date(timeIntervalSince1970: TimeInterval(try container.decode(Int64.self, forKey: key)))
        }
        switch kind {
        case "remove_event_by_new_id":
            self = .removeEventByNewId
        case "remove_reminder_by_new_id":
            self = .removeReminderByNewId
        case "recreate_event":
            self = .recreateEvent(
                title: try container.decode(String.self, forKey: .title),
                start: try date(.start), end: try date(.end),
                allDay: try container.decodeIfPresent(Bool.self, forKey: .allDay) ?? false)
        case "move_event":
            self = .moveEvent(
                id: try container.decode(String.self, forKey: .id),
                start: try date(.start), end: try date(.end))
        case "uncomplete_reminder":
            self = .uncompleteReminder(id: try container.decode(String.self, forKey: .id))
        case "recreate_reminder":
            let due = try container.decodeIfPresent(Int64.self, forKey: .due)
            self = .recreateReminder(
                title: try container.decode(String.self, forKey: .title),
                due: due.map { Date(timeIntervalSince1970: TimeInterval($0)) })
        default:
            throw DecodingError.dataCorruptedError(
                forKey: .kind, in: container, debugDescription: "unknown undo kind \(kind)")
        }
    }
}

struct ActionResolvedPlan: Decodable {
    let previewTitle: String
    let previewDetail: String
    let summary: String
    let subject: String?  // "new" = the id the executor gets back from an add
    let execute: ActionExecuteSpec
    let undo: ActionUndoSpec

    private enum Keys: String, CodingKey {
        case previewTitle = "preview_title"
        case previewDetail = "preview_detail"
        case summary, subject, execute, undo
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: Keys.self)
        previewTitle = try container.decode(String.self, forKey: .previewTitle)
        previewDetail = try container.decode(String.self, forKey: .previewDetail)
        summary = try container.decode(String.self, forKey: .summary)
        subject = try container.decodeIfPresent(String.self, forKey: .subject)
        execute = try container.decode(ActionExecuteSpec.self, forKey: .execute)
        undo = try container.decode(ActionUndoSpec.self, forKey: .undo)
    }
}

enum ActionResolveOutcome {
    case planned(ActionResolvedPlan)
    case choice([ActionCandidate])
    case invalid(String)

    static func decode(_ data: Data) -> ActionResolveOutcome {
        struct Envelope: Decodable {
            let outcome: String
            let message: String?
            let candidates: [RawCandidate]?
            struct RawCandidate: Decodable {
                let id: String
                let label: String
            }
        }
        guard let envelope = try? JSONDecoder().decode(Envelope.self, from: data) else {
            return .invalid("Could not read the resolution.")
        }
        switch envelope.outcome {
        case "planned":
            guard let plan = try? JSONDecoder().decode(ActionResolvedPlan.self, from: data) else {
                return .invalid("Could not read the plan.")
            }
            return .planned(plan)
        case "choice":
            let candidates = (envelope.candidates ?? []).map {
                ActionCandidate(id: $0.id, label: $0.label)
            }
            return .choice(candidates)
        default:
            return .invalid(envelope.message ?? "Couldn't do that.")
        }
    }
}

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
        }
    }
}
