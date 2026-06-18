import SwiftUI

extension ThemeSettingsView {
    var shortcutsTab: some View {
        ScrollView(.vertical, showsIndicators: false) {
            VStack(alignment: .leading, spacing: 14) {
                ForEach(ShortcutDocs.sections) { section in
                    ShortcutSection(title: section.title, items: section.items)
                }

                Text("This panel is intended as living documentation. We can add command and workflow docs here as features grow.")
                    .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .regular))
                    .foregroundStyle(themeStore.secondaryTextColor())

                Text(HintText.Settings.shortcutsTips)
                    .font(themeStore.uiFont(size: CGFloat(settings.fontSize - 1), weight: .regular))
                    .foregroundStyle(themeStore.secondaryTextColor())
            }
            .padding(.top, 4)
        }
    }
}

struct ShortcutItem: Identifiable {
    let id = UUID()
    let keys: String
    let action: String
}

struct ShortcutSectionData: Identifiable {
    let id = UUID()
    let title: String
    let items: [ShortcutItem]
}

enum ShortcutDocs {
    static let sections: [ShortcutSectionData] = [
        ShortcutSectionData(
            title: "Core launcher",
            items: [
                ShortcutItem(keys: "Tab", action: "Select next result"),
                ShortcutItem(keys: "Shift+Tab", action: "Select previous result"),
                ShortcutItem(keys: "Up / Down", action: "Move selection"),
                ShortcutItem(keys: "Cmd+C", action: "Copy selected file/folder to pasteboard"),
                ShortcutItem(keys: "Cmd+F", action: "Reveal selected app/file/folder in Finder"),
                ShortcutItem(keys: "Cmd+Enter", action: "Search query on Google"),
                ShortcutItem(keys: "Cmd+/", action: "Enter command mode"),
                ShortcutItem(keys: ":cmd", action: "Jump to a command from home (e.g. :calc 2+2, :kill chrome, :sys, :pomo)"),
                ShortcutItem(keys: "Cmd+1..5", action: "Switch directly to /shell, /calc, /kill, /sys, /pomo"),
                ShortcutItem(keys: "Cmd+Shift+,", action: "Open/close settings panel"),
                ShortcutItem(keys: "Cmd+Shift+;", action: "Reload .look.config"),
                ShortcutItem(keys: "Cmd+H", action: "Toggle in-window keyboard help screen"),
                ShortcutItem(keys: "Esc", action: "Back to app list (in command mode)"),
                ShortcutItem(keys: "Shift+Esc", action: "Hide launcher"),
            ]
        ),
        ShortcutSectionData(
            title: "Search prefixes",
            // Derived from the canonical prefix list (AppConstants) so this panel,
            // the in-window help screen, and the `"` discovery menu can't drift.
            // Type `"` alone to browse these prefixes interactively.
            items: AppConstants.Launcher.PrefixSuggestion.all.map {
                ShortcutItem(keys: $0.prefix, action: $0.description)
            }
        ),
        ShortcutSectionData(
            title: "Clipboard history",
            items: [
                ShortcutItem(keys: "Enter", action: "Copy selected history item back to clipboard"),
                ShortcutItem(keys: "Delete button", action: "Remove selected clipboard item from look history"),
            ]
        ),
        ShortcutSectionData(
            title: "Pomodoro (/pomo)",
            items: [
                ShortcutItem(keys: "Space", action: "Start / pause the active session"),
                ShortcutItem(keys: "R", action: "Reset the timer back to idle"),
                ShortcutItem(keys: "P", action: "Toggle music play / pause"),
                ShortcutItem(keys: "Mouse / key idle", action: "After 5s, panel fades to clock-only standby; any input restores"),
                ShortcutItem(keys: "Menu bar item", action: "Click the timer icon in the menu bar to jump back into /pomo"),
            ]
        ),
        ShortcutSectionData(
            title: "Panels",
            items: [
                ShortcutItem(keys: "Cmd+Shift+,", action: "Open/close theme and docs panel"),
                ShortcutItem(keys: "Cmd+Shift+;", action: "Reload .look.config"),
                ShortcutItem(keys: "Save Config", action: "Write current UI settings to .look.config"),
            ]
        ),
        ShortcutSectionData(
            title: "Zoom",
            items: [
                ShortcutItem(keys: "Cmd+-", action: "Zoom out UI scale"),
                ShortcutItem(keys: "Cmd+=", action: "Zoom in UI scale"),
                ShortcutItem(keys: "Cmd+0", action: "Reset UI scale"),
            ]
        ),
        ShortcutSectionData(
            title: "Theme controls",
            items: [
                ShortcutItem(keys: "Appearance tab", action: "Tint, blur material, blur opacity"),
                ShortcutItem(keys: "Advanced tab", action: "Background, indexing, privacy, logging controls"),
                ShortcutItem(keys: "Shortcuts tab", action: "In-app keyboard documentation"),
            ]
        ),
    ]
}

struct ShortcutSection: View {
    @EnvironmentObject private var themeStore: ThemeStore

    let title: String
    let items: [ShortcutItem]

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(title)
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize), weight: .medium))
                .foregroundStyle(themeStore.secondaryTextColor())

            ForEach(items) { item in
                HStack(alignment: .firstTextBaseline, spacing: 10) {
                    Text(item.keys)
                        .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .regular))
                        .padding(.horizontal, 8)
                        .padding(.vertical, 3)
                        .background(.white.opacity(0.14), in: Capsule())
                    Text(item.action)
                        .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: .regular))
                        .foregroundStyle(themeStore.secondaryTextColor())
                    Spacer(minLength: 0)
                }
            }
        }
    }
}
