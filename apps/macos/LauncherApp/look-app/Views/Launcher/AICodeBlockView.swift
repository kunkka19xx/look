import AppKit
import SwiftUI

/// A code block in an AI answer: syntax-highlighted via the existing
/// `SyntaxHighlighter` (language from the markdown fence tag) with a hover-free
/// copy button that flashes a checkmark.
struct AICodeBlockView: View {
    let code: String
    let language: String?
    let themeStore: ThemeStore
    /// When set, the block never wraps: it is at most this many lines tall and
    /// scrolls in both directions instead, so one long command cannot push the
    /// rest of a panel off the screen. An AI answer leaves it nil and wraps like
    /// the prose around it.
    var maxVisibleLines: Int? = nil


    /// Fence tags that don't match the extensions `SyntaxHighlighter` keys on.
    private static let tagToExtension: [String: String] = [
        "python": "py", "rust": "rs", "javascript": "js", "typescript": "ts",
        "shell": "sh", "bash": "sh", "zsh": "sh", "console": "sh",
        "c++": "cpp", "objective-c": "m", "objc": "m", "golang": "go",
        "ruby": "rb", "markdown": "md", "yaml": "yml", "kotlin": "kt",
    ]

    private var highlighted: AttributedString {
        guard let language else { return AttributedString(code) }
        let ext = Self.tagToExtension[language] ?? language
        return AttributedString(SyntaxHighlighter.highlight(code, path: "snippet.\(ext)"))
    }

    private var codeText: some View {
        Text(highlighted)
            .font(.system(size: CGFloat(themeStore.settings.fontSize - 2), design: .monospaced))
            .foregroundStyle(themeStore.fontColor())
            .textSelection(.enabled)
    }

    /// One line of the block's own font, measured rather than guessed: the size
    /// follows the user's font-size setting.
    private var lineHeight: CGFloat {
        let font = NSFont.monospacedSystemFont(
            ofSize: CGFloat(themeStore.settings.fontSize - 2), weight: .regular)
        return ceil(font.ascender - font.descender + font.leading)
    }

    /// As tall as the code needs, capped. Sized to the content rather than
    /// pinned to the cap, so a one-line command does not sit in a two-line box.
    private var scrollHeight: CGFloat {
        let lines = max(1, code.split(separator: "\n", omittingEmptySubsequences: false).count)
        return lineHeight * CGFloat(min(lines, maxVisibleLines ?? lines))
    }

    @ViewBuilder
    private var content: some View {
        if maxVisibleLines != nil {
            ScrollView([.horizontal, .vertical]) {
                // Never wrapped: a long command reads better scrolled than
                // folded into six lines, and the panel keeps its shape.
                codeText.fixedSize()
            }
            // No bars: they would sit on top of the code in a box this small,
            // and the content already looks scrollable when it is cut off.
            .scrollIndicators(.hidden)
            .frame(height: scrollHeight)
            .frame(maxWidth: .infinity, alignment: .leading)
        } else {
            codeText
                .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    var body: some View {
        ZStack(alignment: .topTrailing) {
            content
                .padding(8)
                .padding(.trailing, 22)  // room for the copy button
                // Scaled with the panel material: a flat darkening plate at full
                // weight muddies Liquid Glass (see ThemeStore.surfaceFill).
                .background(
                    Color.black.opacity(themeStore.surfaceOpacity(0.18)),
                    in: RoundedRectangle(cornerRadius: themeStore.controlRadius, style: .continuous))

            AnswerCopyButton(text: code, themeStore: themeStore, helpLabel: "Copy code")
                .padding(6)
        }
    }
}
