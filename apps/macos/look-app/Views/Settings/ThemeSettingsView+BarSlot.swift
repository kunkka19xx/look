import SwiftUI

extension ThemeSettingsView {
    static let barSlotLabel = "Search Bar"
    static let barSlotHelp =
        "What the right end of the search bar holds. Running and pinned apps answer to ⌘1-9."
    private static let pinnedLabel = "Pinned Apps"
    private static let pinnedEmpty = "Pin an app from its row's actions menu (⌘K)"
    private static let pinnedMissing = "missing"
    private static let unpinFailed = "Unpin failed"
    private static let unpinFailedDuration = 1.6
    private static let pinGap: CGFloat = 8
    private static let barScriptLabel = "Bar Script"
    private static let barScriptUnset =
        "Set bar_script in ~/.look/config to a command or the path of a script, then reload the config"
    private static let barScriptPending = "Runs when the launcher opens"
    private static let barScriptSilent = "Printed nothing"
    private static let barScriptCommandMaxWidth: CGFloat = 320

    var pinnedAppsRow: some View {
        settingRow(Self.pinnedLabel, controlWidth: nil) {
            if barSlotService.pins.isEmpty {
                settingHint(Self.pinnedEmpty)
            } else {
                ScrollView(.horizontal) {
                    HStack(spacing: Self.pinGap) {
                        ForEach(barSlotService.pins) { pin in
                            RemovablePill(themeStore: themeStore, removeLabel: "Unpin \(pin.title ?? pin.id)") {
                                unpin(pin)
                            } label: {
                                pinLabel(pin)
                            }
                        }
                    }
                }
                .scrollIndicators(.hidden)
                .fixedSize(horizontal: false, vertical: true)
            }
        }
    }

    var barScriptRow: some View {
        let command = themeStore.barScript
        return settingRow(Self.barScriptLabel, controlWidth: nil) {
            if command.isEmpty {
                settingHint(Self.barScriptUnset)
            } else {
                HStack(spacing: 10) {
                    Text(command)
                        .font(.system(size: CGFloat(settings.fontSize - 2), design: .monospaced))
                        .foregroundStyle(themeStore.secondaryTextColor())
                        .lineLimit(1)
                        .truncationMode(.middle)
                        .frame(maxWidth: Self.barScriptCommandMaxWidth, alignment: .leading)
                        .layoutPriority(1)
                        .help(command)

                    scriptStatus(for: command)
                }
            }
        }
    }

    private func pinLabel(_ pin: BarPin) -> some View {
        HStack(spacing: 0) {
            Text(pin.title ?? pin.id)
            if pin.title == nil {
                Text(" (\(Self.pinnedMissing))")
                    .foregroundStyle(themeStore.mutedTextColor())
            }
        }
        .lineLimit(1)
    }

    private func unpin(_ pin: BarPin) {
        guard themeStore.togglePinnedApp(pin.id) == nil else { return }
        saveMessage = SaveMessage(text: Self.unpinFailed, succeeded: false)
        DispatchQueue.main.asyncAfter(deadline: .now() + Self.unpinFailedDuration) {
            saveMessage = nil
        }
    }

    private func scriptStatus(for command: String) -> some View {
        let outcome = barSlotService.scriptOutcome(for: command)
        let text =
            outcome?.error ?? barSlotService.scriptLine(for: command)
            ?? (outcome == nil ? Self.barScriptPending : Self.barScriptSilent)
        return settingHint(text, color: outcome?.error == nil ? nil : themeStore.dangerColor())
            .truncationMode(.tail)
            .textSelection(.enabled)
            .help(text)
    }
}
