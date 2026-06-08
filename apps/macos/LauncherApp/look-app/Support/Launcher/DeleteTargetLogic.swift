import Foundation

/// Pure logic for the "delete file/folder to Trash" feature, kept free of
/// AppKit/SwiftUI so it can be unit-tested in the LauncherLogic package.
///
/// The View layer (`DeleteCommand` / `LauncherView`) handles the actual
/// `NSWorkspace.recycle` call, icons, and state; everything here is data-in /
/// data-out so it's deterministic under test.
enum DeleteTargetLogic {
    /// Filters a set of candidate results down to those that can be trashed:
    /// only `.file`/`.folder` kinds, excluding URL-scheme "paths" (settings
    /// panes, custom schemes) and anything no longer present on disk.
    ///
    /// `fileExists` is injected so tests don't need a real filesystem.
    static func eligible(
        from results: [LauncherResult],
        fileExists: (String) -> Bool
    ) -> [LauncherResult] {
        results.filter { result in
            guard result.kind == .file || result.kind == .folder else { return false }
            if isURLScheme(result.path) { return false }
            return fileExists(result.path)
        }
    }

    /// URL-scheme targets contain a ":" and don't start at the filesystem root.
    static func isURLScheme(_ path: String) -> Bool {
        path.contains(":") && !path.hasPrefix("/")
    }

    /// Title for the confirmation banner. Singular names the item; plural shows
    /// a count so a large basket can't be trashed on a reflexive "Y".
    static func confirmTitle(displayNames: [String]) -> String {
        if displayNames.count == 1 {
            return "Move \"\(displayNames[0])\" to Trash?"
        }
        return "Move \(displayNames.count) items to Trash?"
    }

    /// Detail line: the path for a single item, otherwise a file/folder count.
    static func confirmDetail(fileCount: Int, folderCount: Int, singlePath: String?) -> String {
        if let singlePath, fileCount + folderCount == 1 {
            return singlePath
        }
        var parts: [String] = []
        if fileCount > 0 { parts.append("\(fileCount) file\(fileCount == 1 ? "" : "s")") }
        if folderCount > 0 { parts.append("\(folderCount) folder\(folderCount == 1 ? "" : "s")") }
        return parts.joined(separator: ", ")
    }

    /// Banner text + error flag summarizing a trash outcome.
    static func resultMessage(trashedCount: Int, failureCount: Int, firstFailure: (name: String, reason: String)?) -> (text: String, isError: Bool) {
        if failureCount == 0 {
            return ("Moved \(trashedCount) to Trash", false)
        }
        if trashedCount == 0 {
            let f = firstFailure
            return ("Failed to trash \(f?.name ?? "item"): \(f?.reason ?? "unknown error")", true)
        }
        return ("Moved \(trashedCount), \(failureCount) failed", true)
    }
}
