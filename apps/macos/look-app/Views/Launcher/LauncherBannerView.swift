import SwiftUI

struct LauncherBannerView: View {
    @EnvironmentObject private var themeStore: ThemeStore

    let message: String
    let backgroundColor: Color
    let copyText: String?
    let onCopy: (() -> Void)?
    let onDismiss: (() -> Void)?

    var body: some View {
        HStack(spacing: 8) {
            Text(message)
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .semibold))
                .foregroundStyle(themeStore.fontColor())

            if copyText != nil {
                Button("Copy") {
                    onCopy?()
                }
                .buttonStyle(.plain)
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .semibold))
                .padding(.horizontal, 8)
                .padding(.vertical, 3)
                .background(themeStore.controlFillColor(), in: Capsule())
            }

            if onDismiss != nil {
                Button(action: {
                    onDismiss?()
                }) {
                    Text("Dismiss")
                        .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .semibold))
                        .padding(.horizontal, 6)
                        .padding(.vertical, 3)
                        .background(themeStore.controlFillColor(), in: Capsule())
                }
                .buttonStyle(.plain)
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .background(backgroundColor, in: Capsule())
        .transition(.move(edge: .top).combined(with: .opacity))
    }
}
