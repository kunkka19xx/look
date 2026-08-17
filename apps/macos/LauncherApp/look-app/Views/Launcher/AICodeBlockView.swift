import AppKit
import SwiftUI

/// A code block in an AI answer: syntax-highlighted via the existing
/// `SyntaxHighlighter` (language from the markdown fence tag) with a hover-free
/// copy button that flashes a checkmark.
struct AICodeBlockView: View {
    let code: String
    let language: String?
    let themeStore: ThemeStore


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

    var body: some View {
        ZStack(alignment: .topTrailing) {
            Text(highlighted)
                .font(.system(size: CGFloat(themeStore.settings.fontSize - 2), design: .monospaced))
                .foregroundStyle(themeStore.fontColor())
                .textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(8)
                .padding(.trailing, 22)  // room for the copy button
                // Scaled with the panel material: a flat darkening plate at full
                // weight muddies Liquid Glass (see ThemeStore.surfaceFill).
                .background(
                    Color.black.opacity(themeStore.surfaceOpacity(0.18)),
                    in: RoundedRectangle(cornerRadius: 8, style: .continuous))

            AnswerCopyButton(text: code, themeStore: themeStore, helpLabel: "Copy code")
                .padding(6)
        }
    }
}
