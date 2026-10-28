import XCTest
@testable import LauncherLogic

/// Where the calculator answer lands among the results.
final class CalcRowPlacementTests: XCTestCase {
    private func row(_ id: String, title: String? = nil) -> LauncherResult {
        LauncherResult(id: id, kind: .folder, title: title ?? id, subtitle: nil, path: id, score: 0)
    }

    private let calc = LauncherResult(
        id: "\(AppConstants.Launcher.Calc.resultIDPrefix)-2011", kind: .app, title: "-2,011",
        subtitle: nil, path: "-2011", score: .max)

    private func ids(_ results: [LauncherResult]) -> [String] {
        results.map(\.id)
    }

    func testTheAnswerLeads() {
        let placed = CalcRowPlacement.merged(calc: calc, query: "20-05-2026", into: [row("a"), row("b")])
        XCTAssertEqual(ids(placed), [calc.id, "a", "b"])
    }

    func testAnItemNamedExactlyThatKeepsTheDefaultSlot() {
        // Issue #325: a folder named for a date opens on Enter; the answer is one Down away.
        let placed = CalcRowPlacement.merged(
            calc: calc, query: " 20-05-2026 ", into: [row("dir", title: "20-05-2026"), row("b")])
        XCTAssertEqual(ids(placed), ["dir", calc.id, "b"])
    }

    func testAPartialNameDoesNotYield() {
        let placed = CalcRowPlacement.merged(
            calc: calc, query: "20-05-2026", into: [row("dir", title: "20-05-2026 trip")])
        XCTAssertEqual(ids(placed), [calc.id, "dir"])
    }

    func testASynthesizedRowNeverTakesTheSlot() {
        // A URL row titled with the query is not something the user has.
        let url = row("\(AppConstants.Launcher.WebURL.resultID(url: "16:9"))", title: "16:9")
        let placed = CalcRowPlacement.merged(calc: calc, query: "16:9", into: [url])
        XCTAssertEqual(ids(placed), [calc.id, url.id])
    }

    func testWithNoResultsTheAnswerIsAlone() {
        XCTAssertEqual(ids(CalcRowPlacement.merged(calc: calc, query: "2+2", into: [])), [calc.id])
    }
}
