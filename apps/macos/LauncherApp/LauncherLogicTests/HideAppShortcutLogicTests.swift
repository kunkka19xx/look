import XCTest
@testable import LauncherLogic

final class HideAppShortcutLogicTests: XCTestCase {
    func testDisablesShortcutWhileAnotherLauncherSurfaceOwnsUI() {
        XCTAssertFalse(
            HideAppShortcutLogic.allowsKeyboardHideApp(
                showsThemeSettings: true,
                isCommandMode: false,
                showsHelpScreen: false,
                hidesResultsForEmptyQuery: false,
                isPrefixSuggestionQuery: false,
                isCommandSuggestionQuery: false,
                isTranslationQuery: false,
                isClipboardQuery: false,
                isProcessQuery: false
            )
        )
        XCTAssertFalse(
            HideAppShortcutLogic.allowsKeyboardHideApp(
                showsThemeSettings: false,
                isCommandMode: true,
                showsHelpScreen: false,
                hidesResultsForEmptyQuery: false,
                isPrefixSuggestionQuery: false,
                isCommandSuggestionQuery: false,
                isTranslationQuery: false,
                isClipboardQuery: false,
                isProcessQuery: false
            )
        )
        XCTAssertFalse(
            HideAppShortcutLogic.allowsKeyboardHideApp(
                showsThemeSettings: false,
                isCommandMode: false,
                showsHelpScreen: true,
                hidesResultsForEmptyQuery: false,
                isPrefixSuggestionQuery: false,
                isCommandSuggestionQuery: false,
                isTranslationQuery: false,
                isClipboardQuery: false,
                isProcessQuery: false
            )
        )
    }

    func testDisablesShortcutOnEmptyHome() {
        XCTAssertFalse(
            HideAppShortcutLogic.allowsKeyboardHideApp(
                showsThemeSettings: false,
                isCommandMode: false,
                showsHelpScreen: false,
                hidesResultsForEmptyQuery: true,
                isPrefixSuggestionQuery: false,
                isCommandSuggestionQuery: false,
                isTranslationQuery: false,
                isClipboardQuery: false,
                isProcessQuery: false
            )
        )
    }

    func testDisablesShortcutInNonResultModes() {
        XCTAssertFalse(
            HideAppShortcutLogic.allowsKeyboardHideApp(
                showsThemeSettings: false,
                isCommandMode: false,
                showsHelpScreen: false,
                hidesResultsForEmptyQuery: false,
                isPrefixSuggestionQuery: true,
                isCommandSuggestionQuery: false,
                isTranslationQuery: false,
                isClipboardQuery: false,
                isProcessQuery: false
            )
        )
        XCTAssertFalse(
            HideAppShortcutLogic.allowsKeyboardHideApp(
                showsThemeSettings: false,
                isCommandMode: false,
                showsHelpScreen: false,
                hidesResultsForEmptyQuery: false,
                isPrefixSuggestionQuery: false,
                isCommandSuggestionQuery: true,
                isTranslationQuery: false,
                isClipboardQuery: false,
                isProcessQuery: false
            )
        )
        XCTAssertFalse(
            HideAppShortcutLogic.allowsKeyboardHideApp(
                showsThemeSettings: false,
                isCommandMode: false,
                showsHelpScreen: false,
                hidesResultsForEmptyQuery: false,
                isPrefixSuggestionQuery: false,
                isCommandSuggestionQuery: false,
                isTranslationQuery: true,
                isClipboardQuery: false,
                isProcessQuery: false
            )
        )
        XCTAssertFalse(
            HideAppShortcutLogic.allowsKeyboardHideApp(
                showsThemeSettings: false,
                isCommandMode: false,
                showsHelpScreen: false,
                hidesResultsForEmptyQuery: false,
                isPrefixSuggestionQuery: false,
                isCommandSuggestionQuery: false,
                isTranslationQuery: false,
                isClipboardQuery: true,
                isProcessQuery: false
            )
        )
        XCTAssertFalse(
            HideAppShortcutLogic.allowsKeyboardHideApp(
                showsThemeSettings: false,
                isCommandMode: false,
                showsHelpScreen: false,
                hidesResultsForEmptyQuery: false,
                isPrefixSuggestionQuery: false,
                isCommandSuggestionQuery: false,
                isTranslationQuery: false,
                isClipboardQuery: false,
                isProcessQuery: true
            )
        )
    }

    func testEnablesShortcutOnNormalResultsSurface() {
        XCTAssertTrue(
            HideAppShortcutLogic.allowsKeyboardHideApp(
                showsThemeSettings: false,
                isCommandMode: false,
                showsHelpScreen: false,
                hidesResultsForEmptyQuery: false,
                isPrefixSuggestionQuery: false,
                isCommandSuggestionQuery: false,
                isTranslationQuery: false,
                isClipboardQuery: false,
                isProcessQuery: false
            )
        )
    }
}
