import SwiftUI

extension View {
    /// Pins the digit this icon answers to on its top trailing corner, the badge
    /// the search bar's app icons and the result rows share. Nil draws nothing.
    func keyDigitBadge(_ digit: Int?, themeStore: ThemeStore) -> some View {
        overlay(alignment: .topTrailing) {
            if let digit {
                KeyDigitBadge(digit: digit, themeStore: themeStore)
            }
        }
    }
}

private struct KeyDigitBadge: View {
    let digit: Int
    let themeStore: ThemeStore

    private enum Layout {
        static let size: CGFloat = 14
        static let fontSize: CGFloat = 9
        static let scrimOpacity: Double = 0.72
        static let ringOpacity: Double = 0.45
        static let ringWidth: CGFloat = 1
        static let offsetX: CGFloat = 5
        static let offsetY: CGFloat = -3
    }

    var body: some View {
        Text("\(digit)")
            .font(.system(size: Layout.fontSize, weight: .bold, design: .monospaced))
            .foregroundStyle(themeStore.fontColor())
            .frame(width: Layout.size, height: Layout.size)
            .background(
                RoundedRectangle(cornerRadius: Layout.size / 2)
                    .fill(themeStore.scrimColor(opacity: Layout.scrimOpacity))
            )
            .overlay(
                RoundedRectangle(cornerRadius: Layout.size / 2)
                    .strokeBorder(
                        themeStore.fontColor(opacityMultiplier: Layout.ringOpacity), lineWidth: Layout.ringWidth)
            )
            .offset(x: Layout.offsetX, y: Layout.offsetY)
    }
}
