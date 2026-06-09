import AppKit
import SwiftUI

/// Shared version + update-status UI, used by both Settings → About and the
/// Help screen header so the two stay in sync (single source of the buttons,
/// states, and Homebrew hint).
struct AppUpdateStatusView: View {
    let themeStore: ThemeStore
    @ObservedObject private var updateChecker = UpdateChecker.shared

    private var fontSize: CGFloat { CGFloat(themeStore.settings.fontSize) }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 8) {
                Text(AppVersion.displayString)
                    .font(themeStore.uiFont(size: fontSize - 1, weight: .semibold))
                    .foregroundStyle(themeStore.fontColor())
                    .textSelection(.enabled)

                if updateChecker.availableUpdate == nil, let status = updateChecker.statusMessage {
                    Text(status)
                        .font(themeStore.uiFont(size: fontSize - 2, weight: .regular))
                        .foregroundStyle(themeStore.mutedTextColor())
                        .lineLimit(1)
                }

                Spacer(minLength: 0)

                pill(updateChecker.isChecking ? "Checking…" : "Check for Updates") {
                    updateChecker.checkForUpdates(force: true)
                }
                .disabled(updateChecker.isChecking)
            }

            if let update = updateChecker.availableUpdate {
                HStack(spacing: 8) {
                    Text("Update available: Look \(update.version)")
                        .font(themeStore.uiFont(size: fontSize - 1, weight: .semibold))
                        .foregroundStyle(themeStore.fontColor())

                    pill("Update") { updateChecker.startUpdate() }
                        .help("Runs '\(UpdateChecker.homebrewUpgradeCommand)' in Terminal")
                    pill("Notes") { NSWorkspace.shared.open(update.releaseURL) }
                    pill("Dismiss") { updateChecker.dismissCurrent() }

                    Spacer(minLength: 0)
                }

                Text("Update with: \(UpdateChecker.homebrewUpgradeCommand)")
                    .font(themeStore.uiFont(size: fontSize - 2, weight: .regular))
                    .foregroundStyle(themeStore.mutedTextColor())
                    .textSelection(.enabled)
            }
        }
    }

    private func pill(_ title: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Text(title)
                .font(themeStore.uiFont(size: fontSize - 2, weight: .semibold))
                .foregroundStyle(themeStore.fontColor())
                .padding(.horizontal, 10)
                .padding(.vertical, 4)
                .background(.white.opacity(0.16), in: Capsule())
                .overlay(
                    Capsule().strokeBorder(.white.opacity(0.22), lineWidth: 1)
                )
        }
        .buttonStyle(.plain)
        .contentShape(Capsule())
    }
}
