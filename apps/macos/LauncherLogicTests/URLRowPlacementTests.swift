import XCTest
@testable import LauncherLogic

/// Where the "open this URL" row lands among local results.
final class URLRowPlacementTests: XCTestCase {
    private func row(_ id: String) -> LauncherResult {
        LauncherResult(id: id, kind: .app, title: id, subtitle: nil, path: id, score: 0)
    }

    private func ids(_ results: [LauncherResult]) -> [String] {
        results.map(\.id)
    }

    func testAStructuralURLLeads() {
        // It has a scheme or a path, so it cannot be a file or a search term.
        let placed = URLRowPlacement.merged(
            url: row("url"), isBareHost: false, into: [row("a"), row("b")])
        XCTAssertEqual(ids(placed), ["url", "a", "b"])
    }

    func testABareHostSitsBelowTheDefaultSlot() {
        // Enter still opens the best local match (issue #232), but the row is
        // always one Down away.
        let placed = URLRowPlacement.merged(
            url: row("url"), isBareHost: true, into: [row("a"), row("b"), row("c")])
        XCTAssertEqual(ids(placed), ["a", "url", "b", "c"])
    }

    /// The regression this replaces: the row sat after EVERY local result, and a
    /// declared source that matches a bare host on all of its rows (browser
    /// history carries the host in each subtitle) buried the address the user
    /// typed under hundreds of pages from that same address.
    func testABareHostIsReachableWhateverASourceContributes() {
        let flood = (0..<500).map { row("history-\($0)") }
        let placed = URLRowPlacement.merged(url: row("url"), isBareHost: true, into: flood)
        XCTAssertEqual(placed.count, 501)
        XCTAssertEqual(placed[1].id, "url")
    }

    func testWithNoLocalResultsTheURLRowLeadsEitherWay() {
        // Nothing to keep the default slot, so nothing to sit below.
        for isBareHost in [true, false] {
            let placed = URLRowPlacement.merged(url: row("url"), isBareHost: isBareHost, into: [])
            XCTAssertEqual(ids(placed), ["url"], "bare host: \(isBareHost)")
        }
    }

    func testASingleLocalResultKeepsTheDefaultSlot() {
        let placed = URLRowPlacement.merged(url: row("url"), isBareHost: true, into: [row("a")])
        XCTAssertEqual(ids(placed), ["a", "url"])
    }
}
