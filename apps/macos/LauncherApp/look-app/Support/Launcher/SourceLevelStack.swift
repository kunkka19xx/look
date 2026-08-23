import Foundation

/// The drill-down stack: the levels a user has descended into from a block row
/// (`specs/user-sources.md` §2.10).
///
/// One value type rather than another handful of `@State` booleans on
/// `LauncherView`. A level stack IS navigation state, and the mode flags beside
/// it (`isCommandMode`, `isAIMode`, `isActionMenuOpen`) grew into a pile exactly
/// by being added one at a time.
struct SourceLevelStack {
    private(set) var frames: [SourceLevelFrame] = []

    var isActive: Bool { !frames.isEmpty }
    var current: SourceLevelFrame? { frames.last }
    var depth: Int { frames.count }

    /// What the query bar shows: the rows walked through, then WHAT is being
    /// listed. "look › Changed files", not "look", because the row you came
    /// from does not say which of its targets you picked.
    var breadcrumb: [String] {
        guard let current else { return [] }
        return frames.map(\.parentTitle) + [current.blockName]
    }

    /// The ancestors of the rows IN the current level, nearest first, as the
    /// core expects them for `{parent.*}`.
    var ancestorsOfCurrentRows: [SourceLevelParent] {
        frames.reversed().map {
            SourceLevelParent(id: $0.parentRowID, title: $0.parentTitle, path: $0.parentPath)
        }
    }

    /// The ancestors of the row a level was OPENED from, which is one step out
    /// of the above: descending again passes these plus that row.
    var ancestorsOfCurrentParent: [SourceLevelParent] {
        Array(ancestorsOfCurrentRows.dropFirst())
    }

    mutating func push(_ frame: SourceLevelFrame) {
        frames.append(frame)
    }

    /// Pops one level and hands back what the launcher was showing before it,
    /// so Escape restores rather than re-searches.
    mutating func pop() -> SourceLevelFrame? {
        frames.popLast()
    }

    mutating func clear() {
        frames.removeAll()
    }
}

/// One level: the block that produced it, the row it was opened from, and what
/// to put back on Escape.
struct SourceLevelFrame {
    let blockID: String
    let blockName: String
    /// The row's OWN id, which is what `{parent.id}` expands to, not its
    /// namespaced candidate id.
    let parentRowID: String
    let parentCandidateID: String
    let parentTitle: String
    let parentPath: String
    let rows: [SourceLevelRow]
    let truncated: Bool
    /// The query and selection the level was opened from, restored on Escape.
    let restoredQuery: String
    let restoredSelectionID: String?
}

/// An ancestor as the core reads it (`ParentRow`). `nonisolated`, like the
/// bridge's own payloads: encoding one is pure work that any context may do.
nonisolated struct SourceLevelParent: Encodable {
    let id: String
    let title: String
    let path: String
}

nonisolated extension Array where Element == SourceLevelParent {
    /// The payload every source call takes. An empty array rather than an empty
    /// string, so the core never has to guess what "no ancestors" looked like.
    var ancestorsJSON: String {
        guard let data = try? JSONEncoder().encode(self),
            let json = String(data: data, encoding: .utf8)
        else { return "[]" }
        return json
    }
}
