import XCTest
@testable import LauncherLogic

final class ChatMarkdownTests: XCTestCase {
    func testPlainTextIsOneSegment() {
        XCTAssertEqual(
            ChatMarkdown.segments(from: "hello world"),
            [.text("hello world")])
    }

    func testFencedCodeSplitsIntoThreeSegments() {
        let raw = "Before\n```swift\nlet x = 1\n```\nAfter"
        XCTAssertEqual(
            ChatMarkdown.segments(from: raw),
            [.text("Before"), .code("let x = 1"), .text("After")])
    }

    func testUnclosedFenceRendersAsCode() {
        // Mid-stream: the closing fence hasn't arrived yet.
        let raw = "Intro\n```\nfn main() {"
        XCTAssertEqual(
            ChatMarkdown.segments(from: raw),
            [.text("Intro"), .code("fn main() {")])
    }

    func testMultipleCodeBlocks() {
        let raw = "```\na\n```\nmiddle\n```\nb\n```"
        XCTAssertEqual(
            ChatMarkdown.segments(from: raw),
            [.code("a"), .text("middle"), .code("b")])
    }

    func testEmptySegmentsAreDropped() {
        XCTAssertEqual(ChatMarkdown.segments(from: "```\n```"), [])
        XCTAssertEqual(ChatMarkdown.segments(from: "\n\n"), [])
    }
}
