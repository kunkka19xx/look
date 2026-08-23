import XCTest
@testable import LauncherLogic

/// The drill-down stack: what the query bar says, what the core is told about
/// ancestors, and what Escape puts back.
final class SourceLevelStackTests: XCTestCase {
    private func row(_ id: String) -> SourceLevelRow {
        SourceLevelRow(candidateId: "src:child:\(id)", id: id, title: id, subtitle: "", path: nil)
    }

    private func frame(
        block: String, parent: String, path: String = "", query: String = "",
        selection: String? = nil
    ) -> SourceLevelFrame {
        SourceLevelFrame(
            blockName: block,
            parentRowID: parent,
            parentTitle: parent,
            parentPath: path,
            rows: [row("one")],
            restoredQuery: query,
            restoredSelectionID: selection
        )
    }

    // ── The breadcrumb says where you are AND what you are looking at ────

    func testBreadcrumbEndsWithTheBlockNotTheRowYouCameFrom() {
        // "look" alone cannot tell Changed files from Branches: the row you
        // came from does not say which of its targets you picked.
        var stack = SourceLevelStack()
        stack.push(frame(block: "Changed files", parent: "look"))
        XCTAssertEqual(stack.breadcrumb, ["look", "Changed files"])
    }

    func testBreadcrumbWalksEveryLevelInOrder() {
        var stack = SourceLevelStack()
        stack.push(frame(block: "Browse", parent: "look"))
        stack.push(frame(block: "Browse", parent: "apps"))
        XCTAssertEqual(stack.breadcrumb, ["look", "apps", "Browse"])
    }

    func testAnEmptyStackHasNoBreadcrumbAndIsNotActive() {
        let stack = SourceLevelStack()
        XCTAssertTrue(stack.breadcrumb.isEmpty)
        XCTAssertFalse(stack.isActive)
    }

    // ── Ancestors reach the core nearest-first, which is what {parent.*} means ──

    func testAncestorsAreNearestFirst() {
        var stack = SourceLevelStack()
        stack.push(frame(block: "Browse", parent: "look", path: "/dev/look"))
        stack.push(frame(block: "Browse", parent: "apps", path: "/dev/look/apps"))

        let ancestors = stack.ancestorsOfCurrentRows
        // {parent.path} is the level directly above these rows, and each
        // further `parent.` steps one further out.
        XCTAssertEqual(ancestors.map(\.id), ["apps", "look"])
        XCTAssertEqual(ancestors.first?.path, "/dev/look/apps")
    }

    func testAncestorsEncodeAsTheCoreReadsThem() throws {
        var stack = SourceLevelStack()
        stack.push(frame(block: "Scripts", parent: "animate", path: "/dev/animate"))

        let json = stack.ancestorsOfCurrentRows.ancestorsJSON
        let decoded = try JSONSerialization.jsonObject(with: Data(json.utf8))
        let ancestors = try XCTUnwrap(decoded as? [[String: String]])
        XCTAssertEqual(ancestors.count, 1)
        XCTAssertEqual(ancestors.first?["path"], "/dev/animate")
    }

    func testNoLevelsMeansAnEmptyPayloadRatherThanAnEmptyString() {
        // The core parses this as JSON, so "no ancestors" has to be a list.
        XCTAssertEqual([SourceLevelParent]().ancestorsJSON, "[]")
    }

    // ── Escape restores rather than re-searching ────────────────────────

    func testPoppingHandsBackWhatTheLevelWasOpenedFrom() {
        var stack = SourceLevelStack()
        stack.push(frame(block: "Branches", parent: "look", query: "loo", selection: "src:projects:look"))

        let left = stack.pop()
        XCTAssertEqual(left?.restoredQuery, "loo")
        XCTAssertEqual(left?.restoredSelectionID, "src:projects:look")
        XCTAssertFalse(stack.isActive)
        XCTAssertNil(stack.pop())
    }
}
