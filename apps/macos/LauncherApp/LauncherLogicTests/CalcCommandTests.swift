import XCTest
@testable import LauncherLogic

final class CalcCommandTests: XCTestCase {
    func testExponentPrecedenceUnaryMinusBindsAfterPower() {
        assertValue("-2^2", equals: "-4.0000")
    }

    func testExponentPrecedenceSignedExponentChain() {
        assertValue("2^-3^2", equals: "0.0020")
    }

    func testExponentIsRightAssociative() {
        assertValue("2^3^2", equals: "512.0000")
    }

    func testFactorial() {
        assertValue("4!", equals: "24.0000")
        assertValue("5!", equals: "120.0000")
    }

    func testConstants() {
        assertValue("pi", equals: "3.1416")
        assertValue("e", equals: "2.7183")
        assertValue("2*pi", equals: "6.2832")
    }

    func testFunctions() {
        assertValue("abs(-5)", equals: "5.0000")
        assertValue("round(2.6)", equals: "3.0000")
        assertValue("floor(2.9)", equals: "2.0000")
        assertValue("ceil(2.1)", equals: "3.0000")
    }

    func testPercentShorthand() {
        assertValue("50%", equals: "0.5000")
        assertValue("200*15%", equals: "30.0000")
        assertValue("50%+10", equals: "10.5000")
    }

    func testModuloStillWorks() {
        assertValue("10%3", equals: "1.0000")
    }

    private func assertValue(_ expression: String, equals expected: String, file: StaticString = #filePath, line: UInt = #line) {
        let result = CalcCommand.evaluate(expression)

        switch result {
        case let .value(value):
            XCTAssertEqual(value, expected, file: file, line: line)
        case let .error(message):
            XCTFail("Expression '\(expression)' expected value '\(expected)', got error: \(message)", file: file, line: line)
        }
    }
}
