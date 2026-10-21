import SwiftUI

/// Post-action row shown where the confirm bar was: the receipt summary with an
/// Undo affordance ("Added \"Dentist\"  [Undo ⌘Z]"), or an error message when a
/// plan failed. Dismissed by typing something new or hiding the launcher.
struct ActionResultBar: View {
    let message: String
    let canUndo: Bool
    let themeStore: ThemeStore
    let onUndo: () -> Void

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: canUndo ? "checkmark.circle.fill" : "info.circle")
                .font(.system(size: CGFloat(themeStore.settings.fontSize + 2)))
                .foregroundStyle(canUndo ? Color.green.opacity(0.85) : themeStore.mutedTextColor())
            Text(message)
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .medium))
                .foregroundStyle(themeStore.fontColor())
                .lineLimit(1)
            Spacer()
            if canUndo {
                Button {
                    onUndo()
                } label: {
                    Text("Undo ⌘Z")
                        .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .medium))
                        .foregroundStyle(themeStore.fontColor())
                        .padding(.horizontal, 12)
                        .padding(.vertical, 6)
                        .background(themeStore.controlFillColor(), in: Capsule())
                }
                .buttonStyle(.plain)
            }
        }
        .padding(10)
        .background(themeStore.controlFillColor().opacity(0.92), in: RoundedRectangle(cornerRadius: themeStore.barRadius, style: .continuous))
    }
}
