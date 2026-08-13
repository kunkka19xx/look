import XCTest

@testable import LauncherLogic

final class TextOpSourceTests: XCTestCase {
    func testResolvePrefersASinglePickedFile() {
        XCTAssertEqual(TextOpSource.resolve(pickedFilePaths: []), .clipboard)
        XCTAssertEqual(
            TextOpSource.resolve(pickedFilePaths: ["/tmp/notes.md"]),
            .file(path: "/tmp/notes.md"))
        // Ambiguity is reported, never guessed at.
        XCTAssertEqual(
            TextOpSource.resolve(pickedFilePaths: ["/tmp/a.txt", "/tmp/b.txt"]),
            .ambiguous(count: 2))
    }

    func testDeclaresTextAcceptsTextAndExtensionlessFiles() {
        for path in ["/tmp/a.txt", "/tmp/a.md", "/tmp/a.swift", "/tmp/a.json", "/tmp/Makefile"] {
            XCTAssertTrue(TextExtraction.declaresText(path: path), path)
        }
        for path in ["/tmp/a.png", "/tmp/a.zip", "/tmp/a.mp4"] {
            XCTAssertFalse(TextExtraction.declaresText(path: path), path)
        }
    }

    private func write(_ contents: Data, ext: String) throws -> String {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("textop-\(UUID().uuidString).\(ext)")
        try contents.write(to: url)
        addTeardownBlock { try? FileManager.default.removeItem(at: url) }
        return url.path
    }

    func testExtractReadsTextFiles() throws {
        let path = try write(Data("hello world".utf8), ext: "txt")
        let extracted = try XCTUnwrap(try? TextExtraction.extract(path: path).get())
        XCTAssertEqual(extracted.text, "hello world")
        XCTAssertFalse(extracted.truncated)
    }

    func testExtractFlagsTruncationInsteadOfSilentlyShortening() throws {
        let path = try write(Data(String(repeating: "a", count: 500).utf8), ext: "txt")
        let extracted = try XCTUnwrap(try? TextExtraction.extract(path: path, cap: 100).get())
        XCTAssertEqual(extracted.text.count, 100)
        XCTAssertTrue(extracted.truncated)

        // A file exactly at the cap is complete, not truncated.
        let exact = try write(Data(String(repeating: "b", count: 100).utf8), ext: "txt")
        let whole = try XCTUnwrap(try? TextExtraction.extract(path: exact, cap: 100).get())
        XCTAssertFalse(whole.truncated)
    }

    func testTruncationDoesNotSplitAMultiByteCharacter() throws {
        // Each "é" is 2 bytes, so a 5-byte cap lands mid-character.
        let path = try write(Data(String(repeating: "é", count: 10).utf8), ext: "txt")
        let extracted = try XCTUnwrap(try? TextExtraction.extract(path: path, cap: 5).get())
        XCTAssertEqual(extracted.text, "éé")
        XCTAssertTrue(extracted.truncated)
    }

    func testExtractRejectsWhatItCannotSummarize() throws {
        let binary = try write(Data([0xFF, 0xD8, 0xFF, 0xE0, 0x00]), ext: "png")
        XCTAssertEqual(TextExtraction.extract(path: binary), .failure(.notText))

        // No extension to go on, so the decode decides.
        let disguised = try write(Data([0xFF, 0xFE, 0xFD, 0xFC]), ext: "")
        XCTAssertEqual(TextExtraction.extract(path: disguised), .failure(.notText))

        let blank = try write(Data("   \n".utf8), ext: "txt")
        XCTAssertEqual(TextExtraction.extract(path: blank), .failure(.empty))

        XCTAssertEqual(TextExtraction.extract(path: "/tmp/does-not-exist.txt"), .failure(.unreadable))
    }
}
