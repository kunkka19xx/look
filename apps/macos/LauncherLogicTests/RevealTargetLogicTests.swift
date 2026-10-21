import XCTest

@testable import LauncherLogic

final class RevealTargetLogicTests: XCTestCase {
    func testAnExistingFilesystemPathIsSelectedInTheFileViewer() {
        XCTAssertEqual(
            RevealTargetLogic.plan(for: "/tmp/look/a.txt", exists: true),
            .selectInFileViewer
        )
    }

    func testAMissingFilesystemPathIsUnavailable() {
        XCTAssertEqual(
            RevealTargetLogic.plan(for: "/tmp/look/gone.txt", exists: false),
            .unavailable
        )
    }

    /// A scheme is never stat'd, so `exists` must not decide it.
    func testAURLSchemeIsOpenedRegardlessOfTheStat() {
        for path in ["x-apple.systempreferences:com.apple.Bluetooth", "https://example.com"] {
            XCTAssertEqual(RevealTargetLogic.plan(for: path, exists: false), .openURL, path)
        }
    }

    func testAnEmptyPathIsUnavailable() {
        XCTAssertEqual(RevealTargetLogic.plan(for: "", exists: false), .unavailable)
        XCTAssertEqual(RevealTargetLogic.plan(for: "", exists: true), .unavailable)
    }
}
