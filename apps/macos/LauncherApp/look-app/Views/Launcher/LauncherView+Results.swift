import AppKit
import OSLog
import SwiftUI

// `Logger` is Sendable; declare it nonisolated so AppKit's background open
// completion handlers can log without a main-actor hop.
nonisolated private let openLaunchLog = Logger(subsystem: "noah-code.Look", category: "open")

extension LauncherView {
    func openSelectedApp() {
        guard let selectedResultID,
            let selected = displayedResults.first(where: { $0.id == selectedResultID })
        else { return }

        switch SyntheticRow.classify(resultID: selected.id) {
        case .prefixSuggestion(let prefix):
            // Prefix-discovery menu: choosing an entry fills its prefix into
            // the field (cursor ready for the term) rather than opening anything.
            query = prefix
            isQueryFocused = true
            return
        case .webSuggestion(let suggestion):
            // Google autocomplete row: run the web search for that suggestion.
            performWebSearch(for: suggestion)
            hideLauncherWindow(restorePreviousApp: false)
            return
        case .webURL(let urlString):
            // URL-like query row (live or from history): open the resolved
            // address in the default browser and remember it for faster re-open later.
            openURLScheme(urlString)
            bridge.recordURLHit(url: urlString)
            hideLauncherWindow(restorePreviousApp: false)
            return
        case .commandSuggestion(let commandID):
            // Command-discovery row: enter that command's panel (empty input).
            enterCommandMode(commandID: commandID, prefilledInput: "")
            return
        case .calc(let raw):
            // Calculator row: copy the raw value, launcher out of the way.
            // History keeps the labeled working (`2+2 = 4`); the paste is the number.
            clipboardStore.recordLabeled(display: "\(selected.calcExpression ?? "") = \(selected.title)", payload: raw)
            hideLauncherWindow(restorePreviousApp: false)
            return
        case .aiAction:
            // Planner-proposed action row: the row the user just read IS the
            // confirmation - one Enter performs it, the banner offers ⌘Z.
            guard let planned = mainBarAction else { return }
            runQuickAction(planned)
            return
        case .meeting(let url), .call(let url):
            // Join and call rows: the link travelled in the row id, so pressing
            // Enter never re-reads the calendar or Contacts, and can never open
            // something other than what the row named.
            openURLScheme(url)
            hideLauncherWindow(restorePreviousApp: false)
            return
        case nil:
            break
        }

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
            // ranked candidates - they aren't in the usage index.
            if !selected.id.hasPrefix(AppConstants.Launcher.QuickFolder.idPrefix) {
                recordOpen(selected, action: "open_folder")
            }
            hideLauncherWindow(restorePreviousApp: false)
        case .action:
            // A declared block: there is nothing to open, so Enter performs its
            // steps. The window goes away first, since the steps usually launch
            // something that wants the focus.
            performSourceBlock(selected)
        case .clipboard:
            // Labeled entries (e.g. calculator results) paste their value, not
            // the label shown in the list.
            guard let content = selected.clipboardPayload ?? selected.clipboardContent, !content.isEmpty else { return }
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(content, forType: .string)
            showBanner(
                AppConstants.Launcher.Clipboard.copiedBanner,
                style: .success,
                duration: AppConstants.Launcher.Clipboard.copiedBannerDuration
            )
        case .process:
            // Enter measures CPU on demand and keeps the launcher open.
            measureCPUForSelectedProcess()
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
        DeleteTargetLogic.isURLScheme(target)
    }

