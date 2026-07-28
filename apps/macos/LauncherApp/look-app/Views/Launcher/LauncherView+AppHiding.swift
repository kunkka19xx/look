import AppKit
import SwiftUI

extension LauncherView {
    func hideSelectedApp() {
        guard !isCommandMode, !appUIState.showsThemeSettings, !showsHelpScreen else { return }
        guard pendingEmptyTrashCount == nil else { return }

        guard let selectedResultID,
              let selected = displayedResults.first(where: { $0.id == selectedResultID }),
              selected.kind == .app,
              !selected.path.isEmpty,
              selected.path.hasSuffix(".app")
        else {
            showBanner("Select an app first", style: .info, duration: 1.2)
            return
        }

        pendingHideAppResult = selected
    }

    func confirmHideSelectedApp() {
        guard let selected = pendingHideAppResult else { return }
        pendingHideAppResult = nil

        guard appendAppExcludeName(selected.title) else {
            showBanner("Failed to update .look.config", style: .error, duration: 1.6)
            return
        }

        if reloadConfig() {
            showBanner("Hidden \(selected.title)", style: .success, duration: 1.2)
        }
    }

    func cancelHideSelectedApp() {
        pendingHideAppResult = nil
    }

    private func appendAppExcludeName(_ appName: String) -> Bool {
        let trimmedName = appName.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedName.isEmpty else { return false }

        let path = URL(fileURLWithPath: ConfigPathResolver.resolvedPath())
        guard let raw = try? String(contentsOf: path, encoding: .utf8) else {
            return false
        }

        var lines = ConfigFileLines.parse(raw)
        let values = ConfigFileLines.keyValues(raw)
        let current = values["app_exclude_names"] ?? ""
        var names = current
            .split(separator: ",", omittingEmptySubsequences: true)
            .map { String($0).trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty }

        names.append(trimmedName)
        ConfigFileLines.upsert(&lines, key: "app_exclude_names", value: names.joined(separator: ","))

        do {
            try ConfigFileLines.render(lines).write(to: path, atomically: true, encoding: .utf8)
            return true
        } catch {
            return false
        }
    }
}
