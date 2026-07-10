import Foundation

/// Pure helpers for folder-browse: keyboard navigation of the preview pane's
/// folder listing. Kept UI-free so they can be unit-tested in the
/// LauncherLogic package.
enum FolderBrowseLogic {
    /// Absolute path of a listed child inside its parent folder.
    static func childPath(parent: String, name: String) -> String {
        URL(fileURLWithPath: parent).appendingPathComponent(name).path
    }

    /// Steps the highlighted row by `delta`, wrapping at both ends (matches
    /// the results list's Up/Down behavior). Out-of-range `current` values
    /// (e.g. after a re-list shrank the folder) are clamped first.
    static func steppedIndex(from current: Int, count: Int, delta: Int) -> Int {
        guard count > 0 else { return 0 }
        let clamped = min(max(current, 0), count - 1)
        return ((clamped + delta) % count + count) % count
    }
}
