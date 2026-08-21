import AppKit
import Foundation

/// Cmd+E and Cmd+T: act on the selected row through the tools the user named in
/// their config. Composition lives in core (`look-tools`); this decides which
/// row is eligible and what to do with the result.
extension LauncherView {
    func editSelectedResult() {
        runToolAction(AppConstants.Launcher.Tools.editAction, requiresEditable: true)
    }

    func openTerminalForSelectedResult() {
        runToolAction(AppConstants.Launcher.Tools.terminalAction, requiresEditable: false)
    }

    /// The selected row as a path plus whether it is a directory, or nil when
    /// the action does not apply. An app bundle counts as a file, so a terminal
    /// opens the folder holding it rather than descending into the bundle.
    private func toolTarget(requiresEditable: Bool) -> (path: String, isDirectory: Bool)? {
        guard let selected = actionableSelectedResult(), !selected.path.isEmpty else { return nil }

        switch selected.kind {
        case .folder:
            return (selected.path, true)
        case .file:
            return (selected.path, false)
        case .app:
            return requiresEditable ? nil : (selected.path, false)
        default:
            return nil
        }
    }

    private func runToolAction(_ action: String, requiresEditable: Bool) {
        guard let target = toolTarget(requiresEditable: requiresEditable) else { return }
        let path = target.path
        let isDirectory = target.isDirectory

        Task {
            let outcome = await Task.detached(priority: .userInitiated) {
                EngineBridge.shared.performToolAction(action, path: path, isDirectory: isDirectory)
            }.value

            await MainActor.run {
                applyToolOutcome(outcome, path: path)
            }
        }
    }

    private func applyToolOutcome(_ outcome: ToolAction?, path: String) {
        guard let outcome else { return }

        switch outcome.kind {
        case .performed:
            hideLauncherWindow(restorePreviousApp: false)
        case .application:
            launchApplication(tool: outcome.tool, path: outcome.path ?? path)
        case .unavailable, .failed:
            showToolBanner(outcome.reason)
        case .shell:
            // Only the resolve-only entry point returns this; performing
            // reports `performed` or `failed`.
            break
        }
    }

    private func launchApplication(tool: String?, path: String) {
        guard let tool else { return }
        guard let bundle = AppBundleLocator.bundlePath(forAppNamed: tool) else {
            showToolBanner("\(AppConstants.Launcher.Tools.launchFailedBanner) \(tool)")
            return
        }

        NSWorkspace.shared.open(
            [URL(fileURLWithPath: path)],
            withApplicationAt: URL(fileURLWithPath: bundle),
            configuration: NSWorkspace.OpenConfiguration(),
            completionHandler: nil
        )
        hideLauncherWindow(restorePreviousApp: false)
    }

    private func showToolBanner(_ message: String?) {
        guard let message, !message.isEmpty else { return }
        showBanner(
            message,
            style: .info,
            duration: AppConstants.Launcher.Tools.bannerDuration
        )
    }
}
