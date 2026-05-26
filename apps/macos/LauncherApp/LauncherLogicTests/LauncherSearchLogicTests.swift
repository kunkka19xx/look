import XCTest
@testable import LauncherLogic

final class LauncherSearchLogicTests: XCTestCase {
    private var testSandboxURL: URL?

    override func setUpWithError() throws {
        try super.setUpWithError()

        let folderName = "LauncherLogicTests-\(UUID().uuidString)"
        let sandboxURL = FileManager.default.temporaryDirectory.appendingPathComponent(folderName)
        try FileManager.default.createDirectory(at: sandboxURL, withIntermediateDirectories: true)
        testSandboxURL = sandboxURL
    }

    override func tearDownWithError() throws {
        if let sandboxURL = testSandboxURL,
           FileManager.default.fileExists(atPath: sandboxURL.path)
        {
            try? FileManager.default.removeItem(at: sandboxURL)
        }
        testSandboxURL = nil

        try super.tearDownWithError()
    }

    func testPinnedScopeDetection() {
        XCTAssertEqual(LauncherSearchLogic.pinnedLookupScope(for: "finder"), .unscoped)
        XCTAssertEqual(LauncherSearchLogic.pinnedLookupScope(for: "a\"finder"), .apps)
        XCTAssertEqual(LauncherSearchLogic.pinnedLookupScope(for: "f\"report"), .files)
        XCTAssertEqual(LauncherSearchLogic.pinnedLookupScope(for: "d\"doc"), .folders)
        XCTAssertEqual(LauncherSearchLogic.pinnedLookupScope(for: "r\".*"), .disabled)
        XCTAssertEqual(LauncherSearchLogic.pinnedLookupScope(for: "c\"note"), .disabled)
    }

    func testNormalizedPinnedQueryRespectsScope() {
        XCTAssertEqual(
            LauncherSearchLogic.normalizedPinnedLookupQuery(for: "a\" Finder ", scope: .apps),
            "finder"
        )
        XCTAssertEqual(
            LauncherSearchLogic.normalizedPinnedLookupQuery(for: "d\" Documents ", scope: .folders),
            "documents"
        )
        XCTAssertNil(
            LauncherSearchLogic.normalizedPinnedLookupQuery(for: "r\"foo", scope: .disabled)
        )
        XCTAssertNil(
            LauncherSearchLogic.normalizedPinnedLookupQuery(for: "   ", scope: .unscoped)
        )
    }

    func testFinderInjectionOnlyForAllowedScopes() {
        XCTAssertTrue(LauncherSearchLogic.shouldInjectFinder(normalizedQuery: "find", scope: .unscoped))
        XCTAssertTrue(LauncherSearchLogic.shouldInjectFinder(normalizedQuery: "finder", scope: .apps))
        XCTAssertFalse(LauncherSearchLogic.shouldInjectFinder(normalizedQuery: "finder", scope: .files))
        XCTAssertFalse(LauncherSearchLogic.shouldInjectFinder(normalizedQuery: "finder", scope: .folders))
        XCTAssertFalse(LauncherSearchLogic.shouldInjectFinder(normalizedQuery: "fi", scope: .unscoped))
        XCTAssertFalse(LauncherSearchLogic.shouldInjectFinder(normalizedQuery: nil, scope: .apps))
    }

    func testDedupeKeepsSameNameFilesFromDifferentPaths() {
        let first = LauncherResult(
            id: "file:1",
            kind: .file,
            title: "1.png",
            subtitle: nil,
            path: "/Users/test/Desktop/1.png",
            score: 10
        )
        let second = LauncherResult(
            id: "file:2",
            kind: .file,
            title: "1.png",
            subtitle: nil,
            path: "/Users/test/Documents/1.png",
            score: 8
        )

        let deduped = LauncherSearchLogic.dedupe(results: [first, second])
        XCTAssertEqual(deduped.count, 2)
    }

    func testDedupeRemovesDuplicateAppTitles() {
        let first = LauncherResult(
            id: "app:1",
            kind: .app,
            title: "Finder",
            subtitle: nil,
            path: "/System/Library/CoreServices/Finder.app",
            score: 10
        )
        let second = LauncherResult(
            id: "app:2",
            kind: .app,
            title: "Finder",
            subtitle: "Pinned",
            path: "/Applications/Finder.app",
            score: 9
        )

        let deduped = LauncherSearchLogic.dedupe(results: [first, second])
        XCTAssertEqual(deduped.count, 1)
        XCTAssertEqual(deduped.first?.id, "app:1")
    }

    func testDedupeRemovesDuplicateFoldersByPath() {
        let first = LauncherResult(
            id: "folder:1",
            kind: .folder,
            title: "Documents",
            subtitle: nil,
            path: "/Users/test/Documents",
            score: 10
        )
        let second = LauncherResult(
            id: "folder:2",
            kind: .folder,
            title: "My Docs",
            subtitle: nil,
            path: "/Users/test/Documents",
            score: 8
        )

        let deduped = LauncherSearchLogic.dedupe(results: [first, second])
        XCTAssertEqual(deduped.count, 1)
        XCTAssertEqual(deduped.first?.id, "folder:1")
    }

    func testDedupeKeepsClipboardItemsWithDifferentIDs() {
        var first = LauncherResult(
            id: "clipboard:1",
            kind: .clipboard,
            title: "Token",
            subtitle: nil,
            path: AppConstants.Launcher.Clipboard.resultPath,
            score: 10
        )
        first.clipboardContent = "same-content"

        var second = LauncherResult(
            id: "clipboard:2",
            kind: .clipboard,
            title: "Token",
            subtitle: nil,
            path: AppConstants.Launcher.Clipboard.resultPath,
            score: 8
        )
        second.clipboardContent = "same-content"

        let deduped = LauncherSearchLogic.dedupe(results: [first, second])
        XCTAssertEqual(deduped.count, 2)
    }
}
