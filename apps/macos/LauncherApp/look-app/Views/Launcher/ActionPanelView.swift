import SwiftUI

/// Right-column panel for a result's Quick Actions (see docs/writing-controls.md).
/// Renders each declared action with its live state (a toggle shows On/Off), the
/// focused action highlighted, and the Cmd+J/K + Enter hints. Info-field values
/// are resolved by adapters in a later pass; today a toggle's state is its status.
struct ActionPanelView: View {
    let title: String
    let descriptors: [QuickActionDescriptor]
    let states: [String: ActionState]
    let focusIndex: Int?
    let themeStore: ThemeStore

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(title)
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize + 2), weight: .semibold))
                .foregroundStyle(themeStore.fontColor())
                .lineLimit(1)

            ForEach(Array(descriptors.enumerated()), id: \.element.id) { index, descriptor in
                actionRow(descriptor, isFocused: focusIndex == index)
            }

            Spacer(minLength: 0)

            Text("⌘J / ⌘K move  •  ⏎ run")
                .font(themeStore.uiFont(size: CGFloat(max(10, themeStore.settings.fontSize - 3)), weight: .regular))
                .foregroundStyle(themeStore.mutedTextColor())
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .padding(14)
    }

    @ViewBuilder
    private func actionRow(_ descriptor: QuickActionDescriptor, isFocused: Bool) -> some View {
        HStack(spacing: 10) {
            Text(descriptor.title)
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize), weight: .medium))
                .foregroundStyle(themeStore.fontColor())
            Spacer(minLength: 0)
            stateBadge(for: descriptor)
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
        .background(
            isFocused ? themeStore.selectionFillColor() : .clear,
            in: RoundedRectangle(cornerRadius: 8, style: .continuous)
        )
        .overlay {
            if isFocused {
                RoundedRectangle(cornerRadius: 8, style: .continuous)
                    .stroke(themeStore.dividerColor(), lineWidth: 1)
            }
        }
    }

    @ViewBuilder
    private func stateBadge(for descriptor: QuickActionDescriptor) -> some View {
        switch states[descriptor.actionId] {
        case .on:
            pill(descriptor.onLabel ?? "On", on: true)
        case .off:
            pill(descriptor.offLabel ?? "Off", on: false)
        case .value(let text):
            pill(text, on: false)
        case .unavailable(let reason):
            Text(reason)
                .font(themeStore.uiFont(size: CGFloat(max(10, themeStore.settings.fontSize - 3)), weight: .regular))
                .foregroundStyle(themeStore.mutedTextColor())
        case nil:
            // State still loading.
            Text("…").foregroundStyle(themeStore.mutedTextColor())
        }
    }

    private func pill(_ text: String, on: Bool) -> some View {
        Text(text)
            .font(themeStore.uiFont(size: CGFloat(max(11, themeStore.settings.fontSize - 2)), weight: .semibold))
            .foregroundStyle(on ? Color.white : themeStore.fontColor())
            .padding(.horizontal, 10)
            .padding(.vertical, 3)
            .background(
                (on ? Color.green.opacity(0.75) : themeStore.dividerColor().opacity(0.6)),
                in: Capsule()
            )
    }
}
