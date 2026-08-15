import Foundation

/// Cleanup and a usability verdict for text pulled out of a document.
///
/// A PDF without an embedded character map still "extracts" - into glyph ids,
/// not letters - and a model will summarize that gibberish fluently. So the
/// verdict rejects only provably broken text, never merely unusual text.
nonisolated enum ExtractedTextQuality {
    /// A real document has essentially none; a mis-decoded one is full of
    /// them, so anything in between is rare and better refused.
    static let brokenScalarLimit = 0.02

    /// Undoes the mechanical artifacts of PDF layout only. Column order and
    /// tables are left alone: a wrong guess reorders the user's document.
    static func normalized(_ text: String) -> String {
        // NFKC folds the ligature glyphs PDFs are full of: "ﬁ" -> "fi".
        var out = text.precomposedStringWithCompatibilityMapping
        // Words broken across lines by hyphenation: "exam-\nple" -> "example".
        out = out.replacingOccurrences(
            of: "([\\p{L}])-\\n([\\p{L}])",
            with: "$1$2",
            options: .regularExpression)
        // Page furniture leaves long runs of blank lines behind.
        out = out.replacingOccurrences(
            of: "\\n{3,}", with: "\n\n", options: .regularExpression)
        return out.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    /// The decoder gave up on these bytes and said so.
    private static let replacementCharacter: UInt32 = 0xFFFD

    /// Undefined by design, so fonts number their own glyphs here. Text
    /// landing in these blocks is glyph ids, not letters.
    private static let privateUseAreas: [ClosedRange<UInt32>] = [
        0xE000...0xF8FF,
        0xF0000...0xFFFFD,
        0x100000...0x10FFFD,
    ]

    /// Teletype-era control codes (0x00-0x1F) have no place in a document -
    /// EXCEPT the three that are ordinary text.
    private static let tab: UInt32 = 0x09
    private static let newline: UInt32 = 0x0A
    private static let carriageReturn: UInt32 = 0x0D

    /// Whether a scalar means the decode FAILED, as opposed to the text merely
    /// being unusual. Nothing here is a judgement about language or style.
    static func isBroken(_ scalar: Unicode.Scalar) -> Bool {
        let value = scalar.value
        if value == replacementCharacter { return true }
        if privateUseAreas.contains(where: { $0.contains(value) }) { return true }
        let isControlCode = value <= 0x1F
        let isTextWhitespace = value == tab || value == newline || value == carriageReturn
        return isControlCode && !isTextWhitespace
    }

    /// True when the text is usable. False ONLY for provably broken output, so
    /// a legitimate document in any script passes: this counts broken scalars
    /// rather than guessing at words, which would reject Chinese, Japanese, and
    /// Thai for having no spaces.
    static func isUsable(_ text: String) -> Bool {
        let scalars = text.unicodeScalars
        guard !scalars.isEmpty else { return false }
        let broken = scalars.reduce(into: 0) { count, scalar in
            if isBroken(scalar) { count += 1 }
        }
        return Double(broken) / Double(scalars.count) <= brokenScalarLimit
    }
}
