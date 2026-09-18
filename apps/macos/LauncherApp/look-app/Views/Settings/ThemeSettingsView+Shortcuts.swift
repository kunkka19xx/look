import SwiftUI

extension ThemeSettingsView {
    /// Every group in `ShortcutCatalog`, flat. The help screen (`Cmd+H`) shows
    /// the same catalog filtered by topic, so the two can no longer disagree.
    var shortcutsTab: some View {
        ScrollView(.vertical, showsIndicators: false) {
            VStack(alignment: .leading, spacing: 14) {
                UnsavedShortcutsNotice(bindings: settings.shortcutBindings)

                ForEach(ShortcutCatalog.groups) { group in
                    ShortcutGroupView(title: group.title, entries: group.entries, bindings: $settings.shortcutBindings)
                }

                Text(HintText.Settings.shortcutsTips)
                    .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .regular))
                    .foregroundStyle(themeStore.secondaryTextColor())
            }
            .padding(.top, 4)
        }
    }
}

/// One notice for all unsaved rebinds, not one per row.
private struct UnsavedShortcutsNotice: View {
    @EnvironmentObject private var themeStore: ThemeStore
    @ObservedObject private var launcherHotkey = LauncherHotkeyController.shared

    let bindings: [String: String]

    private var unsavedCount: Int {
        ConfigurableShortcut.all.filter { $0.hasUnsavedChange(in: bindings) }.count
    }

    var body: some View {
        let count = unsavedCount
        if count > 0 {
            Text("\(count) shortcut change\(count == 1 ? "" : "s") pending. Save Config to apply.")
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .semibold))
                .foregroundStyle(themeStore.accentColor())
        }
    }
}
