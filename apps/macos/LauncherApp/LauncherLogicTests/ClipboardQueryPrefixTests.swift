import XCTest

@testable import LauncherLogic

final class ClipboardQueryPrefixTests: XCTestCase {
    /// `ci"` begins with `c`, so the two histories are one keystroke apart.
    /// Each prefix must answer only for itself, whichever order a caller checks
    /// them in.
    func testTheTwoClipboardPrefixesDoNotSwallowEachOther() {
        XCTAssertTrue(ClipboardQueryPrefix.image.matches("ci\"logo"))
        XCTAssertFalse(ClipboardQueryPrefix.text.matches("ci\"logo"))
        XCTAssertTrue(ClipboardQueryPrefix.text.matches("c\"logo"))
        XCTAssertFalse(ClipboardQueryPrefix.image.matches("c\"logo"))
    }

    func testSearchTermIsWhateverFollowsThePrefix() {
        XCTAssertEqual(ClipboardQueryPrefix.image.searchTerm(in: "ci\""), "")
        XCTAssertEqual(ClipboardQueryPrefix.image.searchTerm(in: "ci\"logo"), "logo")
        XCTAssertEqual(ClipboardQueryPrefix.image.searchTerm(in: "  ci\"  logo  "), "logo")
        XCTAssertEqual(ClipboardQueryPrefix.text.searchTerm(in: "c\"mail"), "mail")
    }

    /// The prefix is typed, so it is matched case-insensitively. The term is
    /// not: it is searched against content the user copied verbatim.
    func testThePrefixIgnoresCaseButTheTermKeepsIt() {
        XCTAssertEqual(ClipboardQueryPrefix.image.searchTerm(in: "CI\"Logo"), "Logo")
    }

    func testAQueryWithNoClipboardPrefixBelongsToNeither() {
        for query in ["logo", "", "  ", "f\"logo", "\"", "c"] {
            XCTAssertNil(ClipboardQueryPrefix.text.searchTerm(in: query), query)
            XCTAssertNil(ClipboardQueryPrefix.image.searchTerm(in: query), query)
        }
    }
}
