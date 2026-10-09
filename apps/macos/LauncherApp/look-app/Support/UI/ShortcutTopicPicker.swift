import SwiftUI

/// One capsule in a shortcut filter row: a topic, or the toggle beside them in
/// Settings. `controlFillColor` unselected, which is what separates these from
/// the static key badges below: these are buttons.
struct ShortcutFilterCapsule: View {
    enum Metrics {
        static let selectedOpacity = 0.22
        static let spacing: CGFloat = 6
        static let horizontalPadding: CGFloat = 10
        static let verticalPadding: CGFloat = 4
    }

    let label: String
    let isSelected: Bool
    let themeStore: ThemeStore
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Text(label)
                .font(themeStore.uiFont(
                    size: CGFloat(themeStore.settings.fontSize - 1),
                    weight: isSelected ? .semibold : .regular))
                .foregroundStyle(isSelected ? themeStore.fontColor() : themeStore.mutedTextColor())
                .padding(.horizontal, Metrics.horizontalPadding)
                .padding(.vertical, Metrics.verticalPadding)
                .background(
                    isSelected
                        ? themeStore.accentColor().opacity(Metrics.selectedOpacity)
                        : themeStore.controlFillColor(),
                    in: Capsule())
        }
        .buttonStyle(.plain)
        .pointingHandCursor()
    }
}

/// The capsules that filter a shortcut list by topic, on the help screen
/// (`Cmd+H`) and in Settings > Shortcuts.
struct ShortcutTopicPicker: View {
    let themeStore: ThemeStore
    @Binding var topic: ShortcutTopicFilter

    var body: some View {
        HStack(spacing: ShortcutFilterCapsule.Metrics.spacing) {
            ForEach(ShortcutTopicFilter.allCases) { candidate in
                ShortcutFilterCapsule(
                    label: candidate.label,
                    isSelected: candidate == topic,
                    themeStore: themeStore
                ) {
                    topic = candidate
                }
                .help("Show \(candidate.label) shortcuts")
            }
        }
        // Keeps the capsules at their own width instead of being squeezed by
        // whatever shares their row.
        .fixedSize()
    }
}
