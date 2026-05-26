import AppKit
import SwiftUI

extension LauncherView {
    func openSelectedApp() {
        guard let selectedResultID,
            let selected = displayedResults.first(where: { $0.id == selectedResultID })
        else { return }

        switch selected.kind {
        case .app:
            if openTarget(selected.path) {
                bringOpenedAppToFront(appBundlePath: selected.path)
                if let error = bridge.recordUsage(candidateID: selected.id, action: "open_app") {
                    showBanner(error.userFacingMessage, style: .info, duration: 1.4)
                }
                hideLauncherWindow(restorePreviousApp: false)
            }
        case .file:
            if openTarget(selected.path) {
                if let error = bridge.recordUsage(candidateID: selected.id, action: "open_file") {
                    showBanner(error.userFacingMessage, style: .info, duration: 1.4)
                }
                hideLauncherWindow(restorePreviousApp: false)
            }
        case .folder:
            if openTarget(selected.path) {
                if !selected.id.hasPrefix(AppConstants.Launcher.QuickFolder.idPrefix),
                    let error = bridge.recordUsage(candidateID: selected.id, action: "open_folder")
                {
                    showBanner(error.userFacingMessage, style: .info, duration: 1.4)
                }
                hideLauncherWindow(restorePreviousApp: false)
            }
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

    @discardableResult
    func openTarget(_ target: String) -> Bool {
        if target.contains(":") && !target.hasPrefix("/") {
            if let url = URL(string: target) {
                if NSWorkspace.shared.open(url) {
                    return true
                }
                showBanner("Could not open this item right now", style: .error, duration: 1.2)
                return false
            }
            showBanner("Invalid target URL", style: .error, duration: 1.2)
            return false
        }

        if NSWorkspace.shared.open(URL(fileURLWithPath: target)) {
            return true
        }

        showBanner("Could not open this path", style: .error, duration: 1.2)
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
