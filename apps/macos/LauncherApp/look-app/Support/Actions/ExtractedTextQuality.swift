import Foundation

/// Cleanup and a usability verdict for text pulled out of a document.
///
/// PDFs are the reason this exists. A PDF is a drawing format, not a text
/// format: the characters on screen are glyph ids, and recovering letters from
/// them depends on a mapping the producer may not have embedded. When it is
/// missing, extraction still "succeeds" and returns confident nonsense -
/// private-use scalars, replacement characters, or runs of control codes.
/// Handing that to a model produces a fluent summary of garbage, which is a
/// worse failure than refusing, because nothing about the answer looks wrong.
///
/// So extraction is followed by a verdict, and the verdict is conservative: it
/// only rejects text that is provably broken, never text that is merely
/// unusual. Deciding "this prose looks low quality" is not something a
/// character histogram can do honestly.
nonisolated enum ExtractedTextQuality {
    /// Share of broken scalars above which the text is refused. Deliberately
    /// low: a real document has essentially none, while a mis-decoded one is
    /// full of them, so anything in between is rare and better refused.
    static let brokenScalarLimit = 0.02

    /// Tidies the artifacts of PDF text layout, which are mechanical and safe
    /// to undo. Anything requiring judgement (column order, tables) is left
    /// alone - a wrong guess there would silently reorder the user's document.
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

    /// Unicode leaves these blocks permanently undefined so fonts can number
    /// their own glyphs in them. A PDF whose font has no character map leaks
    /// those raw glyph numbers straight through, so text landing here is a
    /// font's internal numbering rather than letters. Three blocks: the main
    /// one, plus the two supplementary planes.
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
