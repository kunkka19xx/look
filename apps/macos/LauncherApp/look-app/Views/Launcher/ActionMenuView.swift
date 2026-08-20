import SwiftUI

/// Everything you can do to the selected row, in one popup.
///
/// It floats under the preview's header rather than sitting in the layout, so a
/// row with actions costs the preview no space until the user asks for them
/// with Cmd+K. Information (a file preview, a folder listing, Bluetooth's paired
/// devices) stays in the panel; only the verbs live here.
struct ActionMenuView: View {
    let descriptors: [QuickActionDescriptor]
    let states: [String: ActionState]
    let focusedIndex: Int
    let themeStore: ThemeStore
    let onRun: (QuickActionDescriptor) -> Void

    private typealias Layout = AppConstants.Launcher.ActionMenu

    var body: some View {
        ScrollViewReader { proxy in
            ScrollView(.vertical, showsIndicators: false) {
                LazyVStack(alignment: .leading, spacing: 2) {
                    ForEach(Array(descriptors.enumerated()), id: \.element.id) { pair in
                        row(pair.element, isFocused: pair.offset == focusedIndex)
                            .id(pair.offset)
                    }
                }
            }
            .frame(height: menuHeight)
            .onChange(of: focusedIndex) { _, index in
                withAnimation(Motion.Selection.glide) { proxy.scrollTo(index) }
            }
        }
        .padding(4)
        .background(menuBackground)
        .overlay(menuBorder)
    }

    /// Tall enough for its rows, never taller than the cap.
    private var menuHeight: CGFloat {
        let rows = CGFloat(descriptors.count)
        let rowHeight = CGFloat(themeStore.settings.fontSize - 1) + Layout.rowVerticalPadding * 2 + 6
        return min(rows * rowHeight + max(0, rows - 1) * 2, Layout.maxHeight)
    }

    private var menuBackground: some View {
        RoundedRectangle(cornerRadius: Layout.cornerRadius, style: .continuous)
            .fill(.ultraThinMaterial)
            .background(
                RoundedRectangle(cornerRadius: Layout.cornerRadius, style: .continuous)
                    .fill(themeStore.panelFillColor())
            )
            .shadow(color: .black.opacity(0.45), radius: Layout.shadowRadius, x: 0, y: 8)
    }

    private var menuBorder: some View {
        RoundedRectangle(cornerRadius: Layout.cornerRadius, style: .continuous)
            .strokeBorder(themeStore.dividerColor(), lineWidth: 1)
    }

    private func row(_ descriptor: QuickActionDescriptor, isFocused: Bool) -> some View {
        let titleSize = CGFloat(themeStore.settings.fontSize - 1)
        let hintSize = CGFloat(themeStore.settings.fontSize - 2)

        return HStack(spacing: 8) {
            Text(title(for: descriptor))
                .font(themeStore.uiFont(size: titleSize, weight: .medium))
                .foregroundStyle(themeStore.fontColor())
                .lineLimit(1)
            Spacer(minLength: 8)
            if isFocused {
                Text(Layout.runHint)
                    .font(themeStore.uiFont(size: hintSize, weight: .semibold))
                    .foregroundStyle(themeStore.secondaryTextColor())
            }
        }
        .padding(.horizontal, Layout.rowHorizontalPadding)
        .padding(.vertical, Layout.rowVerticalPadding)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(rowBackground(isFocused: isFocused))
        .contentShape(Rectangle())
        .onTapGesture { onRun(descriptor) }
    }

    private func rowBackground(isFocused: Bool) -> some View {
        RoundedRectangle(cornerRadius: 6, style: .continuous)
            .fill(isFocused ? themeStore.selectionFillColor() : Color.clear)
    }

    /// A toggle names the change it would make, not the setting: in a list of
    /// verbs "Turn on Bluetooth" reads as something to do, where a bare
    /// "Bluetooth" reads as a place to go.
    private func title(for descriptor: QuickActionDescriptor) -> String {
        guard descriptor.control == .toggle else { return descriptor.title }
        switch states[descriptor.actionId] {
        case .on:
            return descriptor.offLabel ?? descriptor.title
        case .off:
            return descriptor.onLabel ?? descriptor.title
        default:
            return descriptor.title
        }
    }
}
