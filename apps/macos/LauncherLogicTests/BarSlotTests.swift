import XCTest

@testable import LauncherLogic

final class BarSlotTests: XCTestCase {
    private static let limit = 9

    private func ids(_ count: Int) -> [String] {
        (0..<count).map(String.init)
    }

    func testAPinTogglesAndTheTenthIsRefused() {
        XCTAssertEqual(BarPins.toggled(ids(2), "x", limit: Self.limit).0, ["0", "1", "x"])
        XCTAssertEqual(BarPins.toggled(ids(2), "x", limit: Self.limit).1, .pinned)
        XCTAssertEqual(BarPins.toggled(ids(2), "0", limit: Self.limit).0, ["1"])
        XCTAssertEqual(BarPins.toggled(ids(2), "0", limit: Self.limit).1, .unpinned)
        XCTAssertEqual(BarPins.toggled(ids(9), "x", limit: Self.limit).0, ids(9))
        XCTAssertEqual(BarPins.toggled(ids(9), "x", limit: Self.limit).1, .full)
        XCTAssertEqual(BarPins.toggled(ids(9), "8", limit: Self.limit).1, .unpinned)
    }

    func testAnEmptyDayAsksForAPlan() {
        let line = BarTodoLine(done: 0, total: 0, open: [], pick: 7)
        XCTAssertNil(line.task)
        XCTAssertEqual(line.note, BarTodoLine.emptyDay)
    }

    func testAFinishedDayShowsOnlyTheCount() {
        let line = BarTodoLine(done: 3, total: 3, open: [], pick: 7)
        XCTAssertNil(line.task)
        XCTAssertEqual(line.note, "3/3 done")
    }

    func testAnOpenTaskComesWithTheCount() {
        let open = ["write", "ship"]
        XCTAssertEqual(BarTodoLine(done: 1, total: 3, open: open, pick: 0).task, "write")
        XCTAssertEqual(BarTodoLine(done: 1, total: 3, open: open, pick: 5).task, "ship")
        XCTAssertEqual(BarTodoLine(done: 1, total: 3, open: open, pick: 5).note, "1/3")
    }

    func testOnlyTheAppSlotsTakeTheDigits() {
        XCTAssertEqual(BarSlot.allCases.filter(\.takesDigitKeys), [.running, .pinned])
    }
}
