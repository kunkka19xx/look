import XCTest
@testable import LauncherLogic

final class EmojiRecentsTests: XCTestCase {
    private func hit(_ glyph: String) -> EmojiRecents.Entry {
        EmojiRecents.Entry(glyph: glyph, name: glyph)
    }

    func testNewestFirst() {
        let list = EmojiRecents.pushed(EmojiRecents.pushed([], hit("A")), hit("B"))
        XCTAssertEqual(list.map(\.glyph), ["B", "A"])
    }

    func testReuseMovesToFrontWithoutDuplicate() {
        var list: [EmojiRecents.Entry] = []
        for g in ["A", "B", "A"] { list = EmojiRecents.pushed(list, hit(g)) }
        XCTAssertEqual(list.map(\.glyph), ["A", "B"])
    }

    func testCapAtTwentyDropsOldest() {
        var list: [EmojiRecents.Entry] = []
        for i in 0..<21 { list = EmojiRecents.pushed(list, hit("g\(i)")) }
        XCTAssertEqual(EmojiRecents.cap, 20)
        XCTAssertEqual(list.count, 20)
        XCTAssertEqual(list.first?.glyph, "g20")
        XCTAssertEqual(list.last?.glyph, "g1")
    }

    func testRoundTripsThroughData() {
        let data = EmojiRecents.encode([hit("A")])
        XCTAssertEqual(EmojiRecents.decode(data).map(\.glyph), ["A"])
    }

    func testCorruptOrMissingDataYieldsNothing() {
        XCTAssertEqual(EmojiRecents.decode(nil), [])
        XCTAssertEqual(EmojiRecents.decode(Data("{nope".utf8)), [])
    }

    func testDecodeRespectsCap() {
        let many = (0..<30).map { hit("g\($0)") }
        XCTAssertEqual(EmojiRecents.decode(EmojiRecents.encode(many)).count, 20)
    }
}
