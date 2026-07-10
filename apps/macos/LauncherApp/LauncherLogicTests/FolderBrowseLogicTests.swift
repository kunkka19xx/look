import XCTest
@testable import LauncherLogic

final class FolderBrowseLogicTests: XCTestCase {
    // MARK: - childPath(parent:name:)

    func testJoinsParentAndChildName() {
        XCTAssertEqual(
            FolderBrowseLogic.childPath(parent: "/Users/me/Documents", name: "Projects"),
            "/Users/me/Documents/Projects"
        )
    }

    func testJoinHandlesTrailingSlashOnParent() {
        let joined = FolderBrowseLogic.childPath(parent: "/Users/me/Desktop/", name: "a.txt")
        XCTAssertEqual(joined, "/Users/me/Desktop/a.txt")
        XCTAssertFalse(joined.contains("//"))
    }

    func testJoinPreservesNamesWithSpacesAndUnicode() {
        XCTAssertEqual(
            FolderBrowseLogic.childPath(parent: "/Users/me/Desktop", name: "Giục nữa thì vào mà làm!"),
            "/Users/me/Desktop/Giục nữa thì vào mà làm!"
        )
    }

    // MARK: - steppedIndex(from:count:delta:)

    func testStepsDownWithinBounds() {
        XCTAssertEqual(FolderBrowseLogic.steppedIndex(from: 0, count: 5, delta: 1), 1)
        XCTAssertEqual(FolderBrowseLogic.steppedIndex(from: 3, count: 5, delta: 1), 4)
    }

    func testStepsUpWithinBounds() {
        XCTAssertEqual(FolderBrowseLogic.steppedIndex(from: 4, count: 5, delta: -1), 3)
    }

    func testWrapsPastLastRowToFirst() {
        XCTAssertEqual(FolderBrowseLogic.steppedIndex(from: 4, count: 5, delta: 1), 0)
    }

    func testWrapsBeforeFirstRowToLast() {
        XCTAssertEqual(FolderBrowseLogic.steppedIndex(from: 0, count: 5, delta: -1), 4)
    }

    func testClampsStaleOutOfRangeIndexBeforeStepping() {
        // A re-list can shrink the folder while the old index is still held.
        XCTAssertEqual(FolderBrowseLogic.steppedIndex(from: 9, count: 3, delta: 1), 0)
        XCTAssertEqual(FolderBrowseLogic.steppedIndex(from: -2, count: 3, delta: -1), 2)
    }

    func testEmptyListingAlwaysYieldsZero() {
        XCTAssertEqual(FolderBrowseLogic.steppedIndex(from: 0, count: 0, delta: 1), 0)
        XCTAssertEqual(FolderBrowseLogic.steppedIndex(from: 5, count: 0, delta: -1), 0)
    }

    func testSingleRowStaysSelectedInBothDirections() {
        XCTAssertEqual(FolderBrowseLogic.steppedIndex(from: 0, count: 1, delta: 1), 0)
        XCTAssertEqual(FolderBrowseLogic.steppedIndex(from: 0, count: 1, delta: -1), 0)
    }
}
