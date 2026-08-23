import Foundation

/// Descending into a `then` target that lists rather than performs.
///
/// A level takes the result list over: its rows are produced live against the
/// row it opened from, and filtered here, since they are not in the index.
extension LauncherView {
    var isInLevel: Bool { levelStack.isActive }

    /// Filtered by what is typed at this level. `.action` whatever the row
    /// carries: Enter runs the block's verbs, and a path only says where the
    /// chords act.
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
                    subtitle: row.subtitle,
                    path: row.path ?? "",
                    // The producer's order, kept: its author knows the domain.
                    score: level.rows.count - position
                )
            }
    }

    /// Opens `blockID` as a level below `parent`.
    ///
    /// The parent is passed in rather than read from the selection: the target
    /// that led here already ran a detached call, and the user may have moved
    /// on since it started.
    func descendIntoBlock(
        blockID: String, title: String, parent: LevelParentRow, ancestorsJSON: String
    ) {
        let openedFrom = (query: parent.openedFromQuery, selection: parent.openedFromSelection)
        // A level opened after the stack was cleared is a level from a launcher
        // the user has already closed.
        let epoch = levelEpoch

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
                guard epoch == levelEpoch else { return }
                guard let level else {
                    showBanner("\(title): the core did not answer", style: .error, duration: 3.0)
                    return
                }
                if let error = level.error {
                    // An empty level and a broken command look identical once
                    // you are inside one, so neither is entered.
                    showBanner("\(title): \(error)", style: .error, duration: 4.0)
                    return
                }

                levelStack.push(
                    SourceLevelFrame(
                        blockName: title,
                        parentRowID: level.parentRowId,
                        parentTitle: parent.title,
                        parentPath: parent.path,
                        rows: level.rows,
                        restoredQuery: openedFrom.query,
                        restoredSelectionID: openedFrom.selection
                    )
                )
                // The query that got here means nothing now.
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

    /// Back one level, with the query and selection it opened from. Returns
    /// whether there was a level to leave.
    @discardableResult
    func popLevel() -> Bool {
        guard let left = levelStack.pop() else { return false }
        levelEpoch &+= 1
        // Setting the query runs the change handler, which seeds the selection
        // from the first row. The restore has to survive that, so it is left
        // pending for whichever pass has the rows.
        pendingSelectionRestore = left.restoredSelectionID.map {
            PendingSelection(id: $0, query: left.restoredQuery)
        }
        query = left.restoredQuery
        selectedResultID = left.restoredSelectionID
        return true
    }

    func clearLevels() {
        guard levelStack.isActive || pendingSelectionRestore != nil else { return }
        levelEpoch &+= 1
        pendingSelectionRestore = nil
        levelStack.clear()
    }

    /// Ancestors of the selected row, for `{parent.*}`. Empty outside a level.
    var selectedRowAncestorsJSON: String {
        levelStack.ancestorsOfCurrentRows.ancestorsJSON
    }
}

/// The row a level is opened from, captured when the target was picked.
struct LevelParentRow {
    let candidateID: String
    let title: String
    let path: String
    let openedFromQuery: String
    let openedFromSelection: String?
}

/// A selection to put back once the rows it names are on screen.
struct PendingSelection {
    let id: String
    /// What the query was when it was captured: typing anything else means the
    /// user moved on and the restore is stale.
    let query: String
}

/// Not the engine's scorer: these rows are a list the user is looking at, and
/// narrowing it must not reorder what the block's author wrote.
enum LevelFilter {
    static func matches(_ query: String, row: SourceLevelRow) -> Bool {
        guard !query.isEmpty else { return true }
        let needle = query.lowercased()
        if row.title.lowercased().contains(needle) { return true }
        if row.id.lowercased().contains(needle) { return true }
        return row.subtitle.lowercased().contains(needle)
    }
}
