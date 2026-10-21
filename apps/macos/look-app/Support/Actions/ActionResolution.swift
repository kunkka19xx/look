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
    case setReminderDue(id: String, due: Date?)

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
        func optDate(_ key: Keys) throws -> Date? {
            try container.decodeIfPresent(Int64.self, forKey: key)
                .map { Date(timeIntervalSince1970: TimeInterval($0)) }
        }
        switch kind {
        case "set_reminder_due":
            self = .setReminderDue(
                id: try container.decode(String.self, forKey: .id), due: try optDate(.due))
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
    case setReminderDue(id: String, due: Date?)

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
        func optDate(_ key: Keys) throws -> Date? {
            try container.decodeIfPresent(Int64.self, forKey: key)
                .map { Date(timeIntervalSince1970: TimeInterval($0)) }
        }
        switch kind {
        case "set_reminder_due":
            self = .setReminderDue(
                id: try container.decode(String.self, forKey: .id), due: try optDate(.due))
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