    /// Reveals the `.toml` (or script) a block was declared in. Reading the
    /// sources directory touches disk, so it happens off the main thread.
    private func revealDeclaringFile(for selected: LauncherResult) {
        let candidateID = selected.id
        Task {
            let block = await Task.detached(priority: .userInitiated) {
                EngineBridge.shared.sourceBlock(candidateID: candidateID)
            }.value
            await MainActor.run {
                guard let file = block?.file, FileManager.default.fileExists(atPath: file) else {
                    showBanner("Couldn't find the file that declares this", style: .info, duration: 1.6)
                    return
                }
                NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: file)])
            }
        }
    }

    /// Performs a declared block's steps. Usage is recorded on intent, like an
    /// open, so a routine you run every morning ranks like one.
    private func performSourceBlock(_ selected: LauncherResult) {
        // Resolve BEFORE recording usage or hiding: an unparseable id would
        // otherwise rank the row up, close the window, and do nothing, with no
        // way for the user to tell that it failed.
        guard let blockID = SourceBlockCatalog.blockID(fromCandidateID: selected.id) else {
            showBanner("Couldn't tell which block this row belongs to", style: .error, duration: 2.0)
            return
        }
        recordOpen(selected, action: AppConstants.Launcher.SourceBlock.usageAction)
        hideLauncherWindow(restorePreviousApp: false)

        let name = selected.title
        let row = (id: selected.id, title: selected.title, path: selected.path, query: query)
        Task {
            let outcome = await Task.detached(priority: .userInitiated) {
                EngineBridge.shared.performBlock(
                    blockID: blockID,
                    rowID: row.id,
                    rowTitle: row.title,
                    rowPath: row.path,
                    query: row.query
                )
            }.value
            await MainActor.run {
                guard let failure = outcome.errors.first else { return }
                // Steps are detached, so this only fires when one could not be
                // started at all. A step's own exit code is its business.
                showBanner("\(name): \(failure)", style: .error, duration: 4.0)
            }
        }
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
    /// it would fail silently in the async completion handler, and - because we
    /// record usage on intent - would also boost a dead entry, so it keeps
    /// surfacing and keeps failing. When the target is gone we surface it to the
    /// user, kick off a background reindex so the stale candidate gets pruned,
    /// and skip the open/record/hide. Recording usage is reserved for targets
    /// that actually exist; intent for a thing that no longer exists isn't a
    /// signal worth keeping.
    ///
    /// Returns `true` when the target is openable.
    private func ensureTargetExists(_ selected: LauncherResult) -> Bool {
        // URL-scheme targets (settings panes, custom schemes) aren't filesystem
        // paths and can't be stat'd - treat them as openable.
        if isURLScheme(selected.path) { return true }
        if FileManager.default.fileExists(atPath: selected.path) { return true }

        showBanner(
            "This \(selected.kind.rawValue) no longer exists - refreshing index",
            style: .error,
            duration: 1.6
        )
        _ = bridge.requestIndexRefresh()
        return false
    }

    /// The result eligible for a selection-scoped action (reveal, pick, copy)
    /// right now, or nil when command mode or the prefix-discovery menu has
    /// the field, or nothing (or an unmatched id) is selected.
    func actionableSelectedResult() -> LauncherResult? {
        guard !isCommandMode, !isPrefixSuggestionQuery,
              let selectedID = selectedResultID
        else { return nil }
        return displayedResults.first(where: { $0.id == selectedID })
    }

    func revealSelectedInFinder() {
        guard let selected = actionableSelectedResult() else { return }

        switch selected.kind {
        case .app, .file, .folder:
            revealPathInFinder(selected.path)
        case .clipboard:
            showBanner(
                AppConstants.Launcher.Clipboard.nonFileBanner,
                style: .info,
                duration: AppConstants.Launcher.Clipboard.infoBannerDuration
            )
        case .process:
            showBanner("Processes can't be revealed in Finder", style: .info, duration: 1.2)
        case .action:
            // No file of its own, but the declaration that created it is a real
            // file and is the thing the user wants to get to from here.
            revealDeclaringFile(for: selected)
        }
    }

    /// Reveals one path rather than whatever happens to be selected now. An
    /// action resolved off the main thread must reveal the row it was resolved
    /// for, even if the user has since moved the selection.
    func revealPathInFinder(_ path: String) {
        guard path.contains(":"), !path.hasPrefix("/") else {
            NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: path)])
            return
        }
        guard let url = URL(string: path) else {
            showBanner(
                AppConstants.Launcher.Finder.cannotRevealBanner,
                style: .info,
                duration: AppConstants.Launcher.Clipboard.infoBannerDuration
            )
            return
        }
        NSWorkspace.shared.open(url)
    }

    func togglePickForSelectedResult() {
        guard let selected = actionableSelectedResult() else { return }
        guard selected.kind.isFileOrFolder else {
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

    /// Opens every picked file/folder in one shot, then clears the picks and
    /// hides the launcher. The multi-target counterpart to `openSelectedApp`.
    /// Mirrors linows `openAllPicked`. Picks are restricted to files/folders at
    /// pick time, so apps/clipboard rows can't appear here, but we guard kind
    /// anyway. Targets that no longer exist are skipped (and trigger a reindex
    /// via `ensureTargetExists`) rather than aborting the whole batch.
    func openAllPicked() {
        guard !pickedKeys.isEmpty else { return }
        let items = pickedKeys.compactMap { pickedResultsByKey[$0] }
        var openedCount = 0
        for item in items {
            guard item.kind.isFileOrFolder else { continue }
            guard ensureTargetExists(item) else { continue }
            openTargetAsync(item.path)
            // Quick-folder entries are ephemeral suggestions, not indexed
            // candidates, so don't record usage for them (matches openSelectedApp).
            if !(item.kind == .folder && item.id.hasPrefix(AppConstants.Launcher.QuickFolder.idPrefix)) {
                recordOpen(item, action: item.kind == .folder ? "open_folder" : "open_file")
            }
            openedCount += 1
        }
        guard openedCount > 0 else { return }
        pickedKeys.removeAll()
        pickedResultsByKey.removeAll()
        NSPasteboard.general.clearContents()
        hideLauncherWindow(restorePreviousApp: false)
    }

    /// Picked FILES, in pick order. Folders are excluded: a text op reads
    /// contents, and a folder has none.
    func pickedFilePathsForTextOp() -> [String] {
        pickedKeys.compactMap { pickedResultsByKey[$0] }
            .filter { $0.kind == .file }
            .map(\.path)
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
            guard let r = pickedResultsByKey[key], r.kind.isFileOrFolder else { continue }
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
        guard let selected = actionableSelectedResult() else { return false }

        // In `ps"` mode, Cmd+C copies the selected process's PID.
        if selected.kind == .process {
            copySelectedProcessPID()
            return true
        }

        guard selected.kind.isFileOrFolder else { return false }

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
        if !showsHelpScreen { restoreFocusAfterHelp() }
    }

    @discardableResult
    func dismissHelpIfVisible() -> Bool {
        guard showsHelpScreen else { return false }
        showsHelpScreen = false
        restoreFocusAfterHelp()
        return true
    }

    /// Put the caret back in the search field after help closes. The top row is
    /// dropped entirely while help is up, so the field is a NEW view by the time
    /// we return and setting `isQueryFocused` alone lands on nothing: the staged
    /// recovery delays are what wait for it to exist. Not `activateApp`, the
    /// launcher is already frontmost.
    private func restoreFocusAfterHelp() {
        focusActiveInput(activateApp: false)
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

    // MARK: - Delete to Trash

    /// File/folder targets the delete action should act on: the picked basket
    /// when non-empty, otherwise the single selected result. Non-deletable
    /// kinds, URL-scheme paths, and items gone from disk are filtered out by
    /// `DeleteTargetLogic.eligible`.
    func eligibleDeleteTargets() -> [DeleteCommand.Target] {
        let candidates: [LauncherResult]
        if !pickedKeys.isEmpty {
            candidates = pickedKeys.compactMap { pickedResultsByKey[$0] }
        } else if let selectedResultID,
                  let selected = displayedResults.first(where: { $0.id == selectedResultID }) {
            candidates = [selected]
        } else {
            candidates = []
        }

        return DeleteTargetLogic
            .eligible(from: candidates, fileExists: { FileManager.default.fileExists(atPath: $0) })
            .map { result in
                DeleteCommand.Target(
                    id: result.id,
                    displayName: result.title,
                    path: result.path,
                    kind: result.kind,
                    icon: NSWorkspace.shared.icon(forFile: result.path)
                )
            }
    }

    /// Cmd+D entry point. Files/folders move to Trash immediately (recoverable,
    /// no confirmation - like Finder's Cmd+Delete). When the single selection is
    /// the Trash quick folder, routes to Empty Trash, which DOES confirm because
    /// it's permanent.
    func requestDeleteSelection() {
        guard isLauncherIdle else { return }
        // In `ps"` mode, Cmd+D kills the selected process (SIGKILL) instead of
        // trashing a file.
        if isProcessQuery, selectedProcessPID != nil {
            killSelectedProcess()
            return
        }
        // Don't stack an Empty Trash confirmation, nor start work while a
        // previous trash/empty is still running (recycle / Finder are async).
        guard pendingEmptyTrashCount == nil, !isDeleteInFlight else { return }

        // Cmd+D on the pinned Trash folder empties it rather than trashing it.
        if pickedKeys.isEmpty,
           let selectedResultID,
           let selected = displayedResults.first(where: { $0.id == selectedResultID }),
           selected.kind == .folder,
           DeleteTargetLogic.isTrashPath(selected.path, homeDirectory: NSHomeDirectory()) {
            // Count comes from Finder (TCC blocks reading ~/.Trash directly);
            // this is the right moment to prompt for Automation permission.
            guard let count = EmptyTrashCommand.itemCount() else {
                showBanner(
                    "Allow Look to control Finder in System Settings ▸ Privacy ▸ Automation to empty the Trash",
                    style: .error,
                    duration: 2.8
                )
                return
            }
            guard count > 0 else {
                showBanner("Trash is already empty", style: .info, duration: 1.2)
                return
            }
            pendingEmptyTrashCount = count
            return
        }

        let targets = eligibleDeleteTargets()
        guard !targets.isEmpty else {
            showBanner("Select a file or folder to delete", style: .info, duration: 1.2)
            return
        }
        // Recoverable - trash straight away, no confirmation.
        runDeleteCommand(targets: targets)
    }

    /// Confirm/cancel only apply to the permanent Empty Trash prompt now.
    func confirmDeleteSelection() {
        guard pendingEmptyTrashCount != nil else { return }
        pendingEmptyTrashCount = nil
        runEmptyTrash()
    }

    func cancelDeleteSelection() {
        pendingEmptyTrashCount = nil
    }

    private func runEmptyTrash() {
        isDeleteInFlight = true
        // Finder's "empty the trash" can take seconds on a large Trash; run it
        // off the main thread so the launcher window stays responsive.
        EmptyTrashCommand.empty { error in
            // Completion is delivered on the main queue (see EmptyTrashCommand),
            // so assert main-actor isolation to touch UI state.
            MainActor.assumeIsolated {
                isDeleteInFlight = false
                if let error {
                    showBanner(error, style: .error, duration: 2.6)
                } else {
                    showBanner("Emptied Trash", style: .success, duration: 1.6)
                }
            }
        }
    }

    private func runDeleteCommand(targets: [DeleteCommand.Target]) {
        let targetsByID = Dictionary(targets.map { ($0.id, $0) }, uniquingKeysWith: { first, _ in first })

        isDeleteInFlight = true
        DeleteCommand.trash(targets) { [self] outcome in
            isDeleteInFlight = false
            let trashed = Set(outcome.trashedIDs)

            // Drop trashed items from the picked basket.
            for id in outcome.trashedIDs {
                guard let target = targetsByID[id] else { continue }
                let key = "\(target.kind.rawValue)|\(target.path)"
                if let idx = pickedKeys.firstIndex(of: key) {
                    pickedKeys.remove(at: idx)
                    pickedResultsByKey.removeValue(forKey: key)
                }
            }
            if pickedKeys.isEmpty {
                NSPasteboard.general.clearContents()
            } else {
                writePickedToPasteboard()
            }

            // Drop the trashed rows from the on-screen results immediately - the
            // background index refresh below is async and would otherwise leave
            // the now-gone items visible (and un-previewable) until the next search.
            backendResults.removeAll { trashed.contains($0.id) }

            // Keep selection valid if it pointed at a now-removed row.
            if let selectedResultID, !displayedResults.contains(where: { $0.id == selectedResultID }) {
                self.selectedResultID = displayedResults.first?.id
            }

            let message = DeleteTargetLogic.resultMessage(
                trashedCount: outcome.trashedCount,
                failureCount: outcome.failures.count,
                firstFailure: outcome.firstFailure
            )
            showBanner(message.text, style: message.isError ? .error : .success, duration: message.isError ? 2.0 : 1.4)

            _ = bridge.requestIndexRefresh()
        }
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
