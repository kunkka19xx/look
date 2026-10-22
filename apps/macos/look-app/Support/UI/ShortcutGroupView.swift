import SwiftUI

/// One block of shortcuts, rendered identically on the help screen (`Cmd+H`)
/// and in Settings > Shortcuts. The two screens used to keep private copies
/// that drifted apart by four constants.
///
/// The key capsule uses `liftColor`, not the `controlFillColor` of the topic
/// capsules: a static badge should not look like the button beside it.
struct ShortcutGroupView: View {
    @EnvironmentObject private var themeStore: ThemeStore
    /// Redraws the key capsules when a rebind takes effect.
    @ObservedObject private var shortcuts = ShortcutBindings.shared

    /// Nil where the surface draws its own heading, as the collapsible sections
    /// in Settings do.
    let title: String?
    let entries: [ShortcutEntry]
    /// Given, configurable entries become recorders; the help screen passes nil.
    var bindings: Binding<[String: String]>? = nil

    private enum Metrics {
        static let rowSpacing: CGFloat = 8
        static let keyToActionSpacing: CGFloat = 10
        static let keyHorizontalPadding: CGFloat = 8
        static let keyVerticalPadding: CGFloat = 3
        static let keyFillOpacity = 0.14
        /// Keys sit in their own column so the actions line up. A longer chord
        /// overflows it rather than being truncated.
        static let keyColumnWidth: CGFloat = 170
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Metrics.rowSpacing) {
            if let title {
                Text(title)
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize), weight: .semibold))
                    .foregroundStyle(themeStore.secondaryTextColor())
            }

            ForEach(entries) { entry in
                HStack(alignment: .firstTextBaseline, spacing: Metrics.keyToActionSpacing) {
                    keys(for: entry)
                        .frame(minWidth: Metrics.keyColumnWidth, alignment: .leading)
                    Text(entry.action)
                        .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .regular))
                        .foregroundStyle(themeStore.secondaryTextColor())
                    Spacer(minLength: 0)
                }
            }
        }
    }

    @ViewBuilder
    private func keys(for entry: ShortcutEntry) -> some View {
        if let shortcut = ConfigurableShortcut.forEntry(entry.id) {
            if let bindings {
                ShortcutRecorderField(shortcut: shortcut, bindings: bindings)
            } else {
                keyCapsule(shortcut.display)
            }
        } else {
            // Dimmed only where the column is otherwise editable: on the help
            // screen nothing is, and the distinction would mean nothing.
            keyCapsule(entry.keys, dimmed: bindings != nil)
        }
    }

    private func keyCapsule(_ keys: String, dimmed: Bool = false) -> some View {
        Text(keys)
            .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .regular))
            .foregroundStyle(dimmed ? themeStore.mutedTextColor() : themeStore.fontColor())
            .padding(.horizontal, Metrics.keyHorizontalPadding)
            .padding(.vertical, Metrics.keyVerticalPadding)
            .background(themeStore.liftColor(opacity: Metrics.keyFillOpacity), in: Capsule())
    }
}
