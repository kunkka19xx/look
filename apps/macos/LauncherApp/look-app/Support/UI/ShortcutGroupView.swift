import SwiftUI

/// One titled block of shortcuts, rendered identically wherever it appears: the
/// in-window help screen (`Cmd+H`) and Settings > Shortcuts.
///
/// Both read the same `ShortcutCatalog`, so the content could not drift, but the
/// two screens kept private copies of this view that differed in four constants
/// - a key capsule that was a lighter fill on one screen than the other. Same
/// data rendered two ways is the shallow end of the same problem, so there is
/// one view.
///
/// The key capsule deliberately uses `liftColor` rather than `controlFillColor`:
/// the help screen's topic capsules are `controlFillColor` and are *clickable*,
/// and a static key badge should not look like a button sitting next to one.
struct ShortcutGroupView: View {
    @EnvironmentObject private var themeStore: ThemeStore

    let title: String
    let entries: [ShortcutEntry]

    private enum Metrics {
        static let rowSpacing: CGFloat = 8
        static let keyToActionSpacing: CGFloat = 10
        static let keyHorizontalPadding: CGFloat = 8
        static let keyVerticalPadding: CGFloat = 3
        static let keyFillOpacity = 0.14
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Metrics.rowSpacing) {
            Text(title)
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize), weight: .semibold))
                .foregroundStyle(themeStore.secondaryTextColor())

            ForEach(entries) { entry in
                HStack(alignment: .firstTextBaseline, spacing: Metrics.keyToActionSpacing) {
                    Text(entry.keys)
                        .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .regular))
                        .padding(.horizontal, Metrics.keyHorizontalPadding)
                        .padding(.vertical, Metrics.keyVerticalPadding)
                        .background(themeStore.liftColor(opacity: Metrics.keyFillOpacity), in: Capsule())
                    Text(entry.action)
                        .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .regular))
                        .foregroundStyle(themeStore.secondaryTextColor())
                    Spacer(minLength: 0)
                }
            }
        }
    }
}
