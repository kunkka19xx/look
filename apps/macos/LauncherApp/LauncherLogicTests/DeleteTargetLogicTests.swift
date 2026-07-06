import XCTest
@testable import LauncherLogic

final class DeleteTargetLogicTests: XCTestCase {
    private func result(_ id: String, _ kind: LauncherResultKind, path: String) -> LauncherResult {
        LauncherResult(id: id, kind: kind, title: id, subtitle: nil, path: path, score: 0)
    }

    // MARK: - eligible(from:fileExists:)

    func testKeepsFilesAndFoldersThatExist() {
        let input = [
            result("a", .file, path: "/tmp/a.txt"),
            result("b", .folder, path: "/tmp/dir"),
        ]
        let eligible = DeleteTargetLogic.eligible(from: input, fileExists: { _ in true })
        XCTAssertEqual(eligible.map(\.id), ["a", "b"])
    }

    func testDropsAppsAndClipboard() {
        let input = [
            result("app", .app, path: "/Applications/Foo.app"),
            result("clip", .clipboard, path: "clipboard://1"),
            result("file", .file, path: "/tmp/keep.txt"),
        ]
        let eligible = DeleteTargetLogic.eligible(from: input, fileExists: { _ in true })
        XCTAssertEqual(eligible.map(\.id), ["file"])
    }

    func testDropsURLSchemePaths() {
        let input = [
            result("settings", .file, path: "x-apple.systempreferences:com.apple.preference"),
            result("real", .file, path: "/Users/me/doc.pdf"),
        ]
        let eligible = DeleteTargetLogic.eligible(from: input, fileExists: { _ in true })
        XCTAssertEqual(eligible.map(\.id), ["real"])
    }

    func testDropsNonExistentPaths() {
        let input = [
            result("gone", .file, path: "/tmp/gone.txt"),
            result("here", .file, path: "/tmp/here.txt"),
        ]
        let eligible = DeleteTargetLogic.eligible(from: input, fileExists: { $0 == "/tmp/here.txt" })
        XCTAssertEqual(eligible.map(\.id), ["here"])
    }

    func testProtectsHomeRootAndTrash() {
        let home = "/Users/me"
        let input = [
            result("root", .folder, path: "/"),
            result("home", .folder, path: "/Users/me"),
            result("homeSlash", .folder, path: "/Users/me/"),
            result("trash", .folder, path: "/Users/me/.Trash"),
            result("ok", .folder, path: "/Users/me/Projects"),
        ]
        let eligible = DeleteTargetLogic.eligible(
            from: input,
            fileExists: { _ in true },
            homeDirectory: home
        )
        XCTAssertEqual(eligible.map(\.id), ["ok"])
    }

    func testDropsQuickFolderPins() {
        // Quick-folder pins are navigation shortcuts, not delete targets, even
        // though they're real, existing folders outside the protected set.
        let prefix = AppConstants.Launcher.QuickFolder.idPrefix
        let input = [
            result("\(prefix)applications", .folder, path: "/Applications"),
            result("\(prefix)documents", .folder, path: "/Users/me/Documents"),
            result("real", .folder, path: "/Users/me/Projects"),
        ]
        let eligible = DeleteTargetLogic.eligible(
            from: input,
            fileExists: { _ in true },
            homeDirectory: "/Users/me"
        )
        XCTAssertEqual(eligible.map(\.id), ["real"])
    }

    // MARK: - isTrashPath

    func testIsTrashPath() {
        let home = "/Users/me"
        XCTAssertTrue(DeleteTargetLogic.isTrashPath("/Users/me/.Trash", homeDirectory: home))
        XCTAssertTrue(DeleteTargetLogic.isTrashPath("/Users/me/.Trash/", homeDirectory: home))
        XCTAssertFalse(DeleteTargetLogic.isTrashPath("/Users/me/.Trash/file.txt", homeDirectory: home))
        XCTAssertFalse(DeleteTargetLogic.isTrashPath("/Users/me/Documents", homeDirectory: home))
    }

    // MARK: - empty trash wording

    func testEmptyTrashDetailPluralization() {
        XCTAssertEqual(DeleteTargetLogic.emptyTrashDetail(itemCount: 1), "1 item - deleted permanently")
        XCTAssertEqual(DeleteTargetLogic.emptyTrashDetail(itemCount: 7), "7 items - deleted permanently")
    }

    // MARK: - resultMessage

    func testResultMessageAllSucceeded() {
        let (text, isError) = DeleteTargetLogic.resultMessage(trashedCount: 3, failureCount: 0, firstFailure: nil)
        XCTAssertEqual(text, "Moved 3 to Trash")
        XCTAssertFalse(isError)
    }

    func testResultMessageAllFailed() {
        let (text, isError) = DeleteTargetLogic.resultMessage(
            trashedCount: 0,
            failureCount: 1,
            firstFailure: (name: "locked.txt", reason: "permission denied")
        )
        XCTAssertEqual(text, "Failed to trash locked.txt: permission denied")
        XCTAssertTrue(isError)
    }

    func testResultMessagePartialFailure() {
        let (text, isError) = DeleteTargetLogic.resultMessage(
            trashedCount: 2,
            failureCount: 1,
            firstFailure: (name: "x", reason: "y")
        )
        XCTAssertEqual(text, "Moved 2, 1 failed")
        XCTAssertTrue(isError)
    }
}
