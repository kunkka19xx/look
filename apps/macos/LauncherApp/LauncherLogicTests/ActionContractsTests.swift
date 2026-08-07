import XCTest
@testable import LauncherLogic

/// The Swift-side remains of the action layer: the NSDataDetector date seam's
/// shorthand normalization. Everything else (parser, matcher, tools, wire
/// format) lives in `core/ai` with its own cargo tests.
final class ActionContractsTests: XCTestCase {
    func testNormalizesTomorrowShorthand() {
        XCTAssertEqual(DatePhrase.normalizeShorthand("1pm tmr"), "1pm tomorrow")
        XCTAssertEqual(DatePhrase.normalizeShorthand("lunch 12pm tmrw"), "lunch 12pm tomorrow")
        XCTAssertEqual(DatePhrase.normalizeShorthand("call TDY"), "call today")
    }

    func testNormalizeMapsAtSignToAt() {
        XCTAssertEqual(DatePhrase.normalizeShorthand("sunday @ 5pm"), "sunday at 5pm")
        XCTAssertEqual(DatePhrase.normalizeShorthand("sunday @5pm"), "sunday at 5pm")
    }

    func testNormalizeLeavesNormalTextAlone() {
        XCTAssertEqual(DatePhrase.normalizeShorthand("1pm tomorrow"), "1pm tomorrow")
        XCTAssertEqual(DatePhrase.normalizeShorthand("dentist at 3"), "dentist at 3")
    }
}
