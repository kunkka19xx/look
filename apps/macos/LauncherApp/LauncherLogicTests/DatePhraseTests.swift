import XCTest
@testable import LauncherLogic

/// The shell side of the date seam. NSDataDetector itself is Apple's and not
/// worth re-testing; what matters here is the glue we own - the shorthand we
/// feed it (it does not know "tmr" or "@"), and that non-dates stay nil so a
/// title is never mistaken for a time.
final class DatePhraseTests: XCTestCase {
    /// A fixed instant so a resolved day can be compared without today drifting.
    private let now = Date(timeIntervalSince1970: 1_754_388_000)

    func testShorthandExpandsToWordsTheDetectorKnows() {
        XCTAssertEqual(DatePhrase.normalizeShorthand("tmr 3pm"), "tomorrow 3pm")
        XCTAssertEqual(DatePhrase.normalizeShorthand("2moro"), "tomorrow")
        XCTAssertEqual(DatePhrase.normalizeShorthand("tonite"), "tonight")
    }

    func testAtSignBecomesAtSoTheDetectorSeesTheTime() {
        // "sunday @ 5pm" trips the detector; "sunday at 5pm" does not.
        XCTAssertEqual(DatePhrase.normalizeShorthand("sunday @ 5pm"), "sunday at 5pm")
        // Glued form too: "@3pm" must not survive as one token.
        XCTAssertEqual(DatePhrase.normalizeShorthand("lunch @3pm"), "lunch at 3pm")
    }

    func testShorthandLeavesOrdinaryWordsAlone() {
        XCTAssertEqual(
            DatePhrase.normalizeShorthand("email bob@example.com"),
            "email bob@example.com")
    }

    func testResolvesAShorthandPhraseToARealDate() throws {
        // The point of the shorthand: this resolves only because "tmr" was
        // expanded before the detector saw it.
        let resolved = try XCTUnwrap(DatePhrase.resolve("tmr 9am", now: now))
        let calendar = Calendar.current
        XCTAssertEqual(calendar.component(.hour, from: resolved), 9)
    }

    func testNonDatesStayNil() {
        // A bare title must never resolve, or every "add X" would get a time.
        XCTAssertNil(DatePhrase.resolve("dentist", now: now))
        XCTAssertNil(DatePhrase.resolve("", now: now))
        XCTAssertNil(DatePhrase.resolve("   ", now: now))
    }
}
