import SwiftUI

/// Generic confirm bar for any pending action. Mirrors `KillConfirmationBar`;
/// renders whatever `PlannedAction.preview` says, so every tool reuses it.
struct PendingActionBar: View {
    /// Every step of the plan. Usually one; several for a compound request,
    /// which confirms and undoes as a unit - so the bar must show ALL of what
    /// Enter is about to do, never just the first.
    let steps: [PlannedAction]
    let themeStore: ThemeStore
    let onConfirm: () -> Void
    let onCancel: () -> Void

    init(
        steps: [PlannedAction],
        themeStore: ThemeStore,
        onConfirm: @escaping () -> Void,
        onCancel: @escaping () -> Void
    ) {
        self.steps = steps
        self.themeStore = themeStore
        self.onConfirm = onConfirm
        self.onCancel = onCancel
    }

    init(
        action: PlannedAction,
        themeStore: ThemeStore,
        onConfirm: @escaping () -> Void,
        onCancel: @escaping () -> Void
    ) {
        self.init(
            steps: [action], themeStore: themeStore, onConfirm: onConfirm, onCancel: onCancel)
    }

    private var fontSize: Double { themeStore.settings.fontSize }

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: "sparkles")
                .font(.system(size: CGFloat(fontSize + 4)))
                .foregroundStyle(themeStore.accentColor())
            VStack(alignment: .leading, spacing: steps.count > 1 ? 6 : 2) {
                if steps.count > 1 {
                    Text("Do \(steps.count) things?")
                        .font(themeStore.uiFont(size: CGFloat(fontSize), weight: .semibold))
                        .foregroundStyle(themeStore.fontColor())
                }
                ForEach(Array(steps.enumerated()), id: \.offset) { index, step in
                    HStack(alignment: .top, spacing: 6) {
                        if steps.count > 1 {
                            // Numbered, because order matters when they run.
                            Text("\(index + 1).")
                                .font(themeStore.uiFont(size: CGFloat(fontSize - 1), weight: .semibold))
                                .foregroundStyle(themeStore.mutedTextColor())
                        }
                        VStack(alignment: .leading, spacing: 2) {
                            Text(steps.count > 1 ? step.preview.title : "\(step.preview.title)?")
                                .font(
                                    themeStore.uiFont(
                                        size: CGFloat(steps.count > 1 ? fontSize - 1 : fontSize),
                                        weight: .semibold)
                                )
                                .foregroundStyle(themeStore.fontColor())
                            Text(step.preview.detail)
                                .font(themeStore.uiFont(size: CGFloat(fontSize - 2), weight: .regular))
                                .foregroundStyle(themeStore.mutedTextColor())
                        }
                    }
                }
            }
            Spacer()
            Button {
                onConfirm()
            } label: {
                Text("Enter")
                    .font(themeStore.uiFont(size: CGFloat(fontSize - 1), weight: .medium))
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
                    .font(themeStore.uiFont(size: CGFloat(fontSize - 1), weight: .medium))
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
