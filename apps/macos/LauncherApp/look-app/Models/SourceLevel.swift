import Foundation

/// One level of a drill-down: the rows a block produced against the row it was
/// opened from (`specs/user-sources.md` §2.10).
nonisolated struct SourceLevel: Decodable {
    /// The parent row's own id, decoded by the core so the shell never has to
    /// take a candidate id apart itself.
    let parentRowId: String
    let rows: [SourceLevelRow]
    /// The row cap dropped rows, so the list is not all there is.
    let truncated: Bool
    /// Why the level could not be produced. Non-nil means do not descend.
    let error: String?
}

/// A row inside a level. `candidateId` already encodes the levels it was reached
/// through, so usage ranks per ancestor path and two parents never share a row's
/// history.
nonisolated struct SourceLevelRow: Decodable, Identifiable {
    let candidateId: String
    let id: String
    let title: String
    /// Already resolved against the block name by the core, so a level row and
    /// an indexed one read the same.
    let subtitle: String
    let path: String?
}

/// The row a tool action acts on. One value because the four always travel
/// together, and because a block's `edit` / `terminal` / `reveal` expands
/// against the row exactly like its `open` does (`specs/preferred-tools.md` §6).
nonisolated struct ToolActionRow {
    let candidateID: String
    let title: String
    let path: String
    /// The levels it was reached through, for `{parent.*}`. Empty outside a
    /// drill-down.
    var ancestorsJSON: String = "[]"

    func withCStrings<T>(
        _ body: (UnsafePointer<CChar>?, UnsafePointer<CChar>?, UnsafePointer<CChar>?,
            UnsafePointer<CChar>?) -> T
    ) -> T {
        candidateID.withCString { candidate in
            title.withCString { title in
                path.withCString { path in
                    ancestorsJSON.withCString { ancestors in
                        body(candidate, title, path, ancestors)
                    }
                }
            }
        }
    }
}
