import XCTest
@testable import LauncherLogic

/// The macOS codename mapping is the fix for issue #519: getMacOSName returned
/// "Sequoia" for every major >= 15, so macOS 26 Tahoe and anything newer were
/// mislabeled. The mapping must name the known majors explicitly and fall back
/// to plain "macOS" for any unknown future major rather than guessing; the
/// caller appends the version number.
final class MacOSNameTests: XCTestCase {
    func testSectionKnownNames() {
        XCTAssertEqual(MacOSName.forMajor(15), "Sequoia")
        XCTAssertEqual(MacOSName.forMajor(26), "Tahoe")
        XCTAssertEqual(MacOSName.forMajor(27), "Golden Gate")
    }

    func testKeepsOlderNames() {
        XCTAssertEqual(MacOSName.forMajor(14), "Sonoma")
        XCTAssertEqual(MacOSName.forMajor(13), "Ventura")
        XCTAssertEqual(MacOSName.forMajor(12), "Monterey")
        XCTAssertEqual(MacOSName.forMajor(11), "Big Sur")
    }

    func testPreElevenFallsBackToBareMacOS() {
        XCTAssertEqual(MacOSName.forMajor(10), "macOS")
        XCTAssertEqual(MacOSName.forMajor(0), "macOS")
    }

    func testUnknownNewerMajorFallsBackToBareMacOS() {
        // The skipped 16-25 range and releases after 27 have no codename here.
        XCTAssertEqual(MacOSName.forMajor(28), "macOS")
        XCTAssertEqual(MacOSName.forMajor(16), "macOS")
        XCTAssertEqual(MacOSName.forMajor(20), "macOS")
    }
}
