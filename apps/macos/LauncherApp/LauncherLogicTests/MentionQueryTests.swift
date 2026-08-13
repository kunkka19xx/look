import XCTest

@testable import LauncherLogic

final class MentionQueryTests: XCTestCase {
    /// Caret at the end of the text, the common case while typing.
    private func active(_ text: String) -> MentionQuery.Active? {
        MentionQuery.active(in: text, caret: text.count)
    }

    func testTokenIsWhatFollowsTheAtSign() {
        XCTAssertEqual(active("summarize @rep")?.token, "rep")
        XCTAssertEqual(active("@notes")?.token, "notes")
        XCTAssertEqual(active("compare @a.md")?.token, "a.md")
    }

    func testTheDateFormIsNeverAMention() {
        // `>add lunch @ 1pm`: a space after `@` ends it before it starts.
        XCTAssertNil(active("add lunch @ 1pm"))
        XCTAssertNil(active("add lunch @"))
        // "@1pm" DOES open a search, and that is fine: nothing is attached
        // unless the user picks, so the text stays a date phrase.
        XCTAssertEqual(active("add lunch @1pm")?.token, "1pm")
    }

    func testAnAtSignInsideAWordIsNotAMention() {
        XCTAssertNil(active("mail me at foo@bar.com"))
        XCTAssertNil(active("user@host"))
    }

    func testCaretPositionDecidesTheToken() {
        let text = "see @doc and more"
        // Caret right after "@doc".
        XCTAssertEqual(MentionQuery.active(in: text, caret: 8)?.token, "doc")
        // Caret past the following space: the mention is finished, not active.
        XCTAssertNil(MentionQuery.active(in: text, caret: text.count))
        // Caret mid-token gives only what is left of it.
        XCTAssertEqual(MentionQuery.active(in: text, caret: 7)?.token, "do")
    }

    func testConsumeRemovesTheTokenSoTheQueryStaysClean() {
        let text = "summarize @rep"
        let found = try! XCTUnwrap(active(text))
        let (cleaned, caret) = MentionQuery.consume(text, found)
        XCTAssertEqual(cleaned, "summarize ")
        XCTAssertEqual(caret, 10)

        let midSentence = "compare @a.md with the other"
        let midFound = try! XCTUnwrap(MentionQuery.active(in: midSentence, caret: 13))
        XCTAssertEqual(MentionQuery.consume(midSentence, midFound).text, "compare  with the other")
    }

    func testMultiByteTextDoesNotMiscount() {
        let text = "tóm tắt @báo"
        XCTAssertEqual(active(text)?.token, "báo")
        let found = try! XCTUnwrap(active(text))
        XCTAssertEqual(MentionQuery.consume(text, found).text, "tóm tắt ")
    }

    func testEmptyAndBoundsAreSafe() {
        XCTAssertNil(MentionQuery.active(in: "", caret: 0))
        XCTAssertNil(MentionQuery.active(in: "abc", caret: 0))
        // A caret past the end clamps rather than disabling mentions: the UI
        // reporting a stale offset must not silently break the popup.
        XCTAssertEqual(MentionQuery.active(in: "@a", caret: 99)?.token, "a")
        XCTAssertNil(MentionQuery.active(in: "@a", caret: -5))
    }
}

final class MentionAttachmentsTests: XCTestCase {
    private func write(_ contents: String, ext: String = "txt") throws -> String {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("mention-\(UUID().uuidString).\(ext)")
        try Data(contents.utf8).write(to: url)
        addTeardownBlock { try? FileManager.default.removeItem(at: url) }
        return url.path
    }

    func testAddIsDedupedAndOrdered() throws {
        let first = try write("alpha")
        let second = try write("beta")
        var attachments = MentionAttachments()
        XCTAssertNil(attachments.add(path: first))
        XCTAssertNil(attachments.add(path: second))
        XCTAssertNil(attachments.add(path: first))
        XCTAssertEqual(attachments.paths, [first, second])
        XCTAssertEqual(attachments.totalCharacters, 9)
    }

    func testAnUnreadableFileIsReportedAndNotAttached() throws {
        var attachments = MentionAttachments()
        XCTAssertEqual(attachments.add(path: "/tmp/nope-\(UUID().uuidString).txt"), .unreadable)
        XCTAssertTrue(attachments.isEmpty)

        let binary = try write("x", ext: "png")
        XCTAssertEqual(attachments.add(path: binary), .notText)
        XCTAssertTrue(attachments.isEmpty)
    }

    func testContextBudgetWarnsBeforeTheModelSilentlyTruncates() throws {
        let context = 16384
        var attachments = MentionAttachments()
        let small = try write(String(repeating: "a", count: 400))
        XCTAssertNil(attachments.add(path: small))
        XCTAssertFalse(attachments.exceedsContext(context))

        // Past three quarters of the largest window a text op will request.
        let threshold = context * 3 / 4
        let big = try write(
            String(
                repeating: "b",
                count: threshold * AIGenerationOptions.charactersPerToken + 100))
        XCTAssertNil(attachments.add(path: big))
        XCTAssertTrue(attachments.exceedsContext(context))
        // A bigger window (a cloud model) is not exceeded by the same files.
        XCTAssertFalse(attachments.exceedsContext(context * 8))
    }

    func testContextBlockLabelsEachFileWithItsPath() throws {
        let path = try write("hello")
        var attachments = MentionAttachments()
        XCTAssertNil(attachments.add(path: path))
        let block = attachments.contextBlock()
        XCTAssertTrue(block.contains("--- \(path)"))
        XCTAssertTrue(block.contains("hello"))
    }

    func testRemove() throws {
        let path = try write("gone")
        var attachments = MentionAttachments()
        XCTAssertNil(attachments.add(path: path))
        attachments.remove(path: path)
        XCTAssertTrue(attachments.isEmpty)
    }
}
