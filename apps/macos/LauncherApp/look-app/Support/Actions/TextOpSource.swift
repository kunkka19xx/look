import Foundation
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
    static func resolve(pickedFilePaths: [String]) -> TextOpSource {
        switch pickedFilePaths.count {
        case 0: .clipboard
        case 1: .file(path: pickedFilePaths[0])
        default: .ambiguous(count: pickedFilePaths.count)
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
    }

    /// True when the extension declares a text type. Extensionless files are
    /// allowed through here and settled by decoding instead, so `.zshrc` and a
    /// `Makefile` still work.
    static func declaresText(path: String) -> Bool {
        let ext = (path as NSString).pathExtension
        guard !ext.isEmpty else { return true }
        guard let type = UTType(filenameExtension: ext) else { return true }
        return type.conforms(to: .text)
    }

    static func extract(path: String, cap: Int = defaultCap) -> Result<Extracted, Failure> {
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
