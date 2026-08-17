import SwiftUI

extension ThemeSettingsView {
    /// Every group in `ShortcutCatalog`, flat. The help screen (`Cmd+H`) shows
    /// the same catalog filtered by topic, so the two can no longer disagree.
    var shortcutsTab: some View {
        ScrollView(.vertical, showsIndicators: false) {
            VStack(alignment: .leading, spacing: 14) {
                ForEach(ShortcutCatalog.groups) { group in
                    ShortcutGroupView(title: group.title, entries: group.entries)
                }

                Text(HintText.Settings.shortcutsTips)
                    .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .regular))
                    .foregroundStyle(themeStore.secondaryTextColor())
            }
            .padding(.top, 4)
        }
    }
}
