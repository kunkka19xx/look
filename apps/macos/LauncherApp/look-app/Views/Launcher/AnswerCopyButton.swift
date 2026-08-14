import AppKit
import SwiftUI

/// THE copy control: icon, checkmark confirmation, 1.2s reset. Used by the
/// answer bubble and by code blocks, so the two are one control in two places
/// rather than two implementations that drift.
struct AnswerCopyButton: View {
    let text: String
    let themeStore: ThemeStore
    var helpLabel: String = "Copy answer"

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
        .help(copied ? "Copied" : helpLabel)
    }
}
