import SwiftUI

extension ThemeSettingsView {
    /// The whole catalog, filtered by the same topics the help screen (`Cmd+H`)
    /// offers, so the two surfaces narrow the list the same way.
    var shortcutsTab: some View {
        ShortcutsSettingsTab(bindings: $settings.shortcutBindings)
    }
}

struct ShortcutsSettingsTab: View {
    @EnvironmentObject private var themeStore: ThemeStore
    @ObservedObject private var shortcuts = ShortcutBindings.shared

    @Binding var bindings: [String: String]

    @State private var topic: ShortcutTopicFilter = .all
    @State private var collapsed: Set<String> = []
    @State private var query = ""
    @State private var configurableOnly = false

    private enum Metrics {
        static let sectionSpacing: CGFloat = 14
        static let rowSpacing: CGFloat = 8
        static let titleSpacing: CGFloat = 6
        static let controlSpacing: CGFloat = 8
        static let searchWidth: CGFloat = 230
    }

    /// A group as the list shows it: what the topic, the search and the filter
    /// left of it.
    private struct VisibleGroup: Identifiable {
        let group: ShortcutGroup
        let entries: [ShortcutEntry]
        var id: String { group.id }
    }

    private var isSearching: Bool { !trimmedQuery.isEmpty }
    private var trimmedQuery: String { query.trimmingCharacters(in: .whitespaces).lowercased() }

    /// A search covers the whole catalog, not only the chosen topic: typing
    /// "copy" under Main means the shortcut, not the tab.
    private var visibleGroups: [VisibleGroup] {
        let source = isSearching ? ShortcutCatalog.groups : topic.groups
        return source.compactMap { group in
            let entries = group.entries.filter { shows($0, in: group) }
            return entries.isEmpty ? nil : VisibleGroup(group: group, entries: entries)
        }
    }

    var body: some View {
        ScrollView(.vertical, showsIndicators: false) {
            VStack(alignment: .leading, spacing: Metrics.sectionSpacing) {
                header

                UnsavedShortcutsNotice(bindings: bindings)
                conflictNotice

                if visibleGroups.isEmpty {
                    Text(isSearching ? "No shortcut matches \"\(query)\"" : "Nothing to show")
                        .font(font(weight: .regular))
                        .foregroundStyle(themeStore.secondaryTextColor())
                } else {
                    ForEach(visibleGroups) { visible in
                        section(visible)
                    }
                }

                Text(HintText.Settings.shortcutsTips)
                    .font(font(weight: .regular))
                    .foregroundStyle(themeStore.secondaryTextColor())
            }
            .padding(.top, 4)
        }
    }

    /// One row where there is width for it; the compact window is Spotlight
    /// narrow, so the search drops to a line of its own.
    private var header: some View {
        let isCompact = themeStore.effectiveLayout == .compact
        return VStack(alignment: .leading, spacing: Metrics.controlSpacing) {
            HStack(spacing: Metrics.controlSpacing) {
                ShortcutTopicPicker(themeStore: themeStore, topic: $topic)
                configurableOnlyCapsule
                if !isCompact {
                    searchField.frame(maxWidth: Metrics.searchWidth)
                }
                Spacer(minLength: 0)
                resetAllButton
            }

            if isCompact {
                searchField
            }
        }
    }

    private var searchField: some View {
        TextField("Search shortcuts", text: $query)
            .textFieldStyle(.roundedBorder)
            .font(font(weight: .regular))
    }

    private var configurableOnlyCapsule: some View {
        ShortcutFilterCapsule(
            label: "Configurable",
            isSelected: configurableOnly,
            themeStore: themeStore
        ) {
            configurableOnly.toggle()
        }
        .help("Show only the shortcuts you can rebind")
    }

    @ViewBuilder
    private var resetAllButton: some View {
        if ShortcutBindingTable.all.contains(where: { shortcuts.resettableDefault($0.shortcut) != nil }) {
            Button("Reset all") {
                for definition in ShortcutBindingTable.all {
                    bindings[definition.configKey] = definition.defaults.joined(
                        separator: ShortcutBindings.specSeparator)
                }
            }
            .buttonStyle(.plain)
            .font(font(weight: .regular))
            .foregroundStyle(themeStore.secondaryTextColor())
            .pointingHandCursor()
            .help("Put every in-app shortcut back on its default")
        }
    }

    /// Shadowing a chord is allowed, but it is said out loud.
    @ViewBuilder
    private var conflictNotice: some View {
        let lines = shortcuts.conflictLines
        if !lines.isEmpty {
            VStack(alignment: .leading, spacing: 2) {
                ForEach(lines, id: \.self) { line in
                    Text(line)
                        .font(font(weight: .regular))
                        .foregroundStyle(themeStore.dangerColor())
                }
            }
        }
    }

    private func section(_ visible: VisibleGroup) -> some View {
        // A search result is never hidden behind a collapsed header: the reader
        // asked for these rows by name.
        let isOpen = isSearching || !collapsed.contains(visible.id)
        let count = visible.entries.filter { ConfigurableShortcut.forEntry($0.id) != nil }.count
        return VStack(alignment: .leading, spacing: Metrics.rowSpacing) {
            Button {
                if isOpen {
                    collapsed.insert(visible.id)
                } else {
                    collapsed.remove(visible.id)
                }
            } label: {
                HStack(spacing: Metrics.titleSpacing) {
                    Image(systemName: isOpen ? "chevron.down" : "chevron.right")
                    Text(visible.group.title)
                        .font(font(size: themeStore.settings.fontSize, weight: .semibold))
                    Text("\(count) of \(visible.entries.count) configurable")
                        .font(font(weight: .regular))
                        .foregroundStyle(themeStore.mutedTextColor())
                    Spacer(minLength: 0)
                }
                .foregroundStyle(themeStore.secondaryTextColor())
            }
            .buttonStyle(.plain)
            .pointingHandCursor()
            .disabled(isSearching)

            if isOpen {
                ShortcutGroupView(title: nil, entries: visible.entries, bindings: $bindings)
            }
        }
    }

    /// Matched against what the reader sees - action, keys as bound now, group
    /// - plus the id, which is the config key they would edit by hand.
    private func shows(_ entry: ShortcutEntry, in group: ShortcutGroup) -> Bool {
        let shortcut = ConfigurableShortcut.forEntry(entry.id)
        if configurableOnly, shortcut == nil { return false }
        guard isSearching else { return true }
        let haystack = [entry.action, shortcut?.display ?? entry.keys, group.title, entry.id]
        return haystack.contains { $0.lowercased().contains(trimmedQuery) }
    }

    private func font(size: Double? = nil, weight: Font.Weight) -> Font {
        themeStore.uiFont(size: CGFloat(size ?? (themeStore.settings.fontSize - 1)), weight: weight)
    }
}

/// One notice for all unsaved rebinds, not one per row.
private struct UnsavedShortcutsNotice: View {
    @EnvironmentObject private var themeStore: ThemeStore
    @ObservedObject private var launcherHotkey = LauncherHotkeyController.shared
    @ObservedObject private var shortcuts = ShortcutBindings.shared

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
