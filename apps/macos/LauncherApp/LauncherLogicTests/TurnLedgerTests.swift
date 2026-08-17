import XCTest

@testable import LauncherLogic

final class TurnLedgerTests: XCTestCase {
    func testFirstClaimWinsAndTheRestAreToldNo() {
        var ledger = TurnLedger()
        ledger.startTurn()
        let planner = UUID()
        XCTAssertTrue(ledger.shouldAppend(id: planner))
        // The planner already showed the message; the plan that comes back
        // later must not add it again (this was the duplicate turn bug).
        XCTAssertFalse(ledger.shouldAppend(id: UUID()))
        XCTAssertEqual(ledger.currentID, planner)
    }

    func testEachSubmitGetsItsOwnTurn() {
        var ledger = TurnLedger()
        ledger.startTurn()
        XCTAssertTrue(ledger.shouldAppend(id: UUID()))
        ledger.startTurn()
        XCTAssertTrue(ledger.shouldAppend(id: UUID()))
    }

    func testWithoutAStartTheFirstClaimStillWorks() {
        // A path that appends outside a submit (a restored session, a note)
        // must not be silently swallowed.
        var ledger = TurnLedger()
        XCTAssertTrue(ledger.shouldAppend(id: UUID()))
    }

    func testResetClearsTheInFlightTurn() {
        var ledger = TurnLedger()
        ledger.startTurn()
        XCTAssertTrue(ledger.shouldAppend(id: UUID()))
        ledger.reset()
        XCTAssertNil(ledger.currentID)
        XCTAssertTrue(ledger.shouldAppend(id: UUID()))
    }
}
