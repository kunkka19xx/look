import Foundation

/// What the panel renders of a `preview` command's output.
///
/// The capture bounds the command at 256KB, which is far more than a view should
/// hand to a single Text: the panel would spend its layout on output nobody
/// scrolls to. The head is what answers "what is this row", and saying how much
/// was left out keeps the panel honest about being a preview.
enum PreviewText {
    static let visibleLines = 200

    static func visible(_ text: String) -> (text: String, dropped: Int) {
        let lines = text.split(separator: "\n", omittingEmptySubsequences: false)
        guard lines.count > visibleLines else { return (text, 0) }
        return (
            lines.prefix(visibleLines).joined(separator: "\n"),
            lines.count - visibleLines
        )
    }
}
