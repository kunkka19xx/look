import Foundation

/// The levels a user has descended into from a block row.
///
/// One value type rather than another handful of `@State` booleans: a level
/// stack IS navigation state, and the mode flags beside it grew into a pile by
/// being added one at a time.
struct SourceLevelStack {
    private(set) var frames: [SourceLevelFrame] = []
    /// Bumped by every request and every change, so a target still running
    /// after the launcher is hidden, or after another starts, answers stale.
    private(set) var epoch: UInt64 = 0

    var isActive: Bool { !frames.isEmpty }
    var current: SourceLevelFrame? { frames.last }

    /// "look › Changed files", not "look": the row you came from does not say
    /// which of its targets you picked.
    var breadcrumb: [String] {
        guard let current else { return [] }
        return frames.map(\.parentTitle) + [current.blockName]
    }

    /// Ancestors of the rows IN this level, nearest first, for `{parent.*}`.
    var ancestorsOfCurrentRows: [SourceLevelParent] {
        frames.reversed().map {
            SourceLevelParent(id: $0.parentRowID, title: $0.parentTitle, path: $0.parentPath)
        }
    }

    /// The epoch a request must still hold when it answers.
    mutating func beginRequest() -> UInt64 {
        epoch &+= 1
        return epoch
    }

    mutating func push(_ frame: SourceLevelFrame) {
        frames.append(frame)
        epoch &+= 1
    }

    /// Hands back what the launcher was showing before it, so Escape restores
    /// rather than re-searches.
    mutating func pop() -> SourceLevelFrame? {
        epoch &+= 1
        return frames.popLast()
    }

    mutating func clear() {
        // Bumped even with no frames: a first-level request is in flight then.
        epoch &+= 1
        frames.removeAll()
    }
}

/// The block that produced a level, the row it opened from, and what Escape
/// puts back.
struct SourceLevelFrame {
    let blockName: String
    /// The row's OWN id, what `{parent.id}` expands to.
    let parentRowID: String
    let parentTitle: String
    let parentPath: String
    let rows: [SourceLevelRow]
    /// The query and selection the level was opened from, restored on Escape.
    let restoredQuery: String
    let restoredSelectionID: String?
}

/// An ancestor as the core reads it. `nonisolated` like the bridge's other
/// payloads: encoding one is pure work any context may do.
nonisolated struct SourceLevelParent: Encodable {
    let id: String
    let title: String
    let path: String
}

nonisolated extension Array where Element == SourceLevelParent {
    /// An empty array rather than an empty string: the core parses it as JSON.
    var ancestorsJSON: String {
        guard let data = try? JSONEncoder().encode(self),
            let json = String(data: data, encoding: .utf8)
        else { return "[]" }
        return json
    }
}
