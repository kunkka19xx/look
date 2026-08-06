import SwiftUI

/// Generic confirm bar for any pending action. Mirrors `KillConfirmationBar`;
/// renders whatever `PlannedAction.preview` says, so every tool reuses it.
struct PendingActionBar: View {
    let action: PlannedAction
    let themeStore: ThemeStore
    let onConfirm: () -> Void
    let onCancel: () -> Void

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: "sparkles")
                .font(.system(size: CGFloat(themeStore.settings.fontSize + 4)))
                .foregroundStyle(themeStore.accentColor())
            VStack(alignment: .leading, spacing: 2) {
                Text("\(action.preview.title)?")
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize), weight: .semibold))
                    .foregroundStyle(themeStore.fontColor())
                Text(action.preview.detail)
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: .regular))
                    .foregroundStyle(themeStore.mutedTextColor())
            }
            Spacer()
            Button {
                onConfirm()
            } label: {
                Text("Enter")
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .medium))
                    .foregroundStyle(themeStore.onAccentColor())
                    .padding(.horizontal, 12)
                    .padding(.vertical, 6)
                    .background(themeStore.accentColor(), in: Capsule())
            }
            .buttonStyle(.plain)
            Button {
                onCancel()
            } label: {
                Text("Esc")
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .medium))
                    .foregroundStyle(themeStore.fontColor())
                    .padding(.horizontal, 12)
                    .padding(.vertical, 6)
                    .background(themeStore.controlFillColor(), in: Capsule())
            }
            .buttonStyle(.plain)
        }
        .padding(10)
        .background(themeStore.controlFillColor().opacity(0.92), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
    }
}
