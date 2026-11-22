import XCTest

@testable import LauncherLogic

final class ResultRowDigitsTests: XCTestCase {
    private typealias RowDigits = AppConstants.Launcher.ResultRowDigits

    func testNineRowsFromTheTopOneInViewTakeADigit() {
        XCTAssertNil(RowDigits.digit(forRow: 1, firstInView: 2))
        XCTAssertEqual(RowDigits.digit(forRow: 2, firstInView: 2), 1)
        XCTAssertEqual(RowDigits.digit(forRow: 10, firstInView: 2), 9)
        XCTAssertNil(RowDigits.digit(forRow: 11, firstInView: 2))
    }

    func testADigitOpensTheRowWearingIt() {
        XCTAssertEqual(RowDigits.row(forDigit: 1, firstInView: 2), 2)
        XCTAssertEqual(RowDigits.row(forDigit: 9, firstInView: 2), 10)
        XCTAssertNil(RowDigits.row(forDigit: 0, firstInView: 2))
        for row in 0..<20 {
            guard let digit = RowDigits.digit(forRow: row, firstInView: 4) else { continue }
            XCTAssertEqual(RowDigits.row(forDigit: digit, firstInView: 4), row)
        }
    }

    func testTheNumberRowKeyCodesMapToTheirDigits() {
        typealias KeyCode = AppConstants.Launcher.KeyCode
        let numberRow: [UInt16] = [29, 18, 19, 20, 21, 23, 22, 26, 28, 25]
        XCTAssertEqual(numberRow.map(KeyCode.digit), Array(0...9))
        XCTAssertNil(KeyCode.digit(KeyCode.returnKey))
    }
}
