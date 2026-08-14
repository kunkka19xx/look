import Foundation
import PDFKit
import UniformTypeIdentifiers

/// Where a text op reads its input. The clipboard was the only source when
/// text ops shipped; a picked file is the second, so "summarize" can act on a
/// file the user pointed at instead of whatever they last copied.
///
/// The model never chooses this. It emits an instruction and nothing else, and
/// the target comes from UI state - the same split that keeps the calendar
/// tools reliable.
nonisolated enum TextOpSource: Equatable {
    case clipboard
    case file(path: String)
    /// More than one file is picked. Ambiguity is reported, never guessed at,
    /// mirroring how target resolution handles an ambiguous match.
    case ambiguous(count: Int)

    /// Picking a file writes its PATH to the pasteboard (see
    /// `writePickedToPasteboard`), so without this, a text op after a pick
    /// would transform the path string rather than the file.
    ///
    /// `@`-mentions win over picks: a mention is aimed at this turn, while a
    /// pick may be left over from whatever the user was doing in the main bar.
    static func resolve(mentioned: [String] = [], pickedFilePaths: [String] = [])
        -> TextOpSource
    {
        let chosen = mentioned.isEmpty ? pickedFilePaths : mentioned
        switch chosen.count {
        case 0: return .clipboard
        case 1: return .file(path: chosen[0])
        default: return .ambiguous(count: chosen.count)
        }
    }
}

/// Reads text out of a file for a text op. Deliberately narrow: text-ish files
/// only, capped, no format conversion. PDF and friends need real extraction and
/// anything past the cap needs chunk-and-retrieve, which is a separate build.
nonisolated enum TextExtraction {
    /// Roughly 30k tokens of English, comfortably past what a local model will
    /// take, so the cap is the model's limit rather than ours.
    static let defaultCap = 128 * 1024

    struct Extracted: Equatable {
        let text: String
        /// The file was longer than the cap and this is only its head. The
        /// caller MUST say so rather than pass off a partial read as the whole
        /// file.
        let truncated: Bool
    }

    enum Failure: Error, Equatable {
        case unreadable
        case notText
        case empty
        /// Password-protected: the contents cannot be reached at all.
        case locked
        /// A PDF of page IMAGES with no text layer - a scan or a photo export.
        /// Distinct from `empty`, because the file is far from empty; there is
        /// simply nothing to read without OCR.
        case noTextLayer
        /// Text came out, but it decoded to junk (see `ExtractedTextQuality`).
        /// Refused on purpose: a model will summarize gibberish fluently, and
        /// that answer looks exactly as trustworthy as a real one.
        case garbled
    }

    static func isPDF(path: String) -> Bool {
        UTType(filenameExtension: (path as NSString).pathExtension)?.conforms(to: .pdf) == true
    }

    /// Text out of a PDF, or a named reason there is none. A PDF is a DRAWING
    /// format: extraction depends on a character map the producer may have
    /// omitted, so "it returned a string" is not the same as "it worked" - see
    /// `ExtractedTextQuality`.
    private static func extractPDF(path: String, cap: Int) -> Result<Extracted, Failure> {
        guard let document = PDFDocument(url: URL(fileURLWithPath: path)) else {
            return .failure(.unreadable)
        }
        guard !document.isLocked else { return .failure(.locked) }

        // Page by page, stopping at the cap: a 400-page PDF must not be read
        // into memory in full just to summarize its opening.
        var text = ""
        var truncated = false
        for index in 0..<document.pageCount {
            guard let page = document.page(at: index), let pageText = page.string else { continue }
            if text.count + pageText.count > cap {
                text += String(pageText.prefix(max(0, cap - text.count)))
                truncated = true
                break
            }
            text += pageText
            if index < document.pageCount - 1 { text += "\n\n" }
        }

        let cleaned = ExtractedTextQuality.normalized(text)
        // A scan is not an empty file, and saying so is the difference between
        // the user reaching for OCR and thinking look is broken.
        guard !cleaned.isEmpty else { return .failure(.noTextLayer) }
        guard ExtractedTextQuality.isUsable(cleaned) else { return .failure(.garbled) }
        return .success(Extracted(text: cleaned, truncated: truncated))
    }

    /// True when the extension declares a text type, or when macOS has no
    /// opinion about it. "No opinion" covers three cases that must all fall
    /// through to the DECODE, which is the only thing that actually knows:
    ///
    /// - no extension at all (`Makefile`, `.zshrc`),
    /// - an extension with no registered type,
    /// - an extension macOS answers with a DYNAMIC type (`dyn.ah62d4rv4ge80s52`).
    ///
    /// That last one is why `main.go` was refused: an unregistered extension
    /// gets a synthesized `dyn.*` type rather than nil, and a dynamic type
    /// conforms to nothing - so `.go`, `.rs`, and `.zig` all read as "not
    /// text" while `.swift`, `.py`, `.md`, and `.toml` passed. Which languages
    /// happen to be registered depends on what is installed, so treating a
    /// dynamic type as a verdict would make this vary machine to machine.
    static func declaresText(path: String) -> Bool {
        let ext = (path as NSString).pathExtension
        guard !ext.isEmpty else { return true }
        guard let type = UTType(filenameExtension: ext), !type.isDynamic else { return true }
        return type.conforms(to: .text)
    }

    static func extract(path: String, cap: Int = defaultCap) -> Result<Extracted, Failure> {
        if isPDF(path: path) { return extractPDF(path: path, cap: cap) }
        guard declaresText(path: path) else { return .failure(.notText) }
        guard let handle = FileHandle(forReadingAtPath: path) else { return .failure(.unreadable) }
        defer { try? handle.close() }

        // Read one byte past the cap so "exactly at the cap" is not reported as
        // truncated.
        guard let data = try? handle.read(upToCount: cap + 1) else { return .failure(.unreadable) }
        let truncated = data.count > cap
        let body = truncated ? data.prefix(cap) : data

        var decoded = String(data: body, encoding: .utf8)
        // Only a CAPPED read can split a multi-byte character, so drop up to
        // three trailing bytes to recover it. A complete file that will not
        // decode is simply not text, and trimming it would turn binary into
        // "empty".
        if decoded == nil, truncated {
            var slice = body
            var dropped = 0
            while decoded == nil, dropped < 3, !slice.isEmpty {
                slice = slice.dropLast()
                dropped += 1
                decoded = String(data: slice, encoding: .utf8)
            }
        }
        guard let text = decoded else { return .failure(.notText) }
        guard !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            return .failure(.empty)
        }
        return .success(Extracted(text: text, truncated: truncated))
    }
}
