import SwiftUI

/// A capsule holding a label and its (x): Settings' directory lists and the
/// launcher's picks.
struct RemovablePill<Label: View>: View {
    let themeStore: ThemeStore
    let removeLabel: String
    let onRemove: () -> Void
    @ViewBuilder let label: () -> Label

    var body: some View {
        HStack(spacing: RemovablePillLayout.spacing) {
            label()
            Button {
                withAnimation(Motion.Insert.animation) { onRemove() }
            } label: {
                Image(systemName: RemovablePillLayout.removeSymbol)
                    .font(.system(size: RemovablePillLayout.removeGlyphSize, weight: .semibold))
            }
            .buttonStyle(.plain)
            .accessibilityLabel(removeLabel)
        }
        .font(themeStore.uiFont(
            size: CGFloat(max(RemovablePillLayout.minFontSize, themeStore.settings.fontSize - RemovablePillLayout.fontSizeDrop)),
            weight: .regular))
        .foregroundStyle(themeStore.secondaryTextColor())
        .padding(.horizontal, RemovablePillLayout.paddingX)
        .padding(.vertical, RemovablePillLayout.paddingY)
        .background(themeStore.liftColor(opacity: RemovablePillLayout.fillOpacity), in: Capsule())
        .transition(Motion.Insert.transition)
    }
}

private enum RemovablePillLayout {
    static let spacing: CGFloat = 6
    static let paddingX: CGFloat = 9
    static let paddingY: CGFloat = 5
    static let fillOpacity: Double = 0.12
    static let removeGlyphSize: CGFloat = 10
    static let removeSymbol = "xmark"
    static let fontSizeDrop: Double = 2
    static let minFontSize: Double = 10
}
