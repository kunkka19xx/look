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

    // MARK: - confirmTitle

    func testConfirmTitleSingularNamesItem() {
        XCTAssertEqual(
            DeleteTargetLogic.confirmTitle(displayNames: ["report.pdf"]),
            "Move \"report.pdf\" to Trash?"
        )
    }

    func testConfirmTitlePluralShowsCount() {
        XCTAssertEqual(
            DeleteTargetLogic.confirmTitle(displayNames: ["a", "b", "c"]),
            "Move 3 items to Trash?"
        )
    }

    // MARK: - confirmDetail

    func testConfirmDetailSingleShowsPath() {
        XCTAssertEqual(
            DeleteTargetLogic.confirmDetail(fileCount: 1, folderCount: 0, singlePath: "/tmp/a.txt"),
            "/tmp/a.txt"
        )
    }

    func testConfirmDetailPluralCountsFilesAndFolders() {
        XCTAssertEqual(
            DeleteTargetLogic.confirmDetail(fileCount: 2, folderCount: 1, singlePath: nil),
            "2 files, 1 folder"
        )
    }

    func testConfirmDetailOmitsZeroCategory() {
        XCTAssertEqual(
            DeleteTargetLogic.confirmDetail(fileCount: 0, folderCount: 3, singlePath: nil),
            "3 folders"
        )
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
