import Foundation

/// Descending into a `then` target that lists rather than performs
/// (`specs/user-sources.md` §2.10).
///
/// A level takes the result list over: its rows are produced live against the
/// row it was opened from, filtered here rather than by the engine, since they
/// are not in the index and never will be.
extension LauncherView {
    var isInLevel: Bool { levelStack.isActive }

    /// The current level's rows, filtered by what is typed at this level.
    ///
    /// `.action` whatever the row carries: Enter at a level runs the block's
    /// verbs, and a path only says where the tool chords act.
    var levelResults: [LauncherResult] {
        guard let level = levelStack.current else { return [] }
        let typed = query.trimmingCharacters(in: .whitespacesAndNewlines)

        return level.rows
            .filter { LevelFilter.matches(typed, row: $0) }
            .enumerated()
            .map { position, row in
                LauncherResult(
                    id: row.candidateId,
                    kind: .action,
                    title: row.title,
                    subtitle: row.subtitle ?? row.group ?? level.blockName,
                    path: row.path ?? "",
                    // The producer's order, kept: a script list is written in an
                    // order its author chose, and nothing here knows better yet.
                    score: level.rows.count - position
                )
            }
    }

    /// Opens `blockID` as a level below the selected row.
    func descendIntoBlock(blockID: String, title: String) {
        guard let selected = actionableSelectedResult() else { return }

        // Resolved here, on the main actor, rather than inside the detached task
        // below: the stack is main-actor state and a String crosses freely.
        let ancestorsJSON = levelStack.ancestorsOfCurrentRows.ancestorsJSON
        let parent = (
            candidateID: selected.id, title: selected.title, path: selected.path
        )
        let openedFrom = (query: query, selection: selectedResultID)

        Task {
            let level = await Task.detached(priority: .userInitiated) {
                EngineBridge.shared.sourceRows(
                    blockID: blockID,
                    parentCandidateID: parent.candidateID,
                    parentTitle: parent.title,
                    parentPath: parent.path,
                    query: openedFrom.query,
                    ancestorsJSON: ancestorsJSON
                )
            }.value

            await MainActor.run {
                guard let level else {
                    showBanner("\(title): the core did not answer", style: .error, duration: 3.0)
                    return
                }
                if let error = level.error {
                    // Not descending is the point: an empty level and a broken
                    // command look identical once you are inside one.
                    showBanner("\(title): \(error)", style: .error, duration: 4.0)
                    return
                }

                levelStack.push(
                    SourceLevelFrame(
                        blockID: blockID,
                        blockName: title,
                        parentRowID: level.parentRowId,
                        parentCandidateID: parent.candidateID,
                        parentTitle: parent.title,
                        parentPath: parent.path,
                        rows: level.rows,
                        truncated: level.truncated,
                        restoredQuery: openedFrom.query,
                        restoredSelectionID: openedFrom.selection
                    )
                )
                // A level starts unfiltered: narrowing it is the first thing
                // anyone does, and the query that got here means nothing now.
                query = ""
                setInitialSelection()
                if level.truncated {
                    showBanner(
                        "\(title): showing the first \(level.rows.count)",
                        style: .info, duration: 2.0)
                }
            }
        }
    }

    /// Escape: back one level, with the query and selection it was opened from.
    /// Returns whether a level was there to leave.
    @discardableResult
    func popLevel() -> Bool {
        guard let left = levelStack.pop() else { return false }
        query = left.restoredQuery
        selectedResultID = left.restoredSelectionID
        return true
    }

    func clearLevels() {
        levelStack.clear()
    }

    /// The ancestors of the row currently selected, for the core's `{parent.*}`.
    /// Empty outside a level, which is what every non-drilled row has.
    var selectedRowAncestorsJSON: String {
        levelStack.ancestorsOfCurrentRows.ancestorsJSON
    }
}

/// Matching inside a level. Deliberately not the engine's scorer: these rows are
/// not candidates, they are a list the user is looking at, and the useful thing
/// is to narrow it without reordering what the block's author wrote.
enum LevelFilter {
    static func matches(_ query: String, row: SourceLevelRow) -> Bool {
        guard !query.isEmpty else { return true }
        let needle = query.lowercased()
        if row.title.lowercased().contains(needle) { return true }
        if row.id.lowercased().contains(needle) { return true }
        if let subtitle = row.subtitle, subtitle.lowercased().contains(needle) { return true }
        if let group = row.group, group.lowercased().contains(needle) { return true }
        return false
    }
}
