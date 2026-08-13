import AppKit
import SwiftUI

/// Copies a finished answer. Same affordance as `AICodeBlockView`'s copy (icon,
/// checkmark confirmation, 1.2s reset) so the two read as one control in two
/// places, rather than two controls.
struct AnswerCopyButton: View {
    let text: String
    let themeStore: ThemeStore

    @State private var copied = false

    var body: some View {
        Button {
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(text, forType: .string)
            copied = true
            DispatchQueue.main.asyncAfter(deadline: .now() + 1.2) { copied = false }
        } label: {
            Image(systemName: copied ? "checkmark" : "doc.on.doc")
                .font(.system(size: CGFloat(themeStore.settings.fontSize - 3)))
                .foregroundStyle(copied ? Color.green.opacity(0.85) : themeStore.mutedTextColor())
        }
        .buttonStyle(.plain)
        .help(copied ? "Copied" : "Copy answer")
    }
}
