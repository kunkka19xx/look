import AppKit
import Foundation

/// Cmd+E and Cmd+T: act on the selected row through the tools the user named in
/// their config. Composition lives in core (`look-tools`); this decides which
/// row is eligible and what to do with the result.
extension LauncherView {
    func editSelectedResult() {
        runToolAction(AppConstants.Launcher.Tools.editAction)
    }

    func openTerminalForSelectedResult() {
        runToolAction(AppConstants.Launcher.Tools.terminalAction)
    }

    /// Reveal through the declared `file_manager`, or Finder when none is set.
    /// Core answers which, so linows gets the same rule.
    func revealSelectedResult() {
        guard toolTarget(for: AppConstants.Launcher.Tools.revealAction) != nil else {
            revealSelectedInFinder()
            return
        }
        runToolAction(AppConstants.Launcher.Tools.revealAction)
    }

    /// Whether `action` means anything for a row of this kind.
    ///
    /// Editing and opening a terminal are about a place you work in. An app is a
    /// thing you launch, and the folder holding it is `/Applications`, which is
    /// never "here" — so both are absent for app rows rather than quietly acting
    /// on the wrong directory. Revealing an app is genuinely useful and stays.
    static func toolActionApplies(_ action: String, to kind: LauncherResultKind) -> Bool {
        switch action {
        case AppConstants.Launcher.Tools.revealAction:
            return kind.isFileOrFolder || kind == .app
        default:
            return kind.isFileOrFolder
        }
    }

    /// The selected row as a path plus whether it is a directory, or nil when
    /// the action does not apply to it.
    private func toolTarget(for action: String) -> (path: String, isDirectory: Bool)? {
        guard let selected = actionableSelectedResult(), !selected.path.isEmpty,
            Self.toolActionApplies(action, to: selected.kind)
        else { return nil }

        return (selected.path, selected.kind == .folder)
    }

    private func runToolAction(_ action: String) {
        guard let target = toolTarget(for: action) else { return }
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
            activateTool(named: outcome.tool)
        case .application:
            launchApplication(tool: outcome.tool, path: outcome.path ?? path)
        case .systemDefault:
            revealSelectedInFinder()
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

    /// Bring the terminal forward once its new window exists.
    ///
    /// Nothing else will. `wezterm start` hands a window to a WezTerm that is
    /// already running, and that process never asks to come forward; a freshly
    /// launched app does ask, but Look is an accessory app that stays *active*
    /// after ordering its window out, and one process's request does not
    /// preempt the active app. Look is the only party here holding activation,
    /// so Look is the one that can pass it on.
    ///
    /// Ordered deliberately: this runs while Look is still active, because a
    /// resigned app may no longer be allowed to hand activation to another.
    private func activateTool(named tool: String?) {
        guard let tool, let bundle = AppBundleLocator.bundlePath(forAppNamed: tool) else {
            NSApp.hide(nil)
            return
        }
        let target = URL(fileURLWithPath: bundle).resolvingSymlinksInPath()

        let ownPID = ProcessInfo.processInfo.processIdentifier

        Task { @MainActor in
            try? await Task.sleep(
                nanoseconds: AppConstants.Launcher.Tools.activationSettleNanoseconds)

            for _ in 0..<AppConstants.Launcher.Tools.activationPollAttempts {
                if let app = NSWorkspace.shared.runningApplications.first(where: {
                    $0.bundleURL?.resolvingSymlinksInPath() == target
                }) {
                    // The user may have moved on while a cold terminal started.
                    // Same courtesy `launchApp(at:)` extends to slow apps.
                    if let frontmost = NSWorkspace.shared.frontmostApplication,
                        frontmost.processIdentifier != ownPID,
                        frontmost.processIdentifier != app.processIdentifier
                    {
                        return
                    }
                    app.activate()
                    return
                }
                try? await Task.sleep(
                    nanoseconds: AppConstants.Launcher.Tools.activationPollNanoseconds)
            }

            // It never appeared, so at least stop holding focus ourselves.
            NSApp.hide(nil)
        }
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
