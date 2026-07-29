import AppKit
import SwiftUI

extension LauncherView {
    /// False when another screen owns the launcher, so the caller can leave the
    /// key alone instead of swallowing it into a no-op.
    func hideSelectedApp() -> Bool {
        guard !isCommandMode, !appUIState.showsThemeSettings, !showsHelpScreen else { return false }
        guard pendingEmptyTrashCount == nil else { return false }

        guard let selectedResultID,
              let selected = displayedResults.first(where: { $0.id == selectedResultID }),
              selected.kind == .app,
              !selected.path.isEmpty,
              selected.path.hasSuffix(Self.appBundleExtension)
        else {
            showBanner("Select an app first", style: .info, duration: 1.2)
            return true
        }

        pendingHideAppResult = selected
        return true
    }

    func confirmHideSelectedApp() {
        guard let selected = pendingHideAppResult else { return }
        pendingHideAppResult = nil

        switch appendAppExcludeName(selected.title) {
        case .writeFailed:
            showBanner("Failed to update .look.config", style: .error, duration: 1.6)
        case .alreadyExcluded:
            showBanner("\(selected.title) is already hidden", style: .info, duration: 1.6)
        case .added:
            let reloaded = reloadConfig(
                successMessage: "Hidden \(selected.title)",
                successStyle: .success,
                successDuration: 1.2
            )
            // The name is already on disk, so a failed reload only delays the hide
            // until next launch. The bare reload error would read as "nothing saved".
            if !reloaded {
                showBanner(
                    "Hid \(selected.title) in .look.config, but the reload failed",
                    style: .error,
                    duration: 3.0
                )
            }
        }
    }

    func cancelHideSelectedApp() {
        pendingHideAppResult = nil
    }

    private enum AppExcludeResult {
        case added
        case alreadyExcluded
        case writeFailed
    }

    private func appendAppExcludeName(_ appName: String) -> AppExcludeResult {
        let trimmedName = appName.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedName.isEmpty else { return .writeFailed }

        let path = URL(fileURLWithPath: ConfigPathResolver.resolvedPath())
        guard let raw = try? String(contentsOf: path, encoding: .utf8) else {
            return .writeFailed
        }

        var lines = ConfigFileLines.parse(raw)
        let values = ConfigFileLines.keyValues(raw)
        var names = ConfigFileLines.parseList(values[Self.appExcludeNamesKey] ?? "")

        // Normalised, or `Safari`, `safari`, and `Safari.app` pile up as three
        // entries the backend treats as one.
        let normalizedName = normalizeExcludeName(trimmedName)
        guard !names.contains(where: { normalizeExcludeName($0) == normalizedName }) else {
            return .alreadyExcluded
        }

        names.append(trimmedName)
        ConfigFileLines.upsert(
            &lines,
            key: Self.appExcludeNamesKey,
            value: ConfigFileLines.renderList(names)
        )

        do {
            try ConfigFileLines.render(lines).write(to: path, atomically: true, encoding: .utf8)
            return .added
        } catch {
            return .writeFailed
        }
    }

    /// Mirrors `normalize_app_name` in `core/engine/src/platform/macos/apps.rs`.
    private func normalizeExcludeName(_ value: String) -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        let withoutSuffix = trimmed.hasSuffix(Self.appBundleExtension)
            ? String(trimmed.dropLast(Self.appBundleExtension.count))
            : trimmed
        return withoutSuffix.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
    }

    private static let appExcludeNamesKey = "app_exclude_names"
    private static let appBundleExtension = ".app"
}
