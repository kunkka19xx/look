import Foundation

/// Paths as a reader wants to see them. Shared so the mention list, the
/// attachment capsule, and anything else that names a file abbreviate the same
/// way: two files called `main.go` are the normal case, and only the folder
/// tells them apart.
nonisolated enum PathDisplay {
    /// `~` for home, so the width goes to the part that identifies the file
    /// rather than to `/Users/<name>`.
    static func abbreviated(_ path: String) -> String {
        let home = NSHomeDirectory()
        // On the boundary, not the prefix: with a home of `/Users/alex`, a bare
        // prefix test turns `/Users/alexander/notes` into `~ander/notes`.
        guard path == home || (home != "/" && path.hasPrefix(home + "/")) else { return path }
        return "~" + path.dropFirst(home.count)
    }

    /// The containing folder, abbreviated. Empty for a path with no parent.
    static func directory(of path: String) -> String {
        let parent = (path as NSString).deletingLastPathComponent
        return parent.isEmpty ? "" : abbreviated(parent)
    }

    static func name(of path: String) -> String {
        (path as NSString).lastPathComponent
    }
}
