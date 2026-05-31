import AppKit
import OSLog
import SwiftUI

private let openLaunchLog = Logger(subsystem: "noah-code.Look", category: "open")

extension LauncherView {
    func openSelectedApp() {
        guard let selectedResultID,
            let selected = displayedResults.first(where: { $0.id == selectedResultID })
        else { return }

        switch selected.kind {
        case .app:
            guard ensureTargetExists(selected) else { return }
            launchApp(at: selected.path)
            recordOpen(selected, action: "open_app")
            hideLauncherWindow(restorePreviousApp: false)
        case .file:
            guard ensureTargetExists(selected) else { return }
            openTargetAsync(selected.path)
            recordOpen(selected, action: "open_file")
            hideLauncherWindow(restorePreviousApp: false)
        case .folder:
            guard ensureTargetExists(selected) else { return }
            openTargetAsync(selected.path)
            // Quick-folder entries are ephemeral filesystem suggestions, not
            // ranked candidates — they aren't in the usage index.
            if !selected.id.hasPrefix(AppConstants.Launcher.QuickFolder.idPrefix) {
                recordOpen(selected, action: "open_folder")
            }
            hideLauncherWindow(restorePreviousApp: false)
        case .clipboard:
            guard let content = selected.clipboardContent, !content.isEmpty else { return }
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(content, forType: .string)
            showBanner(
                AppConstants.Launcher.Clipboard.copiedBanner,
                style: .success,
                duration: AppConstants.Launcher.Clipboard.copiedBannerDuration
            )
        }
    }

    func openTargetAsync(_ target: String) {
        if isURLScheme(target) {
            openURLScheme(target)
        } else {
            openFilePath(target)
        }
    }

    private func launchApp(at path: String) {
        // Settings entries arrive as .app with URL-scheme paths
        // (x-apple.systempreferences:…); dispatch those via LaunchServices
        // rather than openApplication(at:), which only understands app bundles.
        if isURLScheme(path) {
            openURLScheme(path)
            return
        }

        let config = NSWorkspace.OpenConfiguration()
        config.activates = true
        let ownPID = ProcessInfo.processInfo.processIdentifier
        NSWorkspace.shared.openApplication(at: URL(fileURLWithPath: path), configuration: config) { runningApp, error in
            DispatchQueue.main.async {
                if let error {
                    openLaunchLog.error("openApplication failed for \(path, privacy: .public): \(error.localizedDescription, privacy: .public)")
                    return
                }
                guard let runningApp else { return }
                // Heavy apps (Slack, Discord) can take seconds to finish
                // launching. If the user has since moved to a different app,
                // don't steal focus back.
                if let frontmost = NSWorkspace.shared.frontmostApplication,
                   frontmost.processIdentifier != ownPID,
                   frontmost.processIdentifier != runningApp.processIdentifier {
                    return
                }
                runningApp.activate()
            }
        }
    }

    private func openURLScheme(_ target: String) {
        // The legacy open(URL) API non-blockingly hands custom schemes
        // (x-apple.systempreferences:, https:, …) to LaunchServices. The
        // configuration-based async variant misroutes non-file URLs to Finder.
        guard let url = URL(string: target) else {
            showBanner("Invalid target URL", style: .error, duration: 1.2)
            return
        }
        if !NSWorkspace.shared.open(url) {
            openLaunchLog.error("open failed for URL \(target, privacy: .public)")
        }
    }

    private func openFilePath(_ target: String) {
        let config = NSWorkspace.OpenConfiguration()
        config.activates = true
        NSWorkspace.shared.open(URL(fileURLWithPath: target), configuration: config) { _, error in
            if let error {
                openLaunchLog.error("open failed for \(target, privacy: .public): \(error.localizedDescription, privacy: .public)")
            }
        }
    }

    private func isURLScheme(_ target: String) -> Bool {
        target.contains(":") && !target.hasPrefix("/")
    }

    private func recordOpen(_ selected: LauncherResult, action: String) {
        if let error = bridge.recordUsage(candidateID: selected.id, action: action) {
            showBanner(error.userFacingMessage, style: .info, duration: 1.4)
        }
    }

    /// Guards against opening a target that no longer exists on disk.
    ///
    /// A candidate can linger in the index after its bundle/file is removed
    /// (an app uninstalled but still indexed, a file moved or deleted). Opening
    /// it would fail silently in the async completion handler, and — because we
    /// record usage on intent — would also boost a dead entry, so it keeps
    /// surfacing and keeps failing. When the target is gone we surface it to the
    /// user, kick off a background reindex so the stale candidate gets pruned,
    /// and skip the open/record/hide. Recording usage is reserved for targets
    /// that actually exist; intent for a thing that no longer exists isn't a
    /// signal worth keeping.
    ///
    /// Returns `true` when the target is openable.
    private func ensureTargetExists(_ selected: LauncherResult) -> Bool {
        // URL-scheme targets (settings panes, custom schemes) aren't filesystem
        // paths and can't be stat'd — treat them as openable.
        if isURLScheme(selected.path) { return true }
        if FileManager.default.fileExists(atPath: selected.path) { return true }

        showBanner(
            "This \(selected.kind.rawValue) no longer exists — refreshing index",
            style: .error,
            duration: 1.6
        )
        _ = bridge.requestIndexRefresh()
        return false
    }

