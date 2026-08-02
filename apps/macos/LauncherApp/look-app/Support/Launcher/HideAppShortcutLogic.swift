import Foundation

enum HideAppShortcutLogic {
    static func allowsKeyboardHideApp(
        showsThemeSettings: Bool,
        isCommandMode: Bool,
        showsHelpScreen: Bool,
        hidesResultsForEmptyQuery: Bool,
        isPrefixSuggestionQuery: Bool,
        isCommandSuggestionQuery: Bool,
        isTranslationQuery: Bool,
        isClipboardQuery: Bool
    ) -> Bool {
        !showsThemeSettings
            && !isCommandMode
            && !showsHelpScreen
            && !hidesResultsForEmptyQuery
            && !isPrefixSuggestionQuery
            && !isCommandSuggestionQuery
            && !isTranslationQuery
            && !isClipboardQuery
    }
}
