import XCTest
@testable import LauncherLogic

/// The macOS codename mapping is the fix for issue #519: getMacOSName returned
/// "Sequoia" for every major >= 15, so macOS 26 Tahoe and anything newer were
/// mislabeled. The mapping must name the known majors explicitly and fall back
/// to "macOS <major>" for any unknown future major rather than guessing.
final class MacOSNameTests: XCTestCase {
    func testSectionKnownNames() {
        XCTAssertEqual(MacOSName.forMajor(15), "Sequoia")
        XCTAssertEqual(MacOSName.forMajor(26), "Tahoe")
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

    func testUnknownNewerMajorShowsRealNumberNotACodename() {
        // macOS 27 and the skipped 16-25 range have no codename yet.
        XCTAssertEqual(MacOSName.forMajor(27), "macOS 27")
        XCTAssertEqual(MacOSName.forMajor(16), "macOS 16")
        XCTAssertEqual(MacOSName.forMajor(20), "macOS 20")
    }
}
