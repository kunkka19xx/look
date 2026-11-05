import XCTest

@testable import LauncherLogic

/// The rendered-page base derived from a catalog that serves raw bytes. This
/// is why a card has a README link at all: the published `index.json` declares
/// no `web`, so without the derivation every card renders none.
final class LookbookPagesTests: XCTestCase {
    func testDerivesTheBlobBaseFromARawBase() {
        XCTAssertEqual(
            LookbookPages.base(
                forRaw: "https://raw.githubusercontent.com/kunkka19xx/lookbook/main/"),
            "https://github.com/kunkka19xx/lookbook/blob/main/"
        )
    }

    func testKeepsADirectoryInsideTheRepo() {
        XCTAssertEqual(
            LookbookPages.base(forRaw: "https://raw.githubusercontent.com/owner/repo/v1/catalog/"),
            "https://github.com/owner/repo/blob/v1/catalog/"
        )
    }

    /// A checkout, or any host whose layout this does not know. Empty rather
    /// than a guess, which would be a link to a 404.
    func testLeavesAnythingElseAlone() {
        XCTAssertEqual(LookbookPages.base(forRaw: "/Users/me/lookbook/"), "")
        XCTAssertEqual(LookbookPages.base(forRaw: "https://example.com/lookbook/"), "")
        XCTAssertEqual(
            LookbookPages.base(forRaw: "https://raw.githubusercontent.com/owner/repo/"), "",
            "no ref to put in the path"
        )
    }
}