    func revealSelectedInFinder() {
        guard !isCommandMode,
              let selectedID = selectedResultID,
              let selected = displayedResults.first(where: { $0.id == selectedID })
        else { return }

        switch selected.kind {
        case .app, .file, .folder:
            if selected.path.contains(":") && !selected.path.hasPrefix("/") {
                if let url = URL(string: selected.path) {
                    NSWorkspace.shared.open(url)
                } else {
                    showBanner(
                        AppConstants.Launcher.Finder.cannotRevealBanner,
                        style: .info,
                        duration: AppConstants.Launcher.Clipboard.infoBannerDuration
                    )
                }
            } else {
                NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: selected.path)])
            }
        case .clipboard:
            showBanner(
                AppConstants.Launcher.Clipboard.nonFileBanner,
                style: .info,
                duration: AppConstants.Launcher.Clipboard.infoBannerDuration
            )
        }
    }

    func togglePickForSelectedResult() {
        guard !isCommandMode,
              let selectedID = selectedResultID,
              let selected = displayedResults.first(where: { $0.id == selectedID })
        else { return }
        guard selected.kind == .file || selected.kind == .folder else {
            showBanner("Only files or folders can be picked", style: .info, duration: 1.0)
            return
        }
        let key = Self.pickedKey(for: selected)
        if let idx = pickedKeys.firstIndex(of: key) {
            pickedKeys.remove(at: idx)
            pickedResultsByKey.removeValue(forKey: key)
        } else {
            pickedKeys.append(key)
            pickedResultsByKey[key] = selected
        }
        writePickedToPasteboard()
    }

    func removePicked(key: String) {
        guard let idx = pickedKeys.firstIndex(of: key) else { return }
        pickedKeys.remove(at: idx)
        pickedResultsByKey.removeValue(forKey: key)
        writePickedToPasteboard()
    }

    func clearAllPicked() {
        guard !pickedKeys.isEmpty else { return }
        pickedKeys.removeAll()
        pickedResultsByKey.removeAll()
        NSPasteboard.general.clearContents()
        showBanner("Cleared picked items", style: .info, duration: 1.0)
    }

    func writePickedToPasteboard() {
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        guard !pickedKeys.isEmpty else { return }
        var objects: [NSPasteboardWriting] = []
        for key in pickedKeys {
            guard let r = pickedResultsByKey[key], r.kind == .file || r.kind == .folder else { continue }
            objects.append(URL(fileURLWithPath: r.path) as NSURL)
            objects.append(r.path as NSString)
        }
        let didWrite = pasteboard.writeObjects(objects)
        if didWrite {
            showBanner("Picked \(pickedKeys.count) item(s)", style: .success, duration: 1.0)
        } else {
            showBanner("Pick failed", style: .error, duration: 1.0)
        }
    }

    @discardableResult
    func copySelectedResultToPasteboard() -> Bool {
        guard !isCommandMode,
              let selectedID = selectedResultID,
              let selected = displayedResults.first(where: { $0.id == selectedID })
        else { return false }

        guard selected.kind == .file || selected.kind == .folder else { return false }

        let targetURL = URL(fileURLWithPath: selected.path)
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        let didWrite = pasteboard.writeObjects([targetURL as NSURL, selected.path as NSString])

        if didWrite {
            showBanner("Copied \(selected.kind.rawValue) to pasteboard", style: .success, duration: 1.0)
        } else {
            showBanner("Copy failed", style: .error, duration: 1.0)
        }

        return didWrite
    }

    func toggleHelpScreen() {
        guard !appUIState.showsThemeSettings else { return }
        guard !isCommandMode else {
            showBanner(
                AppConstants.Launcher.Help.commandModeInfoBanner,
                style: .info,
                duration: AppConstants.Launcher.Clipboard.infoBannerDuration
            )
            return
        }
        showsHelpScreen.toggle()
    }

    @discardableResult
    func dismissHelpIfVisible() -> Bool {
        guard showsHelpScreen else { return false }
        showsHelpScreen = false
        return true
    }

    func deleteClipboardResult(resultID: String) {
        guard let entryID = LauncherClipboardFeature.entryID(fromResultID: resultID) else { return }
        clipboardStore.deleteEntry(id: entryID)

        if selectedResultID == resultID {
            selectedResultID = displayedResults.first?.id
        }

        showBanner(
            AppConstants.Launcher.Clipboard.deletedBanner,
            style: .info,
            duration: AppConstants.Launcher.Clipboard.infoBannerDuration
        )
    }

    func refreshClipboardSelectionIfNeeded() {
        guard !isCommandMode, isClipboardQuery else { return }

        if let selectedResultID,
           displayedResults.contains(where: { $0.id == selectedResultID }) {
            return
        }

        selectedResultID = displayedResults.first?.id
    }
}
