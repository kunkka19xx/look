import Foundation

/// What ⌘I on a clipboard row pastes, and when the chord is allowed to act.
/// Pure data-in / data-out so it can be unit-tested; `FrontmostAppPaste` and
/// `LauncherView` do the pasteboard write and the keystroke.
enum ClipboardPasteTarget: Equatable {
    /// Labeled entries (`2+2 = 4`) paste their value, the same one Enter copies.
    case text(String)
    /// The pixels live in the store, so an image clip travels as a result id.
    case image(resultID: String)

    static func resolve(_ result: LauncherResult?) -> ClipboardPasteTarget? {
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
