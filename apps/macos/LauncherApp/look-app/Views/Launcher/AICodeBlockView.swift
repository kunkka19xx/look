import AppKit
import SwiftUI

/// A code block in an AI answer: syntax-highlighted via the existing
/// `SyntaxHighlighter` (language from the markdown fence tag) with a hover-free
/// copy button that flashes a checkmark.
struct AICodeBlockView: View {
    let code: String
    let language: String?
    let themeStore: ThemeStore

    @State private var copied = false

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
                .background(Color.black.opacity(0.18), in: RoundedRectangle(cornerRadius: 8, style: .continuous))

            Button {
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(code, forType: .string)
                copied = true
                DispatchQueue.main.asyncAfter(deadline: .now() + 1.2) { copied = false }
            } label: {
                Image(systemName: copied ? "checkmark" : "doc.on.doc")
                    .font(.system(size: CGFloat(themeStore.settings.fontSize - 3)))
                    .foregroundStyle(copied ? Color.green.opacity(0.85) : themeStore.mutedTextColor())
            }
            .buttonStyle(.plain)
            .padding(6)
            .help("Copy code")
        }
    }
}
