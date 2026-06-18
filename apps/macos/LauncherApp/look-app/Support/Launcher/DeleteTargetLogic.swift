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
        fileExists: (String) -> Bool,
        homeDirectory: String = NSHomeDirectory()
    ) -> [LauncherResult] {
        results.filter { result in
            guard result.kind == .file || result.kind == .folder else { return false }
            if isQuickFolderPin(result.id) { return false }
            if isURLScheme(result.path) { return false }
            if isProtected(result.path, homeDirectory: homeDirectory) { return false }
            return fileExists(result.path)
        }
    }

    /// Quick-folder results (Desktop, Documents, …, Applications, Trash) are
    /// navigation pins, not delete targets - Cmd+D must never trash the real
    /// folder behind one. They aren't covered by `isProtected` (most are ordinary
    /// home subfolders / `/Applications`), so they're excluded by id here. Cmd+D
    /// on the Trash pin is routed to Empty Trash before eligibility is checked
    /// (see `requestDeleteSelection`).
    static func isQuickFolderPin(_ id: String) -> Bool {
        id.hasPrefix(AppConstants.Launcher.QuickFolder.idPrefix)
    }

    /// URL-scheme targets contain a ":" and don't start at the filesystem root.
    static func isURLScheme(_ path: String) -> Bool {
        path.contains(":") && !path.hasPrefix("/")
    }

    /// Refuses to trash catastrophic targets: the filesystem root, the home
    /// directory itself, and the Trash. (Trash is now a pinned quick folder, so
    /// Cmd+D on it would otherwise try to recycle ~/.Trash into itself.)
    static func isProtected(_ path: String, homeDirectory: String) -> Bool {
        let p = normalize(path)
        if p.isEmpty || p == "/" { return true }
        if p == normalize(homeDirectory) { return true }
        if isTrashPath(path, homeDirectory: homeDirectory) { return true }
        return false
    }

    /// True when `path` is the user's Trash (`~/.Trash`). Used both to protect
    /// it from being trashed and to route Cmd+D on it to "Empty Trash".
    static func isTrashPath(_ path: String, homeDirectory: String) -> Bool {
        normalize(path) == normalize(homeDirectory) + "/.Trash"
    }

    /// Detail line for the Empty Trash confirmation, stressing permanence.
    static func emptyTrashDetail(itemCount: Int) -> String {
        "\(itemCount) item\(itemCount == 1 ? "" : "s") — deleted permanently"
    }

    private static func normalize(_ path: String) -> String {
        var p = path
        while p.count > 1 && p.hasSuffix("/") { p.removeLast() }
        return p
    }

    /// Banner text + error flag summarizing a trash outcome.
    static func resultMessage(
        trashedCount: Int, failureCount: Int, firstFailure: (name: String, reason: String)?
    ) -> (text: String, isError: Bool) {
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
