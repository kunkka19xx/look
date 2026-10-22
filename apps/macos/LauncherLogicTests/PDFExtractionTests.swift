import CoreText
import PDFKit
import XCTest

@testable import LauncherLogic

final class ExtractedTextQualityTests: XCTestCase {
    func testLigaturesAndHyphenationAreUndone() {
        // PDFs emit ligature glyphs and break words across lines; both are
        // mechanical artifacts of layout, safe to reverse.
        XCTAssertEqual(ExtractedTextQuality.normalized("the ﬁle is ﬂat"), "the file is flat")
        XCTAssertEqual(ExtractedTextQuality.normalized("exam-\nple"), "example")
        // A hyphen NOT at a line break is part of the word.
        XCTAssertEqual(ExtractedTextQuality.normalized("well-known"), "well-known")
    }

    func testPageFurnitureCollapsesButParagraphsSurvive() {
        XCTAssertEqual(ExtractedTextQuality.normalized("a\n\n\n\n\nb"), "a\n\nb")
        XCTAssertEqual(ExtractedTextQuality.normalized("a\n\nb"), "a\n\nb")
        XCTAssertEqual(ExtractedTextQuality.normalized("  padded  "), "padded")
    }

    func testRealProseIsUsableInAnyScript() {
        for text in [
            "The quick brown fox jumps over the lazy dog.",
            "Điều này hoàn toàn hợp lệ.",
            "これは正当な日本語のテキストです。",
            "这是一段合法的中文文本。",
            "def main():\n\treturn 1 + 2  # code counts too",
        ] {
            XCTAssertTrue(ExtractedTextQuality.isUsable(text), text)
        }
    }

    func testMisDecodedTextIsRefused() {
        // A PDF with no character map yields private-use scalars or U+FFFD.
        let privateUse = String(repeating: "\u{E000}\u{E001}\u{E002}", count: 40)
        XCTAssertFalse(ExtractedTextQuality.isUsable(privateUse))
        XCTAssertFalse(ExtractedTextQuality.isUsable(String(repeating: "\u{FFFD}", count: 50)))
        XCTAssertFalse(ExtractedTextQuality.isUsable(""))
    }

    func testAStrayBadCharacterDoesNotCondemnARealDocument() {
        // The gate must only catch text that is PROVABLY broken; one control
        // character in a page of prose is normal.
        let mostlyFine = String(repeating: "Real sentence here. ", count: 20) + "\u{FFFD}"
        XCTAssertTrue(ExtractedTextQuality.isUsable(mostlyFine))
    }
}

final class PDFExtractionTests: XCTestCase {
    /// A one-page PDF with real, selectable text drawn through CoreText, so the
    /// test exercises PDFKit's actual extraction rather than a stub.
    private func writePDF(text: String, name: String = UUID().uuidString) throws -> String {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("pdf-\(name).pdf")
        var box = CGRect(x: 0, y: 0, width: 612, height: 792)
        guard let context = CGContext(url as CFURL, mediaBox: &box, nil) else {
            throw XCTSkip("no PDF context")
        }
        context.beginPDFPage(nil)
        let attributed = NSAttributedString(
            string: text,
            attributes: [.font: CTFontCreateWithName("Helvetica" as CFString, 14, nil)])
        let line = CTLineCreateWithAttributedString(attributed)
        context.textPosition = CGPoint(x: 40, y: 700)
        CTLineDraw(line, context)
        context.endPDFPage()
        context.closePDF()
        addTeardownBlock { try? FileManager.default.removeItem(at: url) }
        return url.path
    }

    func testAPDFIsRecognizedAndRead() throws {
        let path = try writePDF(text: "Quarterly revenue landed at 1.2M.")
        XCTAssertTrue(TextExtraction.isPDF(path: path))
        let extracted = try XCTUnwrap(try? TextExtraction.extract(path: path).get())
        XCTAssertTrue(extracted.text.contains("Quarterly revenue"), extracted.text)
        XCTAssertFalse(extracted.truncated)
    }

    /// A PDF with no text layer is a SCAN, not an empty file, and the message
    /// has to say so or the user thinks look is broken instead of reaching for
    /// OCR.
    func testAPageWithNoTextReportsNoTextLayer() throws {
        let path = try writePDF(text: "")
        XCTAssertEqual(TextExtraction.extract(path: path), .failure(.noTextLayer))
    }

    func testAPDFPastTheCapIsReportedAsTruncated() throws {
        let path = try writePDF(text: String(repeating: "alpha beta ", count: 40))
        let extracted = try XCTUnwrap(try? TextExtraction.extract(path: path, cap: 60).get())
        XCTAssertTrue(extracted.truncated)
        XCTAssertLessThanOrEqual(extracted.text.count, 60)
    }

    func testAFileThatIsNotReallyAPDFFailsCleanly() throws {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("fake-\(UUID().uuidString).pdf")
        try Data("this is not a pdf".utf8).write(to: url)
        addTeardownBlock { try? FileManager.default.removeItem(at: url) }
        XCTAssertEqual(TextExtraction.extract(path: url.path), .failure(.unreadable))
    }
}
