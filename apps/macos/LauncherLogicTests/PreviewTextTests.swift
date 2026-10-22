import XCTest
@testable import LauncherLogic

/// How much of a `preview` command's output the panel renders.
final class PreviewTextTests: XCTestCase {
    private func lines(_ count: Int) -> String {
        (0..<count).map { "line \($0)" }.joined(separator: "\n")
    }

    func testShortOutputIsPassedThroughUntouched() {
        // Byte-identical, not merely equal in content: a preview that fits must
        // not gain or lose a trailing newline on the way to the panel.
        let text = "one\ntwo\n"
        let shown = PreviewText.visible(text)
        XCTAssertEqual(shown.text, text)
        XCTAssertEqual(shown.dropped, 0)
    }

    func testOutputExactlyAtTheCapIsNotTruncated() {
        let shown = PreviewText.visible(lines(PreviewText.visibleLines))
        XCTAssertEqual(shown.dropped, 0)
    }

    func testOutputPastTheCapKeepsTheHeadAndCountsTheRest() {
        // The head is what answers "what is this row"; the count is what keeps
        // the panel honest about being a preview.
        let shown = PreviewText.visible(lines(PreviewText.visibleLines + 37))
        XCTAssertEqual(shown.dropped, 37)
        XCTAssertEqual(
            shown.text.split(separator: "\n", omittingEmptySubsequences: false).count,
            PreviewText.visibleLines)
        XCTAssertTrue(shown.text.hasPrefix("line 0\n"))
        XCTAssertTrue(shown.text.hasSuffix("line \(PreviewText.visibleLines - 1)"))
    }

    func testEmptyOutputIsNotAThousandBlankLines() {
        let shown = PreviewText.visible("")
        XCTAssertEqual(shown.text, "")
        XCTAssertEqual(shown.dropped, 0)
    }

    /// The case the cap exists for: a command that prints a whole file.
    func testAVeryLongOutputIsBoundedRatherThanRendered() {
        let shown = PreviewText.visible(lines(50_000))
        XCTAssertEqual(shown.dropped, 50_000 - PreviewText.visibleLines)
        XCTAssertLessThan(shown.text.count, 4_000)
    }
}
