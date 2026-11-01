import XCTest

@testable import LauncherLogic

/// What ⌘I pastes, and when it declines the chord.
final class ClipboardPasteTargetTests: XCTestCase {
    private func clipboardResult(
        id: String = "clipboard:1",
        content: String? = "hello",
        payload: String? = nil,
        imagePath: String? = nil
    ) -> LauncherResult {
        var result = LauncherResult(
            id: id,
            kind: .clipboard,
            title: "hello",
            subtitle: "Clipboard",
            path: AppConstants.Launcher.Clipboard.resultPath,
            score: 0
        )
        result.clipboardContent = content
        result.clipboardPayload = payload
        result.clipboardImagePath = imagePath
        return result
    }

    func testATextClipPastesItsContent() {
        XCTAssertEqual(ClipboardPasteTarget.resolve(clipboardResult()), .text("hello"))
    }

    /// The failure this guards: a calculator clip pasting `2+2 = 4` instead of
    /// the answer, which is what the list shows but not what Enter copies.
    func testALabeledClipPastesItsValueNotItsLabel() {
        let labeled = clipboardResult(content: "2+2 = 4", payload: "4")
        XCTAssertEqual(ClipboardPasteTarget.resolve(labeled), .text("4"))
    }

    func testAnImageClipIsAddressedByResultID() {
        let image = clipboardResult(id: "clipimage:7", content: nil, imagePath: "/tmp/a.png")
        XCTAssertEqual(ClipboardPasteTarget.resolve(image), .image(resultID: "clipimage:7"))
    }

    func testAnEmptyOrMissingClipResolvesToNothing() {
        XCTAssertNil(ClipboardPasteTarget.resolve(clipboardResult(content: "")))
        XCTAssertNil(ClipboardPasteTarget.resolve(clipboardResult(content: nil)))
        XCTAssertNil(ClipboardPasteTarget.resolve(nil))
    }

    private func pathResult(kind: LauncherResultKind, path: String) -> LauncherResult {
        LauncherResult(
            id: "\(kind.rawValue):\(path)", kind: kind, title: path, subtitle: "", path: path,
            score: 0)
    }

    func testAFileOrFolderRowPastesItself() {
        let file = pathResult(kind: .file, path: "/tmp/notes.txt")
        XCTAssertEqual(ClipboardPasteTarget.resolve(file), .files(["/tmp/notes.txt"]))
        let folder = pathResult(kind: .folder, path: "/tmp")
        XCTAssertEqual(ClipboardPasteTarget.resolve(folder), .files(["/tmp"]))
    }

    func testThePicksOutrankTheRow() {
        let row = pathResult(kind: .file, path: "/tmp/notes.txt")
        let picks = [
            pathResult(kind: .file, path: "/tmp/a.txt"),
            pathResult(kind: .app, path: "/Applications/Safari.app"),
            pathResult(kind: .folder, path: "/tmp/b"),
        ]
        XCTAssertEqual(
            ClipboardPasteTarget.resolve(row, picked: picks), .files(["/tmp/a.txt", "/tmp/b"]))
    }

    func testAClipRowPastesTheClipEvenWithPicks() {
        let picks = [pathResult(kind: .file, path: "/tmp/a.txt")]
        XCTAssertEqual(
            ClipboardPasteTarget.resolve(clipboardResult(), picked: picks), .text("hello"))
    }

    func testAnAppRowResolvesToNothing() {
        let app = pathResult(kind: .app, path: "/Applications/Safari.app")
        XCTAssertNil(ClipboardPasteTarget.resolve(app))
        XCTAssertNil(ClipboardPasteTarget.resolve(app, picked: [app]))
    }

    func testTheChordIsRefusedWhileAnotherSurfaceOwnsTheWindow() {
        XCTAssertTrue(
            ClipboardPasteTarget.allowsKeyboardPaste(
                showsThemeSettings: false, showsHelpScreen: false, inCommandMode: false,
                inAIMode: false))
        XCTAssertFalse(
            ClipboardPasteTarget.allowsKeyboardPaste(
                showsThemeSettings: true, showsHelpScreen: false, inCommandMode: false,
                inAIMode: false))
        XCTAssertFalse(
            ClipboardPasteTarget.allowsKeyboardPaste(
                showsThemeSettings: false, showsHelpScreen: true, inCommandMode: false,
                inAIMode: false))
        XCTAssertFalse(
            ClipboardPasteTarget.allowsKeyboardPaste(
                showsThemeSettings: false, showsHelpScreen: false, inCommandMode: true,
                inAIMode: false))
        XCTAssertFalse(
            ClipboardPasteTarget.allowsKeyboardPaste(
                showsThemeSettings: false, showsHelpScreen: false, inCommandMode: false,
                inAIMode: true))
    }
}
