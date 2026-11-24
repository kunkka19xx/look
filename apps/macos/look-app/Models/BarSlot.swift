import Foundation

/// Resolved by core (`look_engine::bar_slot`), which owns the names and the
/// `running_apps_placement` fallback.
enum BarSlot: String, CaseIterable, Codable, Identifiable {
    case running
    case pinned
    case todo
    case script
    case none

    static let configKey = "bar_slot"
    static let pinnedAppsKey = "pinned_apps"
    static let pinnableIDPrefix = "app:"

    var id: String { rawValue }

    var title: String {
        switch self {
        case .running: return "Running Apps"
        case .pinned: return "Pinned Apps"
        case .todo: return "Today's Task"
        case .script: return "Script"
        case .none: return "Nothing"
        }
    }

    var takesDigitKeys: Bool {
        self == .running || self == .pinned
    }
}

nonisolated struct BarConfig: Decodable, Equatable {
    let slot: String
    let pinned: [String]
    let script: String
}

/// No title or path when the index no longer has the app.
nonisolated struct BarPin: Decodable, Equatable, Identifiable {
    let id: String
    let title: String?
    let path: String?
}

nonisolated struct BarScriptOutcome: Decodable, Equatable {
    let line: String?
    let error: String?
}

enum PinChange: Equatable {
    case pinned
    case unpinned
    case full
}

enum BarPins {
    static func toggled(_ ids: [String], _ id: String, limit: Int) -> ([String], PinChange) {
        var ids = ids
        if let at = ids.firstIndex(of: id) {
            ids.remove(at: at)
            return (ids, .unpinned)
        }
        guard ids.count < limit else { return (ids, .full) }
        ids.append(id)
        return (ids, .pinned)
    }
}

struct BarTodoLine: Equatable {
    static let emptyDay = "Plan today"
    static let allDone = "done"

    let task: String?
    let note: String

    init(done: Int, total: Int, open: [String], pick: Int) {
        let count = "\(done)/\(total)"
        if total == 0 {
            task = nil
            note = Self.emptyDay
        } else if open.isEmpty {
            task = nil
            note = "\(count) \(Self.allDone)"
        } else {
            task = open[pick % open.count]
            note = count
        }
    }
}
