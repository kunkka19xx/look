import Foundation

/// What ⌘I pastes, and when the chord is allowed to act: a clipboard row
/// pastes its clip, anywhere else the picks go, or the file or folder under
/// the cursor. Pure data-in / data-out so it can be unit-tested;
/// `FrontmostAppPaste` and `LauncherView` do the pasteboard write and the
/// keystroke.
enum ClipboardPasteTarget: Equatable {
    /// Labeled entries (`2+2 = 4`) paste their value, the same one Enter copies.
    case text(String)
    /// The pixels live in the store, so an image clip travels as a result id.
    case image(resultID: String)
    /// Files and folders, written the way a pick is: Finder pastes the items,
    /// a text field pastes the paths.
    case files([String])

    /// The picks outrank the row since picking is how several get chosen at
    /// all. A declared row's path is where its chords act, so it pastes once
    /// it is on disk; `fileExists` is the stat, injected for the tests.
    static func resolve(
        _ result: LauncherResult?,
        picked: [LauncherResult] = [],
        fileExists: (String) -> Bool = { FileManager.default.fileExists(atPath: $0) }
    ) -> ClipboardPasteTarget? {
        if let clip = clip(result) { return clip }
        let picks = picked.filter { $0.kind.isFileOrFolder }.map(\.path)
        if !picks.isEmpty { return .files(picks) }
        guard let result, onDisk(result, fileExists: fileExists) else { return nil }
        return .files([result.path])
    }

    private static func onDisk(_ result: LauncherResult, fileExists: (String) -> Bool) -> Bool {
        switch result.kind {
        case .file, .folder: return true
        case .action: return !result.path.isEmpty && fileExists(result.path)
        default: return false
        }
    }

    private static func clip(_ result: LauncherResult?) -> ClipboardPasteTarget? {
        guard let result, result.kind == .clipboard else { return nil }
        if result.isClipboardImage {
            return .image(resultID: result.id)
        }
        guard let content = result.clipboardPayload ?? result.clipboardContent,
            !content.isEmpty
        else { return nil }
        return .text(content)
    }

    /// Mirrors `DeleteTargetLogic.allowsKeyboardDelete`: a selection hidden
    /// behind another surface must not be acted on.
    static func allowsKeyboardPaste(
        showsThemeSettings: Bool,
        showsHelpScreen: Bool,
        inCommandMode: Bool,
        inAIMode: Bool
    ) -> Bool {
        !showsThemeSettings && !showsHelpScreen && !inCommandMode && !inAIMode
    }
}
